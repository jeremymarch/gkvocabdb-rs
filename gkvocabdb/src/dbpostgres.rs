/*
gkvocabdb

Copyright (C) 2021  Jeremy March

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU Affero General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <https://www.gnu.org/licenses/>.
*/
use crate::AssignmentRow;
use crate::AssignmentTree;
use crate::ConnectionInfo;
use crate::GlossEntry;
use crate::GlossOccurrence;
use crate::GlosserDb;
use crate::GlosserDbTrx;
use crate::GlosserError;
use crate::LemmatizerRecord;
use crate::SmallWord;
use crate::TextWord;
use crate::UpdateType;
use crate::WordRow;
use secrecy::ExposeSecret;
use secrecy::Secret;
use sqlx::postgres::PgPool;
use sqlx::postgres::PgRow;
use sqlx::Postgres;
use sqlx::Row;
use sqlx::Transaction;
use std::collections::HashMap;
use std::collections::HashSet;
use unicode_normalization::UnicodeNormalization;

/*
pub async fn get_seq_by_prefix(pool: &SqlitePool, table:&str, prefix:&str) -> Result<u32, GlosserError> {
  let query = format!("SELECT seq FROM {} WHERE sortalpha >= '{}' ORDER BY sortalpha LIMIT 1;", table, prefix);

  let rec:Result<(u32,), GlosserError> = sqlx::query_as(&query)
  .fetch_one(pool)
  .await;

  match rec {
      Ok(r) => Ok(r.0),
      Err(GlosserError::UnknownError) => { //not found, return seq of last word
          let max_query = format!("SELECT MAX(seq) as seq,sortalpha FROM {} LIMIT 1;", table);
          let max_rec:(u32,) = sqlx::query_as(&max_query)  //fake it by loading it into DefRow for now
          .fetch_one(pool)
          .await?;

          Ok(max_rec.0)
      },
      Err(r) => Err(r)
  }
}


*/

pub fn map_sqlx_error(err: sqlx::Error) -> GlosserError {
    match err {
        sqlx::Error::Configuration(e) => {
            GlosserError::Database(format!("sqlx Configuration: {}", e))
        }
        sqlx::Error::Database(e) => GlosserError::Database(format!("sqlx Database: {}", e)),
        sqlx::Error::Io(e) => GlosserError::Database(format!("sqlx Io: {}", e)),
        sqlx::Error::Tls(e) => GlosserError::Database(format!("sqlx Tls: {}", e)),
        sqlx::Error::Protocol(e) => GlosserError::Database(format!("sqlx Protocol: {}", e)),
        sqlx::Error::RowNotFound => GlosserError::Database(String::from("sqlx RowNotFound")),
        sqlx::Error::TypeNotFound { .. } => {
            GlosserError::Database(String::from("sqlx TypeNotFound"))
        }
        sqlx::Error::ColumnIndexOutOfBounds { .. } => {
            GlosserError::Database(String::from("sqlx ColumnIndexOutOfBounds"))
        }
        sqlx::Error::ColumnNotFound(e) => {
            GlosserError::Database(format!("sqlx ColumnNotFound: {}", e))
        }
        sqlx::Error::ColumnDecode { .. } => {
            GlosserError::Database(String::from("sqlx ColumnDecode"))
        }
        sqlx::Error::Decode(e) => GlosserError::Database(format!("sqlx Decode: {}", e)),
        sqlx::Error::PoolTimedOut => GlosserError::Database(String::from("sqlx PoolTimedOut")),
        sqlx::Error::PoolClosed => GlosserError::Database(String::from("sqlx PoolClosed")),
        sqlx::Error::WorkerCrashed => GlosserError::Database(String::from("sqlx WorkerCrashed")),
        sqlx::Error::Migrate(e) => GlosserError::Database(format!("sqlx Migrate: {}", e)),
        _ => GlosserError::Database(String::from("sqlx unknown error")),
    }
}

#[derive(Clone, Debug)]
pub struct GlosserDbPostgres {
    pub db: PgPool,
}

pub struct GlosserDbPostgresTrx<'a> {
    pub tx: Transaction<'a, Postgres>,
}

use async_trait::async_trait;

#[async_trait]
impl GlosserDb for GlosserDbPostgres {
    async fn begin_tx(&self) -> Result<Box<dyn GlosserDbTrx>, GlosserError> {
        Ok(Box::new(GlosserDbPostgresTrx {
            tx: self.db.begin().await.map_err(map_sqlx_error)?,
        }))
    }
}

#[async_trait]
impl GlosserDbTrx for GlosserDbPostgresTrx<'_> {
    async fn commit_tx(self: Box<Self>) -> Result<(), GlosserError> {
        self.tx.commit().await.map_err(map_sqlx_error)
    }
    async fn rollback_tx(self: Box<Self>) -> Result<(), GlosserError> {
        self.tx.rollback().await.map_err(map_sqlx_error)
    }

    async fn load_lemmatizer(&mut self) -> Result<(), GlosserError> {
        if let Ok(mut reader) = csv::Reader::from_path("lemmatizer.csv") {
            for row in reader.deserialize::<LemmatizerRecord>().flatten() {
                self.insert_lemmatizer_form(row.form.as_str(), row.gloss_id)
                    .await?;
            }
        }
        Ok(())
    }

    async fn insert_pagebreak(&mut self, word_id: u32) -> Result<(), GlosserError> {
        //WAS REPLACE
        let query = r#"INSERT INTO latex_page_breaks (word_id) VALUES ($1);"#;
        let _ = sqlx::query(query)
            .bind(i32::try_from(word_id).unwrap())
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;
        /*
            self.update_log_trx(
                UpdateType::AddPageBreak,
                Some(word_id.into()),
                None,
                Some(course_id.into()),
                format!(
                    "Add page break on word ({}) in course ({})",
                    word_id, course_id
                )
                .as_str(),
                info,
            )
            .await?;
        */
        Ok(())
    }

    async fn delete_pagebreak(&mut self, word_id: u32) -> Result<(), GlosserError> {
        let query = r#"DELETE FROM latex_page_breaks WHERE word_id = $1;"#;
        let _ = sqlx::query(query)
            .bind(i32::try_from(word_id).unwrap())
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn insert_lemmatizer_form(
        &mut self,
        form: &str,
        gloss_id: u32,
    ) -> Result<(), GlosserError> {
        //WAS REPLACE
        let query = r#"INSERT INTO lemmatizer (form, gloss_id) VALUES ($1, $2);"#;
        let _ = sqlx::query(query)
            .bind(form)
            .bind(i32::try_from(gloss_id).unwrap())
            .execute(&mut *self.tx)
            .await;
        //ignore these errors since they usually happen during testing bc of gloss foreign keys missing

        Ok(())
    }

    // async fn get_courses(&mut self) -> Result<Vec<(u32, String)>, GlosserError> {

    //     let query = "SELECT course_id, name FROM courses;";
    //     match sqlx::query(query)
    //         .map(|rec: PgRow| LemmatizerRecord {
    //             form: rec.get("form"),
    //             gloss_id: rec.get("gloss_id"),
    //         })
    //         .fetch_all(&mut *self.tx)
    //         .await
    //         .map_err(map_sqlx_error)
    //     {
    //         Ok(res) => {
    //             for r in res {
    //                 lemmatizer.insert(r.form, r.gloss_id);
    //             }
    //             Ok(lemmatizer)
    //         }
    //         Err(e) => Err(e),
    //     }
    // }

    async fn get_lemmatizer(&mut self) -> Result<HashMap<String, u32>, GlosserError> {
        let mut lemmatizer = HashMap::new();

        let query = "SELECT form, gloss_id FROM lemmatizer;";
        match sqlx::query(query)
            .map(|rec: PgRow| LemmatizerRecord {
                form: rec.get("form"),
                gloss_id: u32::try_from(rec.get::<i32, _>("gloss_id")).unwrap(),
            })
            .fetch_all(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)
        {
            Ok(res) => {
                for r in res {
                    lemmatizer.insert(r.form, r.gloss_id);
                }
                Ok(lemmatizer)
            }
            Err(e) => Err(e),
        }
    }

    async fn get_hqvocab_column(
        &mut self,
        pos: &str,
        lower_unit: u32,
        unit: u32,
        sort: &str,
    ) -> Result<Vec<(String, u32, String)>, GlosserError> {
        let course_id = 1;
        let s = match sort {
            "alpha" => "sortalpha COLLATE PolytonicGreek ASC",
            _ => "unit, sortalpha COLLATE PolytonicGreek ASC",
        };
        let p = match pos {
            "noun" => "pos == 'noun'",
            "verb" => "pos == 'verb'",
            "adjective" => "pos == 'adjective'",
            _ => "pos != 'noun' AND pos != 'verb' AND pos != 'adjective'",
        };
        let query = format!(
            "SELECT lemma, unit, def FROM glosses \
            LEFT JOIN arrowed_words d ON (a.gloss_id = d.gloss_id AND d.course_id = {course_id}) \
            WHERE {} AND unit >= $1 AND unit <= $2 AND status = 1 ORDER BY {};",
            p, s
        );
        let words: Vec<(String, u32, String)> = sqlx::query(&query)
            .bind(i32::try_from(lower_unit).unwrap())
            .bind(i32::try_from(unit).unwrap())
            .map(|rec: PgRow| {
                (
                    rec.get("lemma"),
                    u32::try_from(rec.get::<i32, _>("unit")).unwrap(),
                    rec.get("def"),
                )
            })
            .fetch_all(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        Ok(words)
    }

    async fn arrow_word_trx(
        &mut self,
        course_id: u32,
        gloss_id: u32,
        word_id: u32,
        info: &ConnectionInfo,
    ) -> Result<(), GlosserError> {
        let query = "SELECT word_id \
    FROM arrowed_words \
    WHERE course_id = $1 AND gloss_id = $2;";
        let course_id = i32::try_from(course_id).unwrap();
        let gloss_id = i32::try_from(gloss_id).unwrap();
        let word_id = i32::try_from(word_id).unwrap();
        let user_id = i32::try_from(info.user_id).unwrap();

        let res: Result<(i32,), sqlx::Error> = sqlx::query_as(query)
            .bind(course_id)
            .bind(gloss_id)
            .fetch_one(&mut *self.tx)
            .await;

        let unwrapped_old_word_id: u32 = match res {
            Ok(old_word_id) => {
                if old_word_id.0 == 1 {
                    //don't allow arrow/unarrow h&q words which are set to word_id 1
                    return Err(GlosserError::UnknownError); //for now
                } else {
                    u32::try_from(old_word_id.0).unwrap()
                }
            }
            Err(sqlx::Error::RowNotFound) => {
                0 // 0 if not exist
            }
            Err(e) => {
                return Err(map_sqlx_error(e)); // return sql error
            }
        };

        //add previous arrow to history, if it was arrowed before
        let query = "INSERT INTO arrowed_words_history (course_id, gloss_id, word_id, updated, user_id, comment) \
        (SELECT course_id, gloss_id, word_id, updated, user_id, comment \
        FROM arrowed_words \
        WHERE course_id = $1 AND gloss_id = $2) \
        RETURNING history_id;";
        let history_id_rows = sqlx::query(query)
            .bind(course_id)
            .bind(gloss_id)
            .fetch_optional(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        let history_id: Option<i64> = if history_id_rows.is_some() {
            Some(history_id_rows.unwrap().get(0))
        } else {
            None
        };
        //.last_insert_rowid();

        //println!("rows: {}",r.rows_affected());

        //if no row existed to be inserted above, then the word was not arrowed before.  Insert new row into history to reflect this.
        //but this way we don't get to know when or by whom it was unarrowed? or do we???

        //$arrowedVal = ($_POST['setArrowedIDTo'] < 1) ? "NULL" : $_POST['setArrowedIDTo'] . "";

        if word_id > 0 {
            //first delete old arrowed location
            let query = "DELETE FROM arrowed_words WHERE course_id = $1 AND gloss_id = $2;";
            sqlx::query(query)
                .bind(course_id)
                .bind(gloss_id)
                .execute(&mut *self.tx)
                .await
                .map_err(map_sqlx_error)?;

            let query = "INSERT INTO arrowed_words (course_id, gloss_id, word_id, updated, user_id, comment) VALUES ($1, $2, $3, $4, $5, NULL);";
            sqlx::query(query)
                .bind(course_id)
                .bind(gloss_id)
                .bind(word_id)
                .bind(info.timestamp)
                .bind(user_id)
                //.bind(comment)
                .execute(&mut *self.tx)
                .await
                .map_err(map_sqlx_error)?;

            //jwm2
            self.update_log_trx(
                UpdateType::ArrowWord,
                Some(gloss_id.into()),
                history_id,
                Some(course_id.into()),
                format!(
                    "Arrow gloss ({}) to word ({}) from word ({}) in course ({})",
                    gloss_id, word_id, unwrapped_old_word_id, course_id
                )
                .as_str(),
                info,
            )
            .await?;
        } else {
            //delete row to remove arrow
            let query = "DELETE FROM arrowed_words WHERE course_id = $1 AND gloss_id = $2;";
            sqlx::query(query)
                .bind(course_id)
                .bind(gloss_id)
                .execute(&mut *self.tx)
                .await
                .map_err(map_sqlx_error)?;

            //add to history now, since can't later
            let query = "INSERT INTO arrowed_words_history (course_id, gloss_id, word_id, updated, user_id, comment) VALUES ($1, $2, NULL, $3, $4, NULL);";
            sqlx::query(query)
                .bind(course_id)
                .bind(gloss_id)
                .bind(info.timestamp)
                .bind(user_id)
                //.bind(comment)
                .execute(&mut *self.tx)
                .await
                .map_err(map_sqlx_error)?;

            //jwm2
            self.update_log_trx(
                UpdateType::UnarrowWord,
                Some(gloss_id.into()),
                history_id,
                Some(course_id.into()),
                format!(
                    "Unarrow gloss ({}) from word ({}) in course ({})",
                    gloss_id, unwrapped_old_word_id, course_id
                )
                .as_str(),
                info,
            )
            .await?;
        }
        Ok(())
    }

    //word_id is unique across courses, so we do not need to use course_id except for where the word is arrowed
    //to do: we need to send back updated counts both for the new gloss_id and for the old gloss_id, if one was set
    //to do: can we limit what is sent back by the text being viewed?
    async fn set_gloss_id(
        &mut self,
        course_id: u32,
        gloss_id: u32,
        word_id: u32,
        info: &ConnectionInfo,
    ) -> Result<Vec<SmallWord>, GlosserError> {
        //1a check if the word whose gloss is being changed is arrowed
        let query =
        "SELECT gloss_id FROM arrowed_words WHERE course_id = $1 AND gloss_id = $2 AND word_id = $3;";
        let arrowed_word_id: Result<(i32,), sqlx::Error> = sqlx::query_as(query)
            .bind(i32::try_from(course_id).unwrap())
            .bind(i32::try_from(gloss_id).unwrap())
            .bind(i32::try_from(word_id).unwrap())
            .fetch_one(&mut *self.tx)
            .await;

        //1b. unarrow word if it is arrowed
        match arrowed_word_id {
            Ok(_) => {
                //unarrow word if it is arrowed
                self.arrow_word_trx(course_id, gloss_id, 0 /*zero to unarrow*/, info)
                    .await?;
            }
            Err(sqlx::Error::RowNotFound) => {
                //continue if row not found
            }
            Err(e) => {
                //return error
                return Err(map_sqlx_error(e)); // return sql error
            }
        }

        //2a. save word row into history before updating gloss_id
        //or could have separate history table just for gloss_id changes
        //word_history_id, word_id, seq, text, word, gloss_id, type,
        let query = "INSERT INTO words_history \
        (word_id, seq, text_id, word, gloss_id, type, updated, updatedUser, isFlagged, note) \
        SELECT word_id, seq, text_id, word, gloss_id, type, updated, updatedUser, isFlagged, note FROM words \
        WHERE word_id = $1 RETURNING word_history_id;";
        let history_id_rows = sqlx::query(query)
            .bind(i32::try_from(word_id).unwrap())
            .fetch_all(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;
        let history_id: i32 = history_id_rows.last().unwrap().get(0);
        //.last_insert_rowid();

        //0. get old gloss_id before changing it so we can update its counts in step 3b
        let query = "SELECT gloss_id FROM words WHERE word_id = $1;";
        let old_gloss_id: (Option<i32>,) = sqlx::query_as(query)
            .bind(i32::try_from(word_id).unwrap())
            .fetch_one(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        //2b. update gloss_id
        let query = "UPDATE words SET gloss_id = $1 WHERE word_id = $2;";
        sqlx::query(query)
            .bind(i32::try_from(gloss_id).unwrap())
            .bind(i32::try_from(word_id).unwrap())
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        //this requests all the places this word shows up, so we can update them in the displayed page.
        //fix me: need to limit this by course_id
        //fix me: need to limit this to the assignment displayed on the page, else this could return huge number of rows for e.g. article/kai/etc
        let query = format!("WITH gloss_total AS (
            SELECT gloss_id, COUNT(gloss_id) AS total_count
            FROM words a2
            INNER JOIN course_x_text b2 ON a2.text_id = b2.text_id AND course_id = {course_id}
            GROUP BY gloss_id
        )
        SELECT B.gloss_id, B.lemma, B.pos, B.def, total_count, A.seq, A.word_id, \
    D.word_id as arrowedID, E.seq AS arrowedseq, A.isflagged, G.text_order,F.text_order AS arrowed_text_order, \
    COUNT(A.gloss_id) OVER (PARTITION BY A.gloss_id ORDER BY G.text_order,A.seq ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
        AS running_count
    FROM words A \
    LEFT JOIN glosses B ON A.gloss_id = B.gloss_id \
    LEFT JOIN arrowed_words D ON (A.gloss_id = D.gloss_id AND D.course_id = {course_id}) \
    LEFT JOIN words E ON E.word_id = D.word_id \
    LEFT JOIN course_x_text F ON (E.text_id = F.text_id AND F.course_id = {course_id}) \
    LEFT JOIN course_x_text G ON (A.text_id = G.text_id AND G.course_id = {course_id}) \
    LEFT JOIN gloss_total ON A.gloss_id = gloss_total.gloss_id \
    WHERE A.gloss_id = {gloss_id} AND A.type > -1 \
    ORDER BY G.text_order,A.seq \
    LIMIT 400;", gloss_id = gloss_id, course_id = course_id);

        let res: Vec<SmallWord> = sqlx::query(&query)
            .map(|rec: PgRow| SmallWord {
                wordid: u32::try_from(rec.get::<i32, _>("word_id")).unwrap(),
                hqid: u32::try_from(rec.get::<i32, _>("gloss_id")).unwrap(),
                lemma: rec.get("lemma"),
                pos: rec.get("pos"),
                def: rec.get("def"),
                runningcount: if rec.get::<Option<i64>, _>("running_count").is_some() {
                    Some(
                        u32::try_from(rec.get::<Option<i64>, _>("running_count").unwrap()).unwrap(),
                    )
                } else {
                    None
                },
                arrowed_seq: if rec.get::<Option<i32>, _>("arrowedseq").is_some() {
                    Some(u32::try_from(rec.get::<Option<i32>, _>("arrowedseq").unwrap()).unwrap())
                } else {
                    None
                },
                total: if rec.get::<Option<i64>, _>("total_count").is_some() {
                    Some(u32::try_from(rec.get::<Option<i64>, _>("total_count").unwrap()).unwrap())
                } else {
                    None
                },
                seq: u32::try_from(rec.get::<i32, _>("seq")).unwrap(),
                is_flagged: (rec.get::<i32, _>("isflagged") > 0),
                word_text_seq: u32::try_from(rec.get::<i32, _>("text_order")).unwrap(),
                arrowed_text_seq: if rec.get::<Option<i32>, _>("arrowed_text_order").is_some() {
                    Some(
                        u32::try_from(rec.get::<Option<i32>, _>("arrowed_text_order").unwrap())
                            .unwrap(),
                    )
                } else {
                    None
                },
            })
            .fetch_all(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        self.update_log_trx(
            UpdateType::SetGlossId,
            Some(word_id.into()),
            Some(history_id as i64),
            Some(course_id.into()),
            format!(
                "Set gloss for word ({}) from ({}) to ({}) in course ({})",
                word_id,
                old_gloss_id.0.unwrap_or(0),
                gloss_id,
                course_id
            )
            .as_str(),
            info,
        )
        .await?;

        Ok(res)
    }

    async fn add_text(
        &mut self,
        course_id: u32,
        text_name: &str,
        words: Vec<TextWord>,
        info: &ConnectionInfo,
    ) -> Result<(u64, i32), GlosserError> {
        let course_id = i32::try_from(course_id).unwrap();

        let query =
            "INSERT INTO texts (name, parent_id, display) VALUES ($1, NULL, 1) RETURNING text_id;";
        let text_id_row = sqlx::query(query)
            .bind(text_name)
            .fetch_one(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        let text_id: i32 = text_id_row.get(0);
        //.last_insert_rowid();

        //(word_id integer NOT NULL PRIMARY KEY AUTOINCREMENT, seq integer NOT NULL, text integer NOT NULL, section varchar (255) DEFAULT NULL, line varchar (255) DEFAULT NULL, word varchar (255) NOT NULL, gloss_id integer DEFAULT NULL REFERENCES glosses (gloss_id), lemma1 varchar (255) NOT NULL, lemma2 varchar (255) NOT NULL, o varchar (255) NOT NULL, runningcount integer NOT NULL, type integer DEFAULT NULL,
        //arrow integer NOT NULL DEFAULT 0, flagged integer NOT NULL DEFAULT 0, updated timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP,
        //updatedUserAgent varchar (255) NOT NULL DEFAULT '', updatedIP varchar (255) NOT NULL DEFAULT '', updatedUser varchar (255) NOT NULL DEFAULT '', isFlagged integer NOT NULL DEFAULT 0, note varchar (1024) NOT NULL DEFAULT '')

        let mut seq: u32 = 1;

        let query = "INSERT INTO words (seq, text_id, word, gloss_id, \
        type, updated, updatedUser, isFlagged, note) \
        VALUES ($1, $2, $3, $4, $5, $6, $7, 0, '');";
        let mut count = 0;
        let mut gloss_ids: HashSet<u32> = HashSet::new();
        for w in words {
            let gloss_id = if w.gloss_id.is_some() {
                Some(i32::try_from(w.gloss_id.unwrap()).unwrap())
            } else {
                None
            };
            let res = sqlx::query(query)
                .bind(i32::try_from(seq).unwrap())
                .bind(text_id)
                .bind(w.word)
                .bind(gloss_id)
                .bind(i32::try_from(w.word_type).unwrap())
                .bind(info.timestamp)
                .bind(i32::try_from(info.user_id).unwrap())
                .execute(&mut *self.tx)
                .await
                .map_err(map_sqlx_error)?;

            if let Some(g_id) = w.gloss_id {
                gloss_ids.insert(g_id);
            }

            seq += 1;

            let affected_rows = res.rows_affected();
            if affected_rows != 1 {
                return Err(GlosserError::UnknownError);
            }
            count += affected_rows;
        }

        let query = "SELECT MAX(text_order) FROM course_x_text WHERE course_id = $1;";
        let max_text_order: (Option<i32>,) = sqlx::query_as(query)
            .bind(course_id)
            .fetch_one(&mut *self.tx)
            .await
            .unwrap();
        //.map_err(map_sqlx_error)?;

        let max_text_order: i32 = if max_text_order.0.is_some() {
            max_text_order.0.unwrap() as i32
        } else {
            0
        };

        let query =
            "INSERT INTO course_x_text (course_id, text_id, text_order) VALUES ($1, $2, $3);";
        sqlx::query(query)
            .bind(course_id)
            .bind(text_id)
            .bind(max_text_order + 1)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        //jwm2
        self.update_log_trx(
            UpdateType::ImportText,
            Some(text_id.into()),
            None,
            None,
            format!("Imported {} words into text ({})", count, text_id).as_str(),
            info,
        )
        .await?;

        //println!("id: {}, count: {}", text_id, count);

        Ok((count, text_id))
    }

    async fn insert_gloss(
        &mut self,
        gloss: &str,
        pos: &str,
        def: &str,
        stripped_lemma: &str,
        note: &str,
        info: &ConnectionInfo,
    ) -> Result<(i64, u64), GlosserError> {
        let query = "INSERT INTO glosses (unit, lemma, sortalpha, \
        def, pos, note, updated, status, updatedUser) \
        VALUES (0, $1, $2, $3, $4, $5, $6, 1, $7) RETURNING gloss_id;";

        //double check that diacritics are stripped and word is lowercased; doesn't handle pua here yet
        let sl = stripped_lemma
            .nfd()
            .filter(|x| !unicode_normalization::char::is_combining_mark(*x))
            .collect::<String>()
            .to_lowercase();

        let res = sqlx::query(query)
            .bind(gloss)
            .bind(sl)
            .bind(def)
            .bind(pos)
            .bind(note)
            .bind(info.timestamp)
            .bind(i32::try_from(info.user_id).unwrap())
            .fetch_one(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        let new_gloss_id: i32 = res.get(0);
        //.last_insert_rowid();

        self.update_log_trx(
            UpdateType::NewGloss,
            Some(new_gloss_id as i64),
            None,
            None,
            format!("Added gloss ({})", new_gloss_id).as_str(),
            info,
        )
        .await?;

        Ok((new_gloss_id as i64, 1))
    }

    async fn update_log_trx(
        &mut self,
        update_type: UpdateType,
        object_id: Option<i64>,
        history_id: Option<i64>,
        course_id: Option<i64>,
        update_desc: &str,
        info: &ConnectionInfo,
    ) -> Result<(), GlosserError> {
        let query = "INSERT INTO update_log \
        (update_type, object_id, history_id, course_id, update_desc, updated, user_id, ip, user_agent) \
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9);";
        sqlx::query(query)
            .bind(i32::try_from(update_type.value()).unwrap())
            .bind(object_id)
            .bind(history_id)
            .bind(course_id)
            .bind(update_desc)
            .bind(info.timestamp)
            .bind(i32::try_from(info.user_id).unwrap())
            .bind(&info.ip_address)
            .bind(&info.user_agent)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        Ok(())
    }

    async fn delete_gloss(
        &mut self,
        gloss_id: u32,
        info: &ConnectionInfo,
    ) -> Result<u64, GlosserError> {
        let query = "SELECT COUNT(*) FROM glosses a INNER JOIN words b ON a.gloss_id = b.gloss_id WHERE a.gloss_id = $1;";
        let count: (i32,) = sqlx::query_as(query)
            .bind(i32::try_from(gloss_id).unwrap())
            .fetch_one(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        if count.0 == 0 {
            //jwm2
            self.update_log_trx(
                UpdateType::DeleteGloss,
                Some(gloss_id.into()),
                Some(gloss_id.into()),
                None,
                format!("Deleted gloss ({})", gloss_id).as_str(),
                info,
            )
            .await?;

            let query = "UPDATE glosses SET status = 0 WHERE gloss_id = $1;";
            let res = sqlx::query(query)
                .bind(i32::try_from(gloss_id).unwrap())
                .execute(&mut *self.tx)
                .await
                .map_err(map_sqlx_error)?;

            Ok(res.rows_affected())
        } else {
            Err(GlosserError::UnknownError) //for now
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn update_gloss(
        &mut self,
        gloss_id: u32,
        gloss: &str,
        pos: &str,
        def: &str,
        stripped_gloss: &str,
        note: &str,
        info: &ConnectionInfo,
    ) -> Result<u64, GlosserError> {
        let query = "INSERT INTO glosses_history \
        (gloss_id, unit, lemma, sortalpha, def, pos, note, updated, status, updatedUser) \
        SELECT * FROM glosses WHERE gloss_id = $1 RETURNING gloss_history_id;";
        let history_id_row = sqlx::query(query)
            .bind(i32::try_from(gloss_id).unwrap())
            .fetch_one(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        let history_id: i32 = history_id_row.get(0);

        //.last_insert_rowid();

        //let _ = update_log_trx(&mut tx, UpdateType::ArrowWord, "Arrowed word x from y to z.", timestamp, user_id, updated_ip, user_agent).await?;
        //let _ = update_log_trx(&mut tx, UpdateType::SetGlossId, "Change gloss for x from y to z.", timestamp, user_id, updated_ip, user_agent).await?;
        //jwm2
        self.update_log_trx(
            UpdateType::EditGloss,
            Some(gloss_id.into()),
            Some(history_id.into()),
            None,
            format!("Edited gloss ({})", gloss_id).as_str(),
            info,
        )
        .await?;
        //let _ = update_log_trx(&mut tx, UpdateType::NewGloss, "New gloss x.", timestamp, user_id, updated_ip, user_agent).await?;

        //CREATE TABLE IF NOT EXISTS update_log (update_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, update_type INTEGER REFERENCES update_types(update_type_id), object_id INTEGER, history_id INTEGER, course_id INTEGER, update_desc TEXT, comment TEXT, updated INTEGER NOT NULL, user_id INTEGER REFERENCES users(user_id), ip TEXT, user_agent TEXT );

        //double check that diacritics are stripped and word is lowercased; doesn't handle pua here yet
        let sl = stripped_gloss
            .nfd()
            .filter(|x| !unicode_normalization::char::is_combining_mark(*x))
            .collect::<String>()
            .to_lowercase();

        let query = "UPDATE glosses SET \
        lemma = $1, \
        sortalpha = $2, \
        def = $3, \
        pos = $4, \
        note = $5, \
        updated = $6, \
        updatedUser = $7 \
        WHERE gloss_id = $8;";

        let res = sqlx::query(query)
            .bind(gloss)
            .bind(sl)
            .bind(def)
            .bind(pos)
            .bind(note)
            .bind(info.timestamp)
            .bind(i32::try_from(info.user_id).unwrap())
            .bind(i32::try_from(gloss_id).unwrap())
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        Ok(res.rows_affected())
    }

    /*
    async fn fix_assignments(pool: &SqlitePool) -> Result<(), GlosserError> {
    let mut tx = pool.begin().await?;
    /*
    INSERT INTO texts (text_id, name, parent_id, display) (SELECT NULL,title,6,1 from assignments where id=28;

    update words set text=129
    where seq >= (select seq from words where word_id=22463)
    and seq <= (select seq from words where word_id =23069);
    */
    let mut text_order:u32 = 35;

    let query = "SELECT title,start,end,id FROM assignments WHERE id > 27 and id < 116;";
    let assignments:Vec<(String,u32,u32,u32,)> = sqlx::query_as(query)
    .fetch_all(&mut *tx).await?;

    for assignment in assignments {

        let parent_id = match assignment.3 {
        27..=69 => 6,
        70..=95 => 7,
        _ => 8
        };

        let query = "INSERT INTO texts (text_id, name, parent_id, display) VALUES (NULL,?,?,1); ";
        let text_id = sqlx::query(query)
        .bind(assignment.0) //title
        .bind(parent_id)
        .execute(&mut *tx).await?
        .last_insert_rowid();

        if assignment.1 > 0 && assignment.2 > 0 {
        let query = "update words set text=? \
        where seq >= (select seq from words where word_id=?) \
        and seq <= (select seq from words where word_id =?);";
        sqlx::query(query)
        .bind(text_id)
        .bind(assignment.1) //start
        .bind(assignment.2) //end
        .execute(&mut *tx).await?;
        }
        let query = "INSERT INTO course_x_text (course_id, text_id, text_order) VALUES (1,?,?); ";
        sqlx::query(query)
        .bind(text_id)
        .bind(text_order)
        .execute(&mut *tx).await?;

        text_order += 1;

        if assignment.3 == 70 || assignment.3 == 97 {
        text_order += 1;
        }
    }

    tx.commit().await?;

    Ok(())
    }
    */
    /*
    async fn get_parent_text_id(pool: &SqlitePool, text_id:u32) -> Result<Option<u32>, GlosserError> {
    let query = "SELECT parent_id FROM texts WHERE text_id = ?;";
    let rec: (Option<u32>,) = sqlx::query_as(query)
    .bind(text_id)
    .fetch_one(pool)
    .await?;

        Ok(rec.0)
    }
    */

    async fn get_words_for_export(
        &mut self,
        text_id: u32,
        course_id: u32,
    ) -> Result<Vec<WordRow>, GlosserError> {
        let query = format!("SELECT a.word_id, a.word, a.type, b.lemma, b.def, b.sortalpha, b.unit, b.pos, d.word_id as arrowedid, \
        b.gloss_id, a.seq, e.seq AS arrowedseq, \
        a.isFlagged, g.text_order, f.text_order AS arrowed_text_order, c.word_id as page_break, h.entry AS appcrit_entry \
        FROM words a \
        LEFT JOIN glosses b ON a.gloss_id = b.gloss_id \
        LEFT JOIN latex_page_breaks c ON a.word_id = c.word_id \
        LEFT JOIN arrowed_words d ON (a.gloss_id = d.gloss_id AND d.course_id = {course_id}) \
        LEFT JOIN words e ON e.word_id = d.word_id \
        LEFT JOIN course_x_text f ON (e.text_id = f.text_id AND f.course_id = {course_id}) \
        LEFT JOIN course_x_text g ON ({text_id} = g.text_id AND g.course_id = {course_id}) \
        LEFT JOIN appCrit h on h.word_id = A.word_id \
        WHERE a.text_id = {text_id} AND a.type > -1 \
        ORDER BY a.seq \
        LIMIT 550000;", text_id = text_id, course_id = course_id);

        let res: Result<Vec<WordRow>, GlosserError> = sqlx::query(&query)
            .map(|rec: PgRow| WordRow {
                wordid: u32::try_from(rec.get::<i32, _>("word_id")).unwrap(),
                word: rec.get("word"),
                word_type: u8::try_from(rec.get::<i32, _>("type")).unwrap(),
                lemma: rec.get("lemma"),
                def: rec.get("def"),
                unit: if rec.get::<Option<i32>, _>("unit").is_some() {
                    Some(u8::try_from(rec.get::<Option<i32>, _>("unit").unwrap()).unwrap())
                } else {
                    None
                },
                pos: rec.get("pos"),
                arrowed_id: if rec.get::<Option<i32>, _>("arrowedid").is_some() {
                    Some(u32::try_from(rec.get::<Option<i32>, _>("arrowedid").unwrap()).unwrap())
                } else {
                    None
                },
                hqid: if rec.get::<Option<i32>, _>("gloss_id").is_some() {
                    Some(u32::try_from(rec.get::<Option<i32>, _>("gloss_id").unwrap()).unwrap())
                } else {
                    None
                },
                seq: u32::try_from(rec.get::<i32, _>("seq")).unwrap(),
                arrowed_seq: if rec.get::<Option<i32>, _>("arrowedseq").is_some() {
                    Some(u32::try_from(rec.get::<Option<i32>, _>("arrowedseq").unwrap()).unwrap())
                } else {
                    None
                },
                freq: None,
                runningcount: None,
                is_flagged: rec.get::<i32, _>("isflagged") > 0,
                word_text_seq: u32::try_from(rec.get::<i32, _>("text_order")).unwrap(),
                arrowed_text_seq: if rec.get::<Option<i32>, _>("arrowed_text_order").is_some() {
                    Some(
                        u32::try_from(rec.get::<Option<i32>, _>("arrowed_text_order").unwrap())
                            .unwrap(),
                    )
                } else {
                    None
                },
                sort_alpha: rec.get("sortalpha"),
                last_word_of_page: rec.get::<Option<i32>, &str>("page_break").is_some(),
                app_crit: rec.get("appcrit_entry"),
            })
            .fetch_all(&mut *self.tx)
            .await
            .map_err(map_sqlx_error);

        res
    }

    /*
    *update get_words to not look for parent_id (just use text_id)
    update db:
        add each text/assignment to course_x_text table with a text_order
        add rest of ULG to text table select by seq between update text_id

    check for session user_id for each function, redirect to login if not set

    add tests
    add basic lemmatising to import for non-declined words
    add arrows to change order
    */

    async fn get_words(
        &mut self,
        text_id: u32,
        course_id: u32,
    ) -> Result<Vec<WordRow>, GlosserError> {
        let query = format!("WITH gloss_basis AS (
            SELECT gloss_id, COUNT(gloss_id) AS running_basis
            FROM words a1
            INNER JOIN course_x_text b1 ON a1.text_id = b1.text_id AND course_id = {course_id}
            WHERE text_order < (SELECT text_order FROM course_x_text WHERE course_id = {course_id} AND text_id = {text_id})
            GROUP BY gloss_id
        ),
        gloss_total AS (
            SELECT gloss_id, COUNT(gloss_id) AS total_count
            FROM words a2
            INNER JOIN course_x_text b2 ON a2.text_id = b2.text_id AND course_id = {course_id}
            GROUP BY gloss_id
        )
        SELECT a.word_id, a.word, a.type, b.lemma AS lemma, b.def, b.unit, b.pos, b.sortalpha, d.word_id as arrowedid, b.gloss_id, a.seq, e.seq AS arrowedseq,
        a.isFlagged, g.text_order, f.text_order AS arrowed_text_order, total_count, c.word_id as page_break,
        COUNT(a.gloss_id) OVER (PARTITION BY a.gloss_id ORDER BY a.seq ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
        + COALESCE(running_basis, 0) AS running_count
        FROM words a
        LEFT JOIN glosses b ON a.gloss_id = b.gloss_id
        LEFT JOIN latex_page_breaks c ON a.word_id = c.word_id
        LEFT JOIN arrowed_words d ON (a.gloss_id = d.gloss_id AND d.course_id = {course_id})
        LEFT JOIN words e ON e.word_id = d.word_id
        LEFT JOIN course_x_text f ON (e.text_id = f.text_id AND f.course_id = {course_id})
        LEFT JOIN course_x_text g ON ({text_id} = g.text_id AND g.course_id = {course_id})
        LEFT JOIN gloss_basis ON a.gloss_id = gloss_basis.gloss_id
        LEFT JOIN gloss_total ON a.gloss_id = gloss_total.gloss_id
        WHERE a.text_id = {text_id} AND a.type > -1
        ORDER BY a.seq
        LIMIT 55000;", text_id = text_id, course_id = course_id);

        let res: Result<Vec<WordRow>, GlosserError> = sqlx::query(&query)
            .map(|rec: PgRow| WordRow {
                wordid: u32::try_from(rec.get::<i32, _>("word_id")).unwrap(),
                word: rec.get("word"),
                word_type: u8::try_from(rec.get::<i32, _>("type")).unwrap(),
                lemma: rec.get("lemma"),
                def: rec.get("def"),
                unit: if rec.get::<Option<i32>, _>("unit").is_some() {
                    Some(u8::try_from(rec.get::<Option<i32>, _>("unit").unwrap()).unwrap())
                } else {
                    None
                },
                pos: rec.get("pos"),
                arrowed_id: if rec.get::<Option<i32>, _>("arrowedid").is_some() {
                    Some(u32::try_from(rec.get::<Option<i32>, _>("arrowedid").unwrap()).unwrap())
                } else {
                    None
                },
                hqid: if rec.get::<Option<i32>, _>("gloss_id").is_some() {
                    Some(u32::try_from(rec.get::<Option<i32>, _>("gloss_id").unwrap()).unwrap())
                } else {
                    None
                },
                seq: u32::try_from(rec.get::<i32, _>("seq")).unwrap(),
                arrowed_seq: if rec.get::<Option<i32>, _>("arrowedseq").is_some() {
                    Some(u32::try_from(rec.get::<Option<i32>, _>("arrowedseq").unwrap()).unwrap())
                } else {
                    None
                },
                freq: if rec.get::<Option<i64>, _>("total_count").is_some() {
                    Some(u32::try_from(rec.get::<Option<i64>, _>("total_count").unwrap()).unwrap())
                } else {
                    None
                },
                runningcount: if rec.get::<Option<i64>, _>("running_count").is_some() {
                    let rc = rec.get::<Option<i64>, _>("running_count").unwrap();
                    if rc < 1 {
                        None
                    } else {
                        Some(u32::try_from(rc).unwrap())
                    }
                } else {
                    None
                },
                is_flagged: rec.get::<i32, _>("isflagged") > 0,
                word_text_seq: u32::try_from(rec.get::<i32, _>("text_order")).unwrap(),
                arrowed_text_seq: if rec.get::<Option<i32>, _>("arrowed_text_order").is_some() {
                    Some(
                        u32::try_from(rec.get::<Option<i32>, _>("arrowed_text_order").unwrap())
                            .unwrap(),
                    )
                } else {
                    None
                },
                sort_alpha: rec.get("sortalpha"),
                last_word_of_page: rec.get::<Option<i32>, &str>("page_break").is_some(),
                app_crit: None,
            })
            .fetch_all(&mut *self.tx)
            .await
            .map_err(map_sqlx_error);

        res
    }

    //*insert assignments into texts
    //update text_id in words table based on assignment seq ranges

    //change get_words to use subtext id
    //order of assignments will be by id?  or word_seq?

    async fn get_sibling_texts(&mut self, text_id: u32) -> Result<Vec<u32>, GlosserError> {
        let query = "SELECT text_id \
    FROM texts \
    WHERE parent_id = (SELECT parent_id FROM texts WHERE text_id = $1) ORDER BY text_id";
        let res: Result<Vec<u32>, GlosserError> = sqlx::query(query)
            .bind(i32::try_from(text_id).unwrap())
            .map(|rec: PgRow| u32::try_from(rec.get::<i32, _>("text_id")).unwrap())
            .fetch_all(&mut *self.tx)
            .await
            .map_err(map_sqlx_error);
        res
    }

    async fn get_text_name(&mut self, text_id: u32) -> Result<String, GlosserError> {
        //let query = "SELECT id,title,wordcount FROM assignments ORDER BY id;";
        let query = "SELECT name \
    FROM texts \
    WHERE text_id = $1";
        let res: (String,) = sqlx::query_as(query)
            .bind(i32::try_from(text_id).unwrap())
            .fetch_one(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        Ok(res.0)
    }

    async fn get_text_title(&mut self, text_id: u32) -> Result<String, GlosserError> {
        //let query = "SELECT id,title,wordcount FROM assignments ORDER BY id;";
        let query = "SELECT title \
    FROM texts \
    WHERE text_id = $1";
        let res: (String,) = sqlx::query_as(query)
            .bind(i32::try_from(text_id).unwrap())
            .fetch_one(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        Ok(res.0)
    }

    // async fn update_counts_for_text_trx<'a, 'b>(
    //     tx: &'a mut sqlx::Transaction<'b, sqlx::Sqlite>,
    //     course_id: u32,
    //     text_id: u32,
    // ) -> Result<(), GlosserError> {

    //     let query = "SELECT hqid FROM words where text_id = ?;";
    //     let gloss_ids: Vec<(u32,)> = sqlx::query_as(&query)
    //         .bind(text_id)
    //         .fetch_all(tx)
    //         .await?;

    //     Ok(())
    // }
    /*

    create table containers (container_id integer primary key autoincrement not null, name text not null);
    insert into containers select null, name from texts where parent_id is null and text_id in (select parent_id from texts where parent_id is not null);
    update texts set display = 0 where parent_id is null and text_id in (select parent_id from texts where parent_id is not null);
    update texts set parent_id = parent_id - 1 where parent_id is not null;


    containers
        container_id
        name

    update parent_id to container id
    delete the parent texts


    container_x_text
        container_id,
        text_id,
        container_order,
        text_order,
    */

    // fix me
    /*
    async fn update_text_order_db(
        pool: &SqlitePool,
        course_id: u32,
        text_id: u32,
        step: i32,
    ) -> Result<(), GlosserError> {
        let mut tx = pool.begin().await?;


        // //has children? move children with parent
        // let query = "SELECT a.text_id,b.text_order FROM texts a \
        //             INNER JOIN course_x_text b ON a.text_id=b.text_id \
        //             WHERE parent_id = ? ORDER BY b.text_order;";
        // let children: Vec<(i32,i32,)> = sqlx::query_as(query)
        //     .bind(text_id)
        //     .fetch_all(pool).await?;

        // println!("children: {:?}", children);

        // let num_to_move = 1 + children.len();

        // if !children.empty() {

        // }

        // //has parent? only move among siblings
        // let query = "SELECT parent_id FROM texts WHERE text_id = ?;";
        // let has_parent: (Option<i32>,) = sqlx::query_as(query)
        //     .bind(text_id)
        //     .fetch_one(pool).await?;

        // println!("parent: {:?}", has_parent);

        // containers
        //     container_id
        //     name

        // containers_x_text
        //     container_id
        //     text_id

        // container moving down: container and its children get + 1, following text gets - 1 or following container + children get -1
        // container moving up: container and its children get - 1, text above gets + num_children + 1
        // text where moving is child and moving up?

        // container_id: move all items in container: select all items in container
        // text_id: move single item, if moving one text check if in container and limit to container bounds

        // text_seq_start, text_seq_end, step





        let query = "SELECT text_order FROM course_x_text WHERE course_id = ? AND text_id = ?;";
        let text_order: i32 = sqlx::query_scalar(query)
            .bind(course_id)
            .bind(text_id)
            .fetch_one(pool).await?;

        let query = "SELECT COUNT(*) FROM texts WHERE parent_id = (SELECT parent_id FROM texts WHERE text_id = ?);";
        let own_children: i32 = sqlx::query_scalar(query)
            .bind(text_id)
            .fetch_one(pool).await?;

        if step > 0 {
            let query = "SELECT COUNT(*) FROM texts WHERE parent_id = (SELECT text_id FROM course_x_text WHERE text_order = ? AND course_id = ?);";
        }
        else if step < 0 {
            let query = "SELECT COUNT(*) FROM texts WHERE parent_id = (SELECT parent_id FROM course_x_text WHERE text_order = ? AND course_id = ?);";
        }

        let target_children: i32 = sqlx::query_scalar(&query)
            .bind(text_order + step)
            .bind(course_id)
            .fetch_one(pool).await?;

        assert_eq!(2, own_children);
        assert_eq!(2, target_children);

        //let own_children = 2;
        //let target_children = 2;

        let query = "SELECT COUNT(*) FROM course_x_text WHERE course_id = ?;";
        let text_count_t: (i32,) = sqlx::query_as(query)
            .bind(course_id)
            .bind(text_id)
            .fetch_one(pool).await?;

        let text_count = text_count_t.0;

        if step == 0 || (text_order + step < 1 && step < 0) || (text_order + step > text_count && step > 0) {
            return Err(GlosserError::UnknownError); //at no where to move: abort
        }
        else if step > 0 { //move down: make room by moving other texts up/earlier in sequence
            let query = "UPDATE course_x_text SET text_order = text_order - 1 - ? \
                WHERE text_order > ? AND text_order < ? + ? + 1 + ? AND course_id = ?;";
            sqlx::query(query)
            .bind(own_children)
            .bind(text_order + own_children) //3
            .bind(text_order + own_children + target_children) //
            .bind(step)
            .bind(target_children)
            .bind(course_id)
            .execute(&mut tx)
            .await?;
        }
        else { //move up: make room by moving other texts down/later in sequence
            let query = "UPDATE course_x_text SET text_order = text_order + 1 + ? \
                WHERE text_order < ? AND text_order > ? - ? - 1 + ? AND course_id = ?;";
            sqlx::query(query)
            .bind(own_children)
            .bind(text_order)
            .bind(text_order)
            .bind(target_children)
            .bind(step) //step will be negative here
            .bind(course_id)
            .execute(&mut tx)
            .await?;
        }
        //set new text order
        let query = "UPDATE course_x_text SET text_order = text_order + ? + ? WHERE course_id = ? AND text_id IN (SELECT text_id FROM texts WHERE (parent_id = ?) OR (text_id = ?));";
        sqlx::query(query)
            .bind(step)
            .bind(target_children * if step > 0 { 1 } else { -1 })
            .bind(course_id)
            .bind(text_id)
            .bind(text_id)
            .execute(&mut tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }
    */

    async fn update_text_order_db(
        &mut self,
        course_id: u32,
        text_id: u32,
        step: i32,
    ) -> Result<(), GlosserError> {
        let text_id: i32 = i32::try_from(text_id).unwrap();
        let course_id: i32 = i32::try_from(course_id).unwrap();
        /*
        //has children? move children with parent
        let query = "SELECT a.text_id,b.text_order FROM texts a \
                    INNER JOIN course_x_text b ON a.text_id=b.text_id \
                    WHERE parent_id = ? ORDER BY b.text_order;";
        let children: Vec<(i32,i32,)> = sqlx::query_as(query)
            .bind(text_id)
            .fetch_all(pool).await?;
        println!("children: {:?}", children);
        let num_to_move = 1 + children.len();
        if !children.empty() {
        }
        //has parent? only move among siblings
        let query = "SELECT parent_id FROM texts WHERE text_id = ?;";
        let has_parent: (Option<i32>,) = sqlx::query_as(query)
            .bind(text_id)
            .fetch_one(pool).await?;
        println!("parent: {:?}", has_parent);
        containers
            container_id
            name
        containers_x_text
            container_id
            text_id
        container moving down: container and its children get + 1, following text gets - 1 or following container + children get -1
        container moving up: container and its children get - 1, text above gets + num_children + 1
        text where moving is child and moving up?
        */

        // get text order int
        let query = "SELECT text_order FROM course_x_text WHERE course_id = $1 AND text_id = $2;";
        let text_order: (i32,) = sqlx::query_as(query)
            .bind(course_id)
            .bind(text_id)
            .fetch_one(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        // get number of texts
        let query = "SELECT COUNT(*) FROM course_x_text WHERE course_id = $1;";
        let text_count: (i64,) = sqlx::query_as(query)
            .bind(course_id)
            .fetch_one(&mut *self.tx)
            .await
            .unwrap();
        //.map_err(map_sqlx_error)?;

        if step == 0
            || (text_order.0 + step < 1 && step < 0)
            || (text_order.0 + step > text_count.0.try_into().unwrap() && step > 0)
        {
            return Err(GlosserError::UnknownError); // no where to move: abort
        } else if step > 0 {
            //make room by moving other texts up/earlier in sequence
            let query = "UPDATE course_x_text SET text_order = text_order - 1 \
                WHERE text_order > $1 AND text_order < $2 + $3 + 1 AND course_id = $4;";
            sqlx::query(query)
                .bind(text_order.0)
                .bind(text_order.0)
                .bind(step)
                .bind(course_id)
                .execute(&mut *self.tx)
                .await
                .map_err(map_sqlx_error)?;
        } else {
            //make room by moving other texts down/later in sequence
            let query = "UPDATE course_x_text SET text_order = text_order + 1 \
                WHERE text_order < $1 AND text_order > $2 + $3 - 1 AND course_id = $4;";
            sqlx::query(query)
                .bind(text_order.0)
                .bind(text_order.0)
                .bind(step) //step will be negative here
                .bind(course_id)
                .execute(&mut *self.tx)
                .await
                .map_err(map_sqlx_error)?;
        }
        //set new text order
        let query =
            "UPDATE course_x_text SET text_order = text_order + $1 WHERE course_id = $2 AND text_id = $3;";
        sqlx::query(query)
            .bind(step)
            .bind(course_id)
            .bind(text_id)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        Ok(())
    }

    async fn get_texts_db(&mut self, course_id: u32) -> Result<Vec<AssignmentRow>, GlosserError> {
        let query = "SELECT A.text_id, A.name, A.parent_id, B.course_id, C.name AS container \
        FROM texts A \
        INNER JOIN course_x_text B ON (A.text_id = B.text_id AND B.course_id = $1) \
        LEFT JOIN containers C ON A.parent_id = C.container_id \
        WHERE display != 0 \
        ORDER BY B.text_order, A.text_id;";
        let res: Result<Vec<AssignmentRow>, GlosserError> = sqlx::query(query)
            .bind(i32::try_from(course_id).unwrap())
            .map(|rec: PgRow| AssignmentRow {
                text_id: u32::try_from(rec.get::<i32, _>("text_id")).unwrap(),
                text: rec.get("name"),
                container_id: if rec.get::<Option<i32>, _>("parent_id").is_some() {
                    Some(u32::try_from(rec.get::<Option<i32>, _>("parent_id").unwrap()).unwrap())
                } else {
                    None
                },
                course_id: if rec.get::<Option<i32>, _>("course_id").is_some() {
                    Some(u32::try_from(rec.get::<Option<i32>, _>("course_id").unwrap()).unwrap())
                } else {
                    None
                },
                container: rec.get("container"),
            })
            .fetch_all(&mut *self.tx)
            .await
            .map_err(map_sqlx_error);

        res
    }
    /*
    async fn _get_titles(pool: &SqlitePool) -> Result<Vec<(String,u32)>, GlosserError> {
        let query = "SELECT id,title FROM titles ORDER BY title;";
        let res: Result<Vec<(String,u32)>, GlosserError> = sqlx::query(query)
        .map(|rec: PgRow| (rec.get("id"),rec.get("title")) )
        .fetch_all(pool)
        .await;

        res
    }
    */
    async fn get_text_id_for_word_id(&mut self, word_id: u32) -> Result<u32, GlosserError> {
        let word_id = i32::try_from(word_id).unwrap();

        let query = "SELECT text_id FROM words WHERE word_id = $1;";
        let rec: (i32,) = sqlx::query_as(query)
            .bind(word_id)
            .fetch_one(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        Ok(rec.0 as u32)
    }
    /*
    async fn old_get_text_id_for_word_id(pool: &SqlitePool, word_id:u32) -> Result<u32, GlosserError> {
    let query = "SELECT A.id FROM assignments A INNER JOIN words B ON A.start = B.word_id INNER JOIN words C ON A.end = C.word_id WHERE B.seq <= (SELECT seq FROM words WHERE word_id = ?) AND C.seq >= (SELECT seq FROM words WHERE word_id = ?) LIMIT 1;";

    let rec: (u32,) = sqlx::query_as(query)
    .bind(word_id)
    .bind(word_id)
    .fetch_one(pool)
    .await?;

    Ok(rec.0)
    }
    */

    /*
    async fn get_start_end(pool: &SqlitePool, text_id:u32) -> Result<(u32,u32), GlosserError> {
    let query = "SELECT b.seq, c.seq FROM assignments a INNER JOIN words b ON a.start = b.word_id INNER JOIN words c ON a.end = c.word_id WHERE a.id = ?;";

    let rec: (u32,u32) = sqlx::query_as(query)
    .bind(text_id)
    .fetch_one(pool)
    .await?;

    Ok(rec)
    }
    */

    async fn get_glossdb(&mut self, gloss_id: u32) -> Result<GlossEntry, GlosserError> {
        let query = "SELECT gloss_id, lemma, pos, def, note FROM glosses WHERE gloss_id = $1;";

        sqlx::query(query)
            .bind(i32::try_from(gloss_id).unwrap())
            .map(|rec: PgRow| GlossEntry {
                hqid: u32::try_from(rec.get::<i32, _>("gloss_id")).unwrap(),
                l: rec.get("lemma"),
                pos: rec.get("pos"),
                g: rec.get("def"),
                n: rec.get("note"),
            })
            .fetch_one(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)
    }

    //SELECT c.name, a.word_id, a.word, d.word_id as arrowed FROM words a INNER JOIN course_x_text b ON (a.text = b.text_id AND b.course_id = 1) INNER JOIN texts c ON a.text = c.text_id LEFT JOIN arrowed_words d ON (d.course_id=1 AND d.gloss_id=564 AND d.word_id = a.word_id) WHERE a.gloss_id = 564 ORDER BY b.text_order, a.seq LIMIT 20000;

    async fn get_gloss_occurrences(
        &mut self,
        course_id: u32,
        gloss_id: u32,
    ) -> Result<Vec<GlossOccurrence>, GlosserError> {
        let query = "SELECT c.name, a.word_id, a.word, d.word_id as arrowed, e.unit, e.lemma \
        FROM words a \
        INNER JOIN course_x_text b ON (a.text_id = b.text_id AND b.course_id = $1) \
        INNER JOIN texts c ON a.text_id = c.text_id \
        INNER JOIN glosses e ON e.gloss_id = a.gloss_id \
        LEFT JOIN arrowed_words d ON (d.course_id = $2 AND d.gloss_id = $3 AND d.word_id = a.word_id) \
        WHERE a.gloss_id = $4 \
        ORDER BY b.text_order, a.seq \
        LIMIT 2000;"
            .to_string();

        let mut res: Vec<GlossOccurrence> = sqlx::query(&query)
            .bind(i32::try_from(course_id).unwrap())
            .bind(i32::try_from(course_id).unwrap())
            .bind(i32::try_from(gloss_id).unwrap())
            .bind(i32::try_from(gloss_id).unwrap())
            .map(|rec: PgRow| GlossOccurrence {
                name: rec.get("name"),
                word_id: u32::try_from(rec.get::<i32, _>("word_id")).unwrap(),
                word: rec.get("word"),
                arrowed: if rec.get::<Option<i32>, _>("arrowed").is_some() {
                    Some(u32::try_from(rec.get::<Option<i32>, _>("arrowed").unwrap()).unwrap())
                } else {
                    None
                },
                unit: if rec.get::<Option<i32>, _>("unit").is_some() {
                    Some(u32::try_from(rec.get::<Option<i32>, _>("unit").unwrap()).unwrap())
                } else {
                    None
                },
                lemma: rec.get("lemma"),
            })
            .fetch_all(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        if !res.is_empty()
            && res[0].unit.is_some()
            && res[0].unit.unwrap() > 0
            && res[0].unit.unwrap() < 21
        {
            res.insert(
                0,
                GlossOccurrence {
                    name: format!("H&Q Unit {}", res[0].unit.unwrap()),
                    word_id: 1,
                    word: res[0].lemma.to_owned(),
                    arrowed: Some(1),
                    unit: res[0].unit,
                    lemma: res[0].lemma.to_owned(),
                },
            )
        }

        Ok(res)
    }

    async fn get_update_log(
        &mut self,
        _course_id: u32,
    ) -> Result<Vec<AssignmentTree>, GlosserError> {
        let query = "SELECT 'temp updated' as timestamp, a.update_id, \
        b.update_type, c.initials, update_desc \
        FROM update_log a \
        INNER JOIN update_types b ON a.update_type = b.update_type_id \
        INNER JOIN users c ON a.user_id = c.user_id \
        ORDER BY updated DESC \
        LIMIT 20000;"
            .to_string();

        let res: Vec<(String, String, String, String, u32)> = sqlx::query(&query)
            .map(|rec: PgRow| {
                (
                    rec.get("timestamp"),
                    rec.get("update_type"),
                    rec.get("initials"),
                    rec.get("update_desc"),
                    u32::try_from(rec.get::<i32, _>("update_id")).unwrap(),
                )
            })
            .fetch_all(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        let mut rows: Vec<AssignmentTree> = vec![];
        for r in &res {
            rows.push(AssignmentTree {
                i: r.4,
                col: vec![format!("{} - {} {}", r.0.clone(), r.2.clone(), r.3.clone(),)],
                h: false,
                c: vec![],
            });
        }

        Ok(rows)
    }

    async fn get_before(
        &mut self,
        searchprefix: &str,
        page: i32,
        limit: u32,
        course_id: u32,
    ) -> Result<Vec<(String, u32, String, u32)>, GlosserError> {
        let query = format!("WITH gloss_total AS (
            SELECT gloss_id, COUNT(gloss_id) AS total_count
            FROM words a2
            INNER JOIN course_x_text b2 ON a2.text_id = b2.text_id AND course_id = {}
            GROUP BY gloss_id
        )
        SELECT a.gloss_id, a.lemma, a.def, COALESCE(b.total_count, 0) AS total_count FROM glosses a LEFT JOIN gloss_total b ON a.gloss_id = b.gloss_id WHERE a.sortalpha < '{}' AND status > 0 AND pos != 'gloss' ORDER BY a.sortalpha DESC LIMIT {} OFFSET {};", course_id, searchprefix, limit, -page * limit as i32);
        let res: Result<Vec<(String, u32, String, u32)>, GlosserError> = sqlx::query(&query)
            .map(|rec: PgRow| {
                (
                    rec.get("lemma"),
                    u32::try_from(rec.get::<i32, _>("gloss_id")).unwrap(),
                    rec.get("def"),
                    u32::try_from(rec.get::<i64, _>("total_count")).unwrap(),
                )
            })
            .fetch_all(&mut *self.tx)
            .await
            .map_err(map_sqlx_error);

        res
    }

    async fn get_equal_and_after(
        &mut self,
        searchprefix: &str,
        page: i32,
        limit: u32,
        course_id: u32,
    ) -> Result<Vec<(String, u32, String, u32)>, GlosserError> {
        let query = format!("WITH gloss_total AS (
            SELECT gloss_id, COUNT(gloss_id) AS total_count
            FROM words a2
            INNER JOIN course_x_text b2 ON a2.text_id = b2.text_id AND course_id = {}
            GROUP BY gloss_id
        )
        SELECT a.gloss_id, a.lemma, a.def, COALESCE(b.total_count, 0) AS total_count FROM glosses a LEFT JOIN gloss_total b ON a.gloss_id = b.gloss_id WHERE a.sortalpha >= '{}' AND status > 0 AND pos != 'gloss' ORDER BY a.sortalpha LIMIT {} OFFSET {};",
        course_id, searchprefix, limit, page * limit as i32);
        let res: Result<Vec<(String, u32, String, u32)>, GlosserError> = sqlx::query(&query)
            .map(|rec: PgRow| {
                (
                    rec.get("lemma"),
                    u32::try_from(rec.get::<i32, _>("gloss_id")).unwrap(),
                    rec.get("def"),
                    u32::try_from(rec.get::<i64, _>("total_count")).unwrap(),
                )
            })
            .fetch_all(&mut *self.tx)
            .await
            .map_err(map_sqlx_error);

        res
    }

    #[allow(dead_code)]
    async fn create_user(
        &mut self,
        name: &str,
        initials: &str,
        user_type: u32,
        password: Secret<String>,
        email: &str,
    ) -> Result<i64, GlosserError> {
        let query = r#"INSERT INTO users (name, initials, user_type, password, email) VALUES ($1, $2, $3, $4, $5) RETURNING user_id;"#;
        let user_id_row = sqlx::query(query)
            .bind(name)
            .bind(initials)
            .bind(i32::try_from(user_type).unwrap())
            .bind(password.expose_secret())
            .bind(email)
            .fetch_one(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;
        let user_id: i32 = user_id_row.get(0);
        //.last_insert_rowid();
        Ok(user_id as i64)
    }

    async fn get_credentials(
        &mut self,
        username: &str,
    ) -> Result<Option<(u32, Secret<String>)>, GlosserError> {
        let row = sqlx::query(
            r#"
            SELECT user_id, password
            FROM users
            WHERE initials = $1
            "#,
        )
        .bind(username)
        .map(|row: PgRow| {
            (
                u32::try_from(row.get::<i32, _>("user_id")).unwrap(),
                Secret::new(row.get("password")),
            )
        })
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(map_sqlx_error)?;

        Ok(row)
    }

    async fn insert_word(
        &mut self,
        before_word_id: u32,
        word_type: u32,
        word: &str,
    ) -> Result<i64, GlosserError> {
        let query = r#"UPDATE words set seq = seq + 1 WHERE text_id > 269 AND text_id < 296 AND seq >= (SELECT seq FROM words WHERE word_id = $1);"#;
        let _res = sqlx::query(query)
            .bind(i32::try_from(before_word_id).unwrap())
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        //do not insert value for updated so it gets default timestamp, inserting null to it does not set the timestamp
        let query2 = r#"INSERT INTO words (word_id, seq, text_id, word, gloss_id, type, updatedUser, isFlagged, note)
            VALUES (NULL,
                (SELECT seq - 1 FROM words WHERE word_id = $1),
                (SELECT text_id FROM words WHERE word_id = $2),
                $3, NULL, $4, '', 0, '');"#;
        let word_id_row = sqlx::query(query2)
            .bind(i32::try_from(before_word_id).unwrap())
            .bind(i32::try_from(before_word_id).unwrap())
            .bind(word) //''
            .bind(i32::try_from(word_type).unwrap()) //6
            .fetch_one(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;
        let word_id: i32 = word_id_row.get(0);
        //.last_insert_rowid();

        Ok(word_id as i64)
    }

    async fn create_db(&mut self) -> Result<(), GlosserError> {
        let query = r#"CREATE TABLE IF NOT EXISTS users (user_id SERIAL PRIMARY KEY, name TEXT NOT NULL UNIQUE, initials TEXT NOT NULL UNIQUE, user_type INTEGER NOT NULL, password TEXT NOT NULL, email TEXT);"#;
        let _res = sqlx::query(query)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;
        let query = r#"CREATE TABLE IF NOT EXISTS courses (course_id SERIAL PRIMARY KEY, name TEXT NOT NULL);"#;
        let _res = sqlx::query(query)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;
        let query = r#"CREATE TABLE IF NOT EXISTS texts (text_id SERIAL PRIMARY KEY, name TEXT NOT NULL, parent_id INTEGER references texts (text_id) DEFAULT NULL, display INTEGER DEFAULT 1, title TEXT NOT NULL DEFAULT '');"#;
        let _res = sqlx::query(query)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;
        let query = r#"CREATE TABLE IF NOT EXISTS course_x_text (course_id INTEGER NOT NULL REFERENCES courses (course_id), text_id INTEGER NOT NULL REFERENCES texts (text_id), text_order INTEGER NOT NULL, PRIMARY KEY (course_id, text_id));"#;
        let _res = sqlx::query(query)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;
        let query = r#"CREATE TABLE IF NOT EXISTS glosses (gloss_id SERIAL PRIMARY KEY, unit INTEGER NOT NULL, lemma TEXT NOT NULL, sortalpha TEXT NOT NULL DEFAULT '', def TEXT NOT NULL, pos TEXT NOT NULL, note TEXT NOT NULL, updated TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, status INTEGER NOT NULL DEFAULT 1, updatedUser TEXT NOT NULL DEFAULT '');"#;
        let _res = sqlx::query(query)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;
        let query = r#"CREATE TABLE IF NOT EXISTS words (word_id SERIAL PRIMARY KEY, seq INTEGER NOT NULL, text_id INTEGER NOT NULL, word TEXT NOT NULL, gloss_id INTEGER DEFAULT NULL REFERENCES glosses (gloss_id), type INTEGER DEFAULT NULL, updated TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, updatedUser TEXT NOT NULL DEFAULT '', isFlagged INTEGER NOT NULL DEFAULT 0, note TEXT NOT NULL DEFAULT '');"#;
        let _res = sqlx::query(query)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;
        let query = r#"CREATE TABLE IF NOT EXISTS arrowed_words (course_id INTEGER NOT NULL REFERENCES courses (course_id), gloss_id INTEGER NOT NULL REFERENCES glosses (gloss_id), word_id INTEGER NOT NULL REFERENCES words (word_id), updated INTEGER, user_id INTEGER REFERENCES users (user_id), comment TEXT, PRIMARY KEY (course_id, gloss_id, word_id));"#;
        let _res = sqlx::query(query)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;
        let query = r#"CREATE TABLE IF NOT EXISTS arrowed_words_history (history_id SERIAL PRIMARY KEY, course_id INTEGER NOT NULL REFERENCES courses (course_id), gloss_id INTEGER NOT NULL REFERENCES glosses (gloss_id), word_id INTEGER, updated INTEGER, user_id INTEGER REFERENCES users (user_id), comment TEXT);"#;
        let _res = sqlx::query(query)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;
        let query = r#"CREATE TABLE IF NOT EXISTS appcrit (word_id INTEGER NOT NULL, entry TEXT DEFAULT NULL, PRIMARY KEY (word_id));"#;
        let _res = sqlx::query(query)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;
        let query = r#"CREATE TABLE IF NOT EXISTS words_history (word_history_id SERIAL PRIMARY KEY, word_id INTEGER NOT NULL, seq INTEGER NOT NULL, text_id INTEGER NOT NULL, word TEXT NOT NULL, gloss_id INTEGER DEFAULT NULL REFERENCES glosses (gloss_id), type INTEGER DEFAULT NULL, updated TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, updatedUser TEXT NOT NULL DEFAULT '', isFlagged INTEGER NOT NULL DEFAULT 0, note TEXT NOT NULL DEFAULT '');"#;
        let _res = sqlx::query(query)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;
        let query = r#"CREATE TABLE IF NOT EXISTS glosses_history (gloss_history_id SERIAL PRIMARY KEY, gloss_id INTEGER NOT NULL, unit INTEGER NOT NULL, lemma TEXT NOT NULL, sortalpha TEXT NOT NULL DEFAULT '', def TEXT NOT NULL, pos TEXT NOT NULL, note TEXT NOT NULL, updated TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, status INTEGER NOT NULL DEFAULT 1, updatedUser TEXT NOT NULL DEFAULT '');"#;
        let _res = sqlx::query(query)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;
        let query = r#"CREATE TABLE IF NOT EXISTS update_types (update_type_id SERIAL PRIMARY KEY, update_type TEXT NOT NULL);"#;
        let _res = sqlx::query(query)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;
        let query = r#"CREATE TABLE IF NOT EXISTS update_log (update_id SERIAL PRIMARY KEY, update_type INTEGER REFERENCES update_types(update_type_id), object_id INTEGER, history_id INTEGER, course_id INTEGER, update_desc TEXT, comment TEXT, updated INTEGER NOT NULL, user_id INTEGER REFERENCES users(user_id), ip TEXT, user_agent TEXT );"#;
        let _res = sqlx::query(query)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;
        let query = r#"CREATE TABLE IF NOT EXISTS latex_page_breaks (word_id INTEGER NOT NULL UNIQUE REFERENCES words(word_id));"#;
        let _res = sqlx::query(query)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;
        let query = r#"CREATE TABLE IF NOT EXISTS containers (container_id SERIAL PRIMARY KEY, name TEXT NOT NULL);"#;
        let _res = sqlx::query(query)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;
        let query = r#"CREATE TABLE IF NOT EXISTS lemmatizer (form TEXT PRIMARY KEY NOT NULL, gloss_id INTEGER NOT NULL REFERENCES glosses(gloss_id));"#;
        let _res = sqlx::query(query)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;
        let query = r#"CREATE INDEX IF NOT EXISTS idx_hqvocab_lemma ON glosses (lemma);"#;
        let _res = sqlx::query(query)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;
        let query = r#"CREATE INDEX IF NOT EXISTS idx_hqvocab_sortalpha ON glosses (sortalpha);"#;
        let _res = sqlx::query(query)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;
        let query = r#"CREATE INDEX IF NOT EXISTS idx_hqvocab_updated ON glosses (updated);"#;
        let _res = sqlx::query(query)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;
        let query = r#"CREATE INDEX IF NOT EXISTS arrowed_words_history_idx ON arrowed_words (course_id, gloss_id);"#;
        let _res = sqlx::query(query)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;
        let query = r#"CREATE INDEX IF NOT EXISTS idx_gkvocabdb_lemmaid ON words (gloss_id);"#;
        let _res = sqlx::query(query)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;
        let query = r#"CREATE INDEX IF NOT EXISTS idx_gkvocabdb_seq ON words (seq);"#;
        let _res = sqlx::query(query)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;
        let query = r#"CREATE INDEX IF NOT EXISTS idx_gkvocabdb_text ON words (text_id);"#;

        let _res = sqlx::query(query)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        //create default course
        let query = r#"INSERT INTO courses VALUES (1, 'Greek') ON CONFLICT DO NOTHING;"#;
        sqlx::query(query)
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx_error)?;

        //insert update types
        let query = r#"INSERT INTO update_types VALUES ($1, $2) ON CONFLICT DO NOTHING;"#;
        let update_types = vec![
            (1, "Arrow word"),
            (2, "Unarrow word"),
            (3, "New gloss"),
            (4, "Edit gloss"),
            (5, "Set gloss"),
            (6, "Import text"),
            (7, "Delete gloss"),
        ];

        for t in update_types {
            sqlx::query(query)
                .bind(t.0)
                .bind(t.1)
                .execute(&mut *self.tx)
                .await
                .map_err(map_sqlx_error)?;
        }
        Ok(())
    }
}
