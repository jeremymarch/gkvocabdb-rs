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

use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteRow;
use sqlx::{FromRow, Row, SqlitePool};
use std::collections::HashSet;
use crate::ConnectionInfo;
use crate::GlossOccurrence;
use unicode_normalization::UnicodeNormalization;

/*
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum PhilologusWords {
    GreekDefs { seq: u32, def: String },
}

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct DefRow {
    pub word: String,
    pub sortword: String,
    pub def: String,
    pub seq: u32
}
*/

#[derive(Debug, Serialize, Deserialize, Clone, FromRow, Eq, PartialEq)]
pub struct WordRow {
    #[serde(rename(serialize = "i"), rename(deserialize = "i"))]
    pub wordid: u32,
    #[serde(rename(serialize = "w"), rename(deserialize = "w"))]
    pub word: String,
    #[serde(rename(serialize = "t"), rename(deserialize = "t"))]
    pub word_type: u8,
    #[serde(rename(serialize = "l"), rename(deserialize = "l"))]
    pub lemma: String,
    #[serde(rename(serialize = "l1"), rename(deserialize = "l1"))]
    pub lemma1: String,
    pub def: String,
    #[serde(rename(serialize = "u"), rename(deserialize = "u"))]
    pub unit: u8,
    pub pos: String,
    #[serde(rename(serialize = "a"), rename(deserialize = "a"))]
    pub arrowed_id: Option<u32>,
    pub hqid: u32,
    #[serde(rename(serialize = "s"), rename(deserialize = "s"))]
    pub seq: u32,
    #[serde(rename(serialize = "s2"), rename(deserialize = "s2"))]
    pub arrowed_seq: Option<u32>,
    #[serde(rename(serialize = "c"), rename(deserialize = "c"))]
    pub freq: u32,
    #[serde(rename(serialize = "rc"), rename(deserialize = "rc"))]
    pub runningcount: u32,
    #[serde(rename(serialize = "if"), rename(deserialize = "if"))]
    pub is_flagged: bool,
    pub word_text_seq: u32,
    pub arrowed_text_seq: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromRow, Eq, PartialEq)]
pub struct SmallWord {
    #[serde(rename(serialize = "i"))]
    pub wordid: u32,
    pub hqid: u32,
    #[serde(rename(serialize = "l"))]
    pub lemma: String,
    pub pos: String,
    #[serde(rename(serialize = "g"))]
    pub def: String,
    #[serde(rename(serialize = "rc"))]
    pub runningcount: Option<u32>,
    #[serde(rename(serialize = "ls"))]
    pub arrowed_seq: Option<u32>,
    #[serde(rename(serialize = "fr"))]
    pub total: Option<u32>,
    #[serde(rename(serialize = "ws"))]
    pub seq: u32,
    #[serde(rename(serialize = "if"))]
    pub is_flagged: bool,
    #[serde(rename(serialize = "wtseq"))]
    pub word_text_seq: u32,
    #[serde(rename(serialize = "atseq"))]
    pub arrowed_text_seq: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct AssignmentRow {
    pub id: u32,
    pub assignment: String,
    pub parent_id: Option<u32>,
    pub course_id: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct DefRow {
    pub word: String,
    pub sortword: String,
    pub def: String,
    pub seq: u32,
}

#[derive(Debug, Clone)]
pub struct TextWord {
    pub word: String,
    pub word_type: u32,
    pub gloss_id: Option<u32>,
}
/*
pub async fn get_seq_by_prefix(pool: &SqlitePool, table:&str, prefix:&str) -> Result<u32, sqlx::Error> {
  let query = format!("SELECT seq FROM {} WHERE sortalpha >= '{}' ORDER BY sortalpha LIMIT 1;", table, prefix);

  let rec:Result<(u32,), sqlx::Error> = sqlx::query_as(&query)
  .fetch_one(pool)
  .await;

  match rec {
      Ok(r) => Ok(r.0),
      Err(sqlx::Error::RowNotFound) => { //not found, return seq of last word
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlossEntry {
    pub hqid: u32,
    pub l: String,
    pub pos: String,
    pub g: String,
    pub n: String,
}

pub enum UpdateType {
    ArrowWord,
    UnarrowWord,
    NewGloss,
    EditGloss,
    SetGlossId,
    ImportText,
    DeleteGloss,
}

impl UpdateType {
    fn value(&self) -> u32 {
        match *self {
            UpdateType::ArrowWord => 1,
            UpdateType::UnarrowWord => 2,
            UpdateType::NewGloss => 3,
            UpdateType::EditGloss => 4,
            UpdateType::SetGlossId => 5,
            UpdateType::ImportText => 6,
            UpdateType::DeleteGloss => 7,
        }
    }
}
pub async fn get_hqvocab_column(pool: &SqlitePool, pos:&str, unit:u32, sort:&str) -> Result<Vec<(String,u32,String)>, sqlx::Error> {
    let s = match sort {
        "alpha" => "sortalpha COLLATE PolytonicGreek ASC",
        _ => "unit,sortalpha COLLATE PolytonicGreek ASC"
    };
    let p = match pos {
        "noun" => "pos == 'noun'",
        "verb" => "pos == 'verb'",
        "adjective" => "pos == 'adjective'",
        _ => "pos != 'noun' AND pos != 'verb' AND pos != 'adjective'",
    };
    let query = format!("SELECT lemma,unit,def FROM glosses where {} AND unit > 0 AND unit <= {} AND status=1 ORDER BY {};", p, unit, s);
    let words: Vec<(String,u32,String)> = sqlx::query_as(&query)
        .fetch_all(pool)
        .await?;

    Ok(words)
}

pub async fn arrow_word(
    pool: &SqlitePool,
    course_id: u32,
    gloss_id: u32,
    word_id: u32,
    info: &ConnectionInfo,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    arrow_word_trx(
        &mut tx, course_id, gloss_id, word_id, info,
    )
    .await?;

    tx.commit().await?;

    Ok(())
}

pub async fn arrow_word_trx<'a, 'b>(
    tx: &'a mut sqlx::Transaction<'b, sqlx::Sqlite>,
    course_id: u32,
    gloss_id: u32,
    word_id: u32,
    info: &ConnectionInfo,
) -> Result<(), sqlx::Error> {
    let query = "SELECT word_id \
  FROM arrowed_words \
  WHERE course_id = ? AND gloss_id = ?;";
    let old_word_id: Result<(u32,), sqlx::Error> = sqlx::query_as(query)
        .bind(course_id)
        .bind(gloss_id)
        .fetch_one(&mut *tx)
        .await;

    let unwrapped_old_word_id = old_word_id.unwrap_or((0,)).0; //0 if not exist

    if unwrapped_old_word_id == 1 { //don't allow arrow/unarrow h&q words which are set to word_id 1
      return Err(sqlx::Error::RowNotFound); //for now
    }

    //add previous arrow to history, if it was arrowed before
    let query = "INSERT INTO arrowed_words_history \
    SELECT NULL, course_id, gloss_id, word_id, updated, user_id, comment \
    FROM arrowed_words \
    WHERE course_id = ? AND gloss_id = ?;";
    let history_id = sqlx::query(query)
        .bind(course_id)
        .bind(gloss_id)
        .execute(&mut *tx)
        .await?
        .last_insert_rowid();

    //println!("rows: {}",r.rows_affected());

    //if no row existed to be inserted above, then the word was not arrowed before.  Insert new row into history to reflect this.
    //but this way we don't get to know when or by whom it was unarrowed? or do we???

    //$arrowedVal = ($_POST['setArrowedIDTo'] < 1) ? "NULL" : $_POST['setArrowedIDTo'] . "";

    if word_id > 0 {
        //first delete old arrowed location
        let query = "DELETE FROM arrowed_words WHERE course_id = ? AND gloss_id = ?;";
        sqlx::query(query)
            .bind(course_id)
            .bind(gloss_id)
            .execute(&mut *tx)
            .await?;

        let query = "INSERT INTO arrowed_words VALUES (?, ?, ?, ?, ?, NULL);";
        sqlx::query(query)
            .bind(course_id)
            .bind(gloss_id)
            .bind(word_id)
            .bind(info.timestamp)
            .bind(info.user_id)
            //.bind(comment)
            .execute(&mut *tx)
            .await?;

        update_log_trx(
            &mut *tx,
            UpdateType::ArrowWord,
            Some(gloss_id.into()),
            Some(history_id),
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
        let query = "DELETE FROM arrowed_words WHERE course_id = ? AND gloss_id = ?;";
        sqlx::query(query)
            .bind(course_id)
            .bind(gloss_id)
            .execute(&mut *tx)
            .await?;

        //add to history now, since can't later
        let query = "INSERT INTO arrowed_words_history VALUES (NULL, ?, ?, NULL, ?, ?, NULL);";
        sqlx::query(query)
            .bind(course_id)
            .bind(gloss_id)
            .bind(info.timestamp)
            .bind(info.user_id)
            //.bind(comment)
            .execute(&mut *tx)
            .await?;

        update_log_trx(
            &mut *tx,
            UpdateType::UnarrowWord,
            Some(gloss_id.into()),
            Some(history_id),
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
pub async fn set_gloss_id(
    pool: &SqlitePool,
    course_id: u32,
    gloss_id: u32,
    word_id: u32,
    info: &ConnectionInfo,
) -> Result<Vec<SmallWord>, sqlx::Error> {
    let mut tx = pool.begin().await?;

    //1a check if the word whose gloss is being changed is arrowed
    let query =
        "SELECT gloss_id FROM arrowed_words WHERE course_id = ? AND gloss_id = ? AND word_id = ?;";
    let arrowed_word_id: Result<(u32,), sqlx::Error> = sqlx::query_as(query)
        .bind(course_id)
        .bind(gloss_id)
        .bind(word_id)
        .fetch_one(&mut tx)
        .await;

    //1b. unarrow word if it is arrowed
    if arrowed_word_id.is_ok() {
        //r.rows_affected() < 1 {
        arrow_word_trx(
            &mut tx, course_id, gloss_id, 0, /*zero to unarrow*/
            info,
        )
        .await?;
    }

    //2a. save word row into history before updating gloss_id
    //or could have separate history table just for gloss_id changes
    let query = "INSERT INTO words_history SELECT NULL,* FROM words WHERE word_id = ?;";
    let history_id = sqlx::query(query)
        .bind(word_id)
        .execute(&mut tx)
        .await?
        .last_insert_rowid();

    //0. get old gloss_id before changing it so we can update its counts in step 3b
    let query = "SELECT gloss_id FROM words WHERE word_id = ?;";
    let old_gloss_id: (Option<u32>,) = sqlx::query_as(query)
        .bind(word_id)
        .fetch_one(&mut tx)
        .await?;

    //2b. update gloss_id
    let query = "UPDATE words SET gloss_id = ? WHERE word_id = ?;";
    sqlx::query(query)
        .bind(gloss_id)
        .bind(word_id)
        .execute(&mut tx)
        .await?;

    //3. update counts
    update_counts_for_gloss_id(&mut tx, course_id, gloss_id).await?;
    if old_gloss_id.0.is_some() {
        update_counts_for_gloss_id(&mut tx, course_id, old_gloss_id.0.unwrap()).await?;
    }

    //this requests all the places this word shows up, so we can update them in the displayed page.
    //fix me: need to limit this by course_id
    //fix me: need to limit this to the assignment displayed on the page, else this could return huge number of rows for e.g. article/kai/etc
    let query = format!("SELECT B.gloss_id, B.lemma, B.pos, B.def, I.total_count, A.seq, H.running_count, A.word_id, \
  D.word_id as arrowedID, E.seq AS arrowedSeq, A.isFlagged, G.text_order,F.text_order AS arrowed_text_order \
  FROM words A \
  LEFT JOIN glosses B ON A.gloss_id = B.gloss_id \
  LEFT JOIN arrowed_words D ON (A.gloss_id = D.gloss_id AND D.course_id = {course_id}) \
  LEFT JOIN words E ON E.word_id = D.word_id \
  LEFT JOIN course_x_text F ON (E.text = F.text_id AND F.course_id = {course_id}) \
  LEFT JOIN course_x_text G ON (A.text = G.text_id AND G.course_id = {course_id}) \
  LEFT JOIN running_counts_by_course H ON (H.course_id = {course_id} AND H.word_id = A.word_id) \
  LEFT JOIN total_counts_by_course I ON (I.course_id = {course_id} AND I.gloss_id = A.gloss_id) \
  WHERE A.gloss_id = {gloss_id} AND A.type > -1 \
  ORDER BY G.text_order,A.seq \
  LIMIT 400;", gloss_id=gloss_id, course_id = course_id);

    let res: Result<Vec<SmallWord>, sqlx::Error> = sqlx::query(&query)
        .map(|rec: SqliteRow| SmallWord {
            wordid: rec.get("word_id"),
            hqid: rec.get("gloss_id"),
            lemma: rec.get("lemma"),
            pos: rec.get("pos"),
            def: rec.get("def"),
            runningcount: rec.get("running_count"),
            arrowed_seq: rec.get("arrowedSeq"),
            total: rec.get("total_count"),
            seq: rec.get("seq"),
            is_flagged: rec.get("isFlagged"),
            word_text_seq: rec.get("text_order"),
            arrowed_text_seq: rec.get("arrowed_text_order"),
        })
        .fetch_all(&mut tx)
        .await;

    update_log_trx(
        &mut tx,
        UpdateType::SetGlossId,
        Some(word_id.into()),
        Some(history_id),
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

    tx.commit().await?;

    res
}

pub async fn add_text(
    pool: &SqlitePool,
    course_id: u32,
    text_name: &str,
    words: Vec<TextWord>,
    info: &ConnectionInfo,
) -> Result<u64, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let query = "INSERT INTO texts VALUES (NULL, ?, NULL, 1);";
    let text_id = sqlx::query(query)
        .bind(text_name)
        .execute(&mut tx)
        .await?
        .last_insert_rowid();

    //(word_id integer NOT NULL PRIMARY KEY AUTOINCREMENT, seq integer NOT NULL, text integer NOT NULL, section varchar (255) DEFAULT NULL, line varchar (255) DEFAULT NULL, word varchar (255) NOT NULL, gloss_id integer DEFAULT NULL REFERENCES glosses (gloss_id), lemma1 varchar (255) NOT NULL, lemma2 varchar (255) NOT NULL, o varchar (255) NOT NULL, runningcount integer NOT NULL, type integer DEFAULT NULL,
    //arrow integer NOT NULL DEFAULT 0, flagged integer NOT NULL DEFAULT 0, updated timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP,
    //updatedUserAgent varchar (255) NOT NULL DEFAULT '', updatedIP varchar (255) NOT NULL DEFAULT '', updatedUser varchar (255) NOT NULL DEFAULT '', isFlagged integer NOT NULL DEFAULT 0, note varchar (1024) NOT NULL DEFAULT '')

    let mut seq: u32 = 1;

    let query = "INSERT INTO words (word_id, seq, text, section, line, word, gloss_id, \
    lemma1, lemma2, o, runningcount, type, arrow, flagged, updated, \
    updatedUserAgent, updatedIP, updatedUser, isFlagged, note) \
    VALUES (NULL, ?, ?, '', '', ?, ?, '', '', '', 0, ?, 0, 0, ?, ?, ?, ?, 0, '');";
    let mut count = 0;
    let mut gloss_ids:HashSet<u32> = HashSet::new();
    for w in words {
        let res = sqlx::query(query)
            .bind(seq)
            .bind(text_id)
            .bind(w.word)
            .bind(w.gloss_id)
            .bind(w.word_type)
            .bind(info.timestamp)
            .bind(&info.user_agent)
            .bind(&info.ip_address)
            .bind(info.user_id)
            .execute(&mut tx)
            .await?;

        if let Some(g_id) = w.gloss_id {
            gloss_ids.insert(g_id);
        }

        seq += 1;

        let affected_rows = res.rows_affected();
        if affected_rows != 1 {
            tx.rollback().await?;
            return Ok(0); //or panic?
        }
        count += affected_rows;
    }

    let query = "SELECT MAX(text_order) FROM course_x_text WHERE course_id = ?;";
    let max_text_order: (u32,) = sqlx::query_as(query)
        .bind(course_id)
        .fetch_one(&mut tx)
        .await?;

    let query = "INSERT INTO course_x_text VALUES (?, ?, ?);";
    sqlx::query(query)
        .bind(course_id)
        .bind(text_id)
        .bind(max_text_order.0 + 1)
        .execute(&mut tx)
        .await?;

    update_log_trx(
        &mut tx,
        UpdateType::ImportText,
        Some(text_id),
        None,
        None,
        format!("Imported {} words into text ({})", count, text_id).as_str(),
        info,
    )
    .await?;

    for gloss_id in gloss_ids.iter() {
        update_counts_for_gloss_id(&mut tx, course_id, *gloss_id).await?;
    }

    //println!("id: {}, count: {}", text_id, count);

    tx.commit().await?;

    Ok(count)
}

pub async fn insert_gloss(
    pool: &SqlitePool,
    gloss: &str,
    pos: &str,
    def: &str,
    stripped_lemma: &str,
    note: &str,
    info: &ConnectionInfo,
) -> Result<(i64,u64), sqlx::Error> {
    let mut tx = pool.begin().await?;

    let query = "INSERT INTO glosses (gloss_id, seqold, seq, unit, lemma, lemma2, sortalpha, sortkey, \
    present, future, aorist, perfect, perfectmid, aoristpass, def, pos, link, freq, note, verbClass, \
    updated, arrowedDay, arrowedID, pageLine, parentid, status, updatedUserAgent, updatedIP, updatedUser) \
    VALUES (NULL, 0, 0, 0, ?, '', ?, '', '', '', '', '', '', '', ?, ?, '', 0, ?, 0, ?, 0, NULL, '', NULL, 1, ?, ?, ?);";

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
        .bind(&info.user_agent)
        .bind(&info.ip_address)
        .bind(info.user_id)
        .execute(&mut tx)
        .await?;

    let new_gloss_id = res.last_insert_rowid();

    update_log_trx(
        &mut tx,
        UpdateType::NewGloss,
        Some(new_gloss_id),
        None,
        None,
        format!("Added gloss ({})", new_gloss_id).as_str(),
        info,
    )
    .await?;

    tx.commit().await?;

    Ok((new_gloss_id, res.rows_affected()))
}

pub async fn update_log_trx<'a, 'b>(
    tx: &'a mut sqlx::Transaction<'b, sqlx::Sqlite>,
    update_type: UpdateType,
    object_id: Option<i64>,
    history_id: Option<i64>,
    course_id: Option<i64>,
    update_desc: &str,
    info: &ConnectionInfo,
) -> Result<(), sqlx::Error> {
    let query = "INSERT INTO update_log (update_id,update_type,object_id,history_id,course_id,update_desc,updated,user_id,ip,user_agent) VALUES (NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?);";
    sqlx::query(query)
        .bind(update_type.value())
        .bind(object_id)
        .bind(history_id)
        .bind(course_id)
        .bind(update_desc)
        .bind(info.timestamp)
        .bind(info.user_id)
        .bind(&info.ip_address)
        .bind(&info.user_agent)
        .execute(&mut *tx)
        .await?;

    Ok(())
}

pub async fn delete_gloss(
    pool: &SqlitePool,
    gloss_id: u32,
    info: &ConnectionInfo,
) -> Result<u64, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let query = "select count(*) from glosses a inner join words b on a.gloss_id=b.gloss_id where a.gloss_id = ?;";
    let count: (u32,) = sqlx::query_as(query)
        .bind(gloss_id)
        .fetch_one(&mut *tx)
        .await?;

    if count.0 == 0 {
        update_log_trx(
            &mut tx,
            UpdateType::DeleteGloss,
            Some(gloss_id.into()),
            Some(gloss_id.into()),
            None,
            format!("Deleted gloss ({})", gloss_id).as_str(),
            info,
        )
        .await?;

        let query = "UPDATE glosses SET status = 0 WHERE gloss_id = ?;";
        let res = sqlx::query(query).bind(gloss_id).execute(&mut tx).await?;

        tx.commit().await?;

        Ok(res.rows_affected())
    } else {
        Err(sqlx::Error::RowNotFound) //for now
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn update_gloss(
    pool: &SqlitePool,
    gloss_id: u32,
    gloss: &str,
    pos: &str,
    def: &str,
    stripped_gloss: &str,
    note: &str,
    info: &ConnectionInfo,
) -> Result<u64, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let query = "INSERT INTO glosses_history SELECT NULL,* FROM glosses WHERE gloss_id = ?;";
    let history_id = sqlx::query(query)
        .bind(gloss_id)
        .execute(&mut tx)
        .await?
        .last_insert_rowid();

    //let _ = update_log_trx(&mut tx, UpdateType::ArrowWord, "Arrowed word x from y to z.", timestamp, user_id, updated_ip, user_agent).await?;
    //let _ = update_log_trx(&mut tx, UpdateType::SetGlossId, "Change gloss for x from y to z.", timestamp, user_id, updated_ip, user_agent).await?;
    update_log_trx(
        &mut tx,
        UpdateType::EditGloss,
        Some(gloss_id.into()),
        Some(history_id),
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
    lemma = ?, \
    sortalpha = ?, \
    def = ?, \
    pos = ?, \
    note = ?, \
    updated = ?, \
    updatedUserAgent = ?, \
    updatedIP = ?, \
    updatedUser = ? \
    WHERE gloss_id = ?;";

    let res = sqlx::query(query)
        .bind(gloss)
        .bind(sl)
        .bind(def)
        .bind(pos)
        .bind(note)
        .bind(info.timestamp)
        .bind(&info.user_agent)
        .bind(&info.ip_address)
        .bind(info.user_id)
        .bind(gloss_id)
        .execute(&mut tx)
        .await?;

    tx.commit().await?;

    Ok(res.rows_affected())
}

/*
pub async fn update_counts_all<'a>(tx: &'a mut sqlx::Transaction<'a, sqlx::Sqlite>, course_id:u32) -> Result<(), sqlx::Error> {
  //select count(*) as c,b.lemma from words a inner join glosses b on a.gloss_id=b.gloss_id group by a.gloss_id order by c;

  // to update all total counts
  let query = format!("REPLACE INTO total_counts_by_course \
    SELECT {course_id},gloss_id,COUNT(*) \
    FROM words \
    WHERE gloss_id IS NOT NULL \
    GROUP BY gloss_id;", course_id=course_id);

  sqlx::query(&query).execute(&mut *tx).await?;

  let query = format!("REPLACE INTO running_counts_by_course \
    SELECT {course_id},a.word_id,count(*) AS running_count \
    FROM words a \
    INNER JOIN words b ON a.gloss_id=b.gloss_id \
    INNER JOIN course_x_text c ON a.text = c.text_id \
    INNER JOIN course_x_text d ON b.text = d.text_id \
    WHERE d.text_order <= c.text_order AND b.seq <= a.seq AND a.gloss_id IS NOT NULL \
    GROUP BY a.word_id \
    ORDER BY a.gloss_id, running_count;", course_id=course_id);

  sqlx::query(&query).execute(&mut *tx).await?;

  //to select running counts
  //select a.gloss_id,a.word_id,count(*) as num from words a INNER JOIN words b ON a.gloss_id=b.gloss_id inner join course_x_text c on a.text = c.text_id inner join course_x_text d on b.text = d.text_id where c.text_order <= d.text_order and a.seq <= b.seq and a.gloss_id=4106 group by a.word_id order by a.gloss_id, num;

  //when updating running count of just one we only need to update the words equal and after this one.
  Ok(())
}
*/

pub async fn update_counts_for_gloss_id<'a, 'b>(
    tx: &'a mut sqlx::Transaction<'b, sqlx::Sqlite>,
    course_id: u32,
    gloss_id: u32,
) -> Result<(), sqlx::Error> {

    if gloss_id < 1 {
        return Ok(());
    }
    // to update total counts for gloss in course
    let query = "SELECT COUNT(*) \
  FROM words a \
  INNER JOIN course_x_text b ON a.text = b.text_id \
  WHERE a.gloss_id = ? AND b.course_id = ? \
  GROUP BY a.gloss_id;";
    let count: Result<(u32,), sqlx::Error> = sqlx::query_as(query)
        .bind(gloss_id)
        .bind(course_id)
        .fetch_one(&mut *tx)
        .await;

    let c = if count.is_ok() { count.unwrap().0 } else { 0 };
    let query = "REPLACE INTO total_counts_by_course VALUES (?,?,?)";
    sqlx::query(query)
        .bind(course_id)
        .bind(gloss_id)
        .bind(c)
        .execute(&mut *tx)
        .await?; //https://stackoverflow.com/questions/41273041/what-does-combined-together-do-in-rust

    //to update running counts for gloss in course
    /*
      //this did not work:
      let query = "REPLACE INTO running_counts_by_course \
        SELECT c.course_id,a.word_id,COUNT(*) AS running_count \
        FROM words a \
        INNER JOIN words b ON a.gloss_id=b.gloss_id \
        INNER JOIN course_x_text c ON (a.text = c.text_id AND c.course_id = ?) \
        INNER JOIN course_x_text d ON (b.text = d.text_id AND d.course_id = ?) \
        WHERE d.text_order <= c.text_order AND b.seq <= a.seq AND a.gloss_id = ? \
        GROUP BY a.word_id;";

      /to select running counts
      //select a.gloss_id,a.word_id,count(*) as num from words a INNER JOIN words b ON a.gloss_id=b.gloss_id inner join course_x_text c on a.text = c.text_id inner join course_x_text d on b.text = d.text_id where c.text_order <= d.text_order and a.seq <= b.seq and a.gloss_id=4106 group by a.word_id order by a.gloss_id, num;



        //???this works
        //select a.gloss_id,a.word_id,count(*) as num from words a INNER JOIN words b ON a.gloss_id=b.gloss_id inner join course_x_text c on (a.text = c.text_id and c.course_id = 1) inner join course_x_text d on (b.text = d.text_id and d.course_id = 1) where c.text_order <= d.text_order and a.seq <= b.seq and a.gloss_id=4106 group by a.word_id order by a.gloss_id, num;
        //???but not this
        //select a.gloss_id,a.word_id,count(*) as num from words a INNER JOIN words b ON a.gloss_id=b.gloss_id inner join course_x_text c on (a.text = c.text_id and c.course_id = 1) inner join course_x_text d on (b.text = d.text_id and c.course_id = 1) where c.text_order <= d.text_order and a.seq <= b.seq and a.gloss_id=1422 group by a.word_id order by a.gloss_id, num;



      //when updating running count of just one we only need to update the words equal and after this one?
    */
    let query = "SELECT a.word_id FROM words a \
  INNER JOIN course_x_text c ON (a.text = c.text_id AND c.course_id = ?) \
  WHERE a.gloss_id = ? \
  ORDER BY c.text_order, a.seq;";
    let words: Vec<(u32,)> = sqlx::query_as(query)
        .bind(course_id)
        .bind(gloss_id)
        .fetch_all(&mut *tx)
        .await?;

    let mut running_count = 1;
    for word_id in words {
        let query = "REPLACE INTO running_counts_by_course VALUES (?,?,?)";
        sqlx::query(query)
            .bind(course_id)
            .bind(word_id.0)
            .bind(running_count)
            .execute(&mut *tx)
            .await?;

        running_count += 1;
    }

    Ok(())
}

/*
pub async fn fix_assignments(pool: &SqlitePool) -> Result<(), sqlx::Error> {
  let mut tx = pool.begin().await?;
/*
INSERT INTO texts (SELECT NULL,title,6,1 from assignments where id=28;

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

    let query = "INSERT INTO texts VALUES (NULL,?,?,1); ";
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
    let query = "INSERT INTO course_x_text VALUES (1,?,?); ";
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
pub async fn get_parent_text_id(pool: &SqlitePool, text_id:u32) -> Result<Option<u32>, sqlx::Error> {
  let query = "SELECT parent_id FROM texts WHERE text_id = ?;";
  let rec: (Option<u32>,) = sqlx::query_as(query)
  .bind(text_id)
  .fetch_one(pool)
  .await?;

    Ok(rec.0)
}
*/
pub async fn num_child_texts(pool: &SqlitePool, text_id: u32) -> Result<u32, sqlx::Error> {
    let query = "SELECT COUNT(*) FROM texts WHERE parent_id = ?;";
    let rec: (u32,) = sqlx::query_as(query).bind(text_id).fetch_one(pool).await?;

    Ok(rec.0)
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

pub async fn get_words(
    pool: &SqlitePool,
    text_id: u32,
    course_id: u32,
) -> Result<Vec<WordRow>, sqlx::Error> {
    //do not get words of whole text, if text is split into assignments
    let children = num_child_texts(pool, text_id).await?;
    if children > 0 {
        return Ok(vec![]);
    }

    let query = format!("SELECT A.word_id,A.word,A.type,B.lemma,A.lemma1,B.def,B.unit,pos,D.word_id as arrowedID,B.gloss_id,A.seq,E.seq AS arrowedSeq, \
    I.total_count, H.running_count,A.isFlagged, G.text_order,F.text_order AS arrowed_text_order \
    FROM words A \
    LEFT JOIN glosses B ON A.gloss_id = B.gloss_id \
    LEFT JOIN arrowed_words D ON (A.gloss_id = D.gloss_id AND D.course_id = {course_id}) \
    LEFT JOIN words E ON E.word_id = D.word_id \
    LEFT JOIN course_x_text F ON (E.text = F.text_id AND F.course_id = {course_id}) \
    LEFT JOIN course_x_text G ON ({text_id} = G.text_id AND G.course_id = {course_id}) \
    LEFT JOIN running_counts_by_course H ON (H.course_id = {course_id} AND H.word_id = A.word_id) \
    LEFT JOIN total_counts_by_course I ON (I.course_id = {course_id} AND I.gloss_id = A.gloss_id) \
    WHERE A.text = {text_id} AND A.type > -1 \
    ORDER BY A.seq \
    LIMIT 55000;", text_id = text_id, course_id = course_id);

    //WHERE A.seq >= {start_seq} AND A.seq <= {end_seq} AND A.type > -1 \

    //println!("{}", query);

    let res: Result<Vec<WordRow>, sqlx::Error> = sqlx::query(&query)
        .map(|rec: SqliteRow| WordRow {
            wordid: rec.get("word_id"),
            word: rec.get("word"),
            word_type: rec.get("type"),
            lemma: rec.get("lemma"),
            lemma1: rec.get("lemma1"),
            def: rec.get("def"),
            unit: rec.get("unit"),
            pos: rec.get("pos"),
            arrowed_id: rec.get("arrowedID"),
            hqid: rec.get("gloss_id"),
            seq: rec.get("seq"),
            arrowed_seq: rec.get("arrowedSeq"),
            freq: rec.get("total_count"),
            runningcount: rec.get("running_count"),
            is_flagged: rec.get("isFlagged"),
            word_text_seq: rec.get("text_order"),
            arrowed_text_seq: rec.get("arrowed_text_order"),
        })
        .fetch_all(pool)
        .await;

    res
}

//*insert assignments into texts
//update text_id in words table based on assignment seq ranges

//change get_words to use subtext id
//order of assignments will be by id?  or word_seq?

pub async fn get_text_name(
  pool: &SqlitePool,
  text_id: u32,
) -> Result<String, sqlx::Error> {
  //let query = "SELECT id,title,wordcount FROM assignments ORDER BY id;";
  let query = "SELECT name \
  FROM texts \
  WHERE text_id = ?";
  let res:(String,) = sqlx::query_as(query)
      .bind(text_id)
      .fetch_one(pool)
      .await?;

  Ok(res.0)
}

// pub async fn update_counts_for_text_trx<'a, 'b>(
//     tx: &'a mut sqlx::Transaction<'b, sqlx::Sqlite>,
//     course_id: u32,
//     text_id: u32,
// ) -> Result<(), sqlx::Error> {

//     let query = "SELECT hqid FROM words where text_id = ?;";
//     let gloss_ids: Vec<(u32,)> = sqlx::query_as(&query)
//         .bind(text_id)
//         .fetch_all(tx)
//         .await?;

//     for gloss_id in gloss_ids {
//         update_counts_for_gloss_id(tx, course_id, gloss_id.0).await?;
//     }
//     Ok(())
// }

pub async fn update_text_order_db(
    pool: &SqlitePool,
    course_id: u32,
    text_id: u32,
    step: i32,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    let query = "SELECT text_order FROM course_x_text WHERE course_id = ? AND text_id = ?;";
    let text_order: (i32,) = sqlx::query_as(query)
        .bind(course_id)
        .bind(text_id)
        .fetch_one(pool).await?;

    let query = "SELECT COUNT(*) FROM course_x_text WHERE course_id = ?;";
    let text_count: (i32,) = sqlx::query_as(query)
        .bind(course_id)
        .bind(text_id)
        .fetch_one(pool).await?;

    if step == 0 || (text_order.0 + step < 1 && step < 0) || (text_order.0 + step > text_count.0 && step > 0) {
        return Err(sqlx::Error::RowNotFound); //at no where to move: abort
    }
    else if step > 0 { //make room by moving other texts up/earlier in sequence
        let query = "UPDATE course_x_text SET text_order = text_order - 1 \
            WHERE text_order > ? AND text_order < ? + ? + 1 AND course_id = ?;";
        sqlx::query(query)
        .bind(text_order.0)
        .bind(text_order.0)
        .bind(step)
        .bind(course_id)
        .execute(&mut tx)
        .await?;
    }
    else { //make room by moving other texts down/later in sequence
        let query = "UPDATE course_x_text SET text_order = text_order + 1 \
            WHERE text_order < ? AND text_order > ? + ? - 1 AND course_id = ?;";
        sqlx::query(query)
        .bind(text_order.0)
        .bind(text_order.0)
        .bind(step) //step will be negative here
        .bind(course_id)
        .execute(&mut tx)
        .await?;
    }
    //set new text order
    let query = "UPDATE course_x_text SET text_order = text_order + ? WHERE course_id = ? AND text_id = ?;";
    sqlx::query(query)
        .bind(step)
        .bind(course_id)
        .bind(text_id)
        .execute(&mut tx)
        .await?;

    //update_counts_for_text_trx(&mut tx, course_id, text_id).await?;
    let query = "SELECT gloss_id FROM words where text = ? AND gloss_id IS NOT NULL;";
    let gloss_ids: Vec<(u32,)> = sqlx::query_as(query)
        .bind(text_id)
        .fetch_all(&mut tx)
        .await?;

    for gloss_id in gloss_ids {
        update_counts_for_gloss_id(&mut tx, course_id, gloss_id.0).await?;
    }

    tx.commit().await?;
    Ok(())
}

pub async fn get_texts_db(
    pool: &SqlitePool,
    course_id: u32,
) -> Result<Vec<AssignmentRow>, sqlx::Error> {
    //let query = "SELECT id,title,wordcount FROM assignments ORDER BY id;";
    let query = "SELECT A.text_id, A.name, A.parent_id, B.course_id \
    FROM texts A \
    INNER JOIN course_x_text B ON (A.text_id = B.text_id AND B.course_id = ?) \
    ORDER BY B.text_order, A.text_id;";
    let res: Result<Vec<AssignmentRow>, sqlx::Error> = sqlx::query(query)
        .bind(course_id)
        .map(|rec: SqliteRow| AssignmentRow {
            id: rec.get("text_id"),
            assignment: rec.get("name"),
            parent_id: rec.get("parent_id"),
            course_id: rec.get("course_id"),
        })
        .fetch_all(pool)
        .await;

    res
}
/*
pub async fn _get_titles(pool: &SqlitePool) -> Result<Vec<(String,u32)>, sqlx::Error> {
    let query = "SELECT id,title FROM titles ORDER BY title;";
    let res: Result<Vec<(String,u32)>, sqlx::Error> = sqlx::query(query)
    .map(|rec: SqliteRow| (rec.get("id"),rec.get("title")) )
    .fetch_all(pool)
    .await;

    res
}
*/
pub async fn get_text_id_for_word_id(pool: &SqlitePool, word_id: u32) -> Result<u32, sqlx::Error> {
    let query = "SELECT text FROM words WHERE word_id = ?;";

    let rec: (u32,) = sqlx::query_as(query).bind(word_id).fetch_one(pool).await?;

    Ok(rec.0)
}
/*
pub async fn old_get_text_id_for_word_id(pool: &SqlitePool, word_id:u32) -> Result<u32, sqlx::Error> {
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
pub async fn get_start_end(pool: &SqlitePool, text_id:u32) -> Result<(u32,u32), sqlx::Error> {
  let query = "SELECT b.seq, c.seq FROM assignments a INNER JOIN words b ON a.start = b.word_id INNER JOIN words c ON a.end = c.word_id WHERE a.id = ?;";

  let rec: (u32,u32) = sqlx::query_as(query)
  .bind(text_id)
  .fetch_one(pool)
  .await?;

  Ok(rec)
}
*/

pub async fn get_glossdb(pool: &SqlitePool, gloss_id: u32) -> Result<GlossEntry, sqlx::Error> {
    let query = "SELECT gloss_id, lemma, pos, def, note FROM glosses WHERE gloss_id = ? ";

    sqlx::query(query)
        .bind(gloss_id)
        .map(|rec: SqliteRow| GlossEntry {
            hqid: rec.get("gloss_id"),
            l: rec.get("lemma"),
            pos: rec.get("pos"),
            g: rec.get("def"),
            n: rec.get("note"),
        })
        .fetch_one(pool)
        .await
}

//SELECT c.name, a.word_id, a.word, d.word_id as arrowed FROM words a INNER JOIN course_x_text b ON (a.text = b.text_id AND b.course_id = 1) INNER JOIN texts c ON a.text = c.text_id LEFT JOIN arrowed_words d ON (d.course_id=1 AND d.gloss_id=564 AND d.word_id = a.word_id) WHERE a.gloss_id = 564 ORDER BY b.text_order, a.seq LIMIT 20000;

pub async fn get_gloss_occurrences(
    pool: &SqlitePool,
    course_id: u32,
    gloss_id: u32,
) -> Result<Vec<GlossOccurrence>, sqlx::Error> {
    let query = "SELECT c.name, a.word_id, a.word, d.word_id as arrowed, e.unit, e.lemma \
    FROM words a \
    INNER JOIN course_x_text b ON (a.text = b.text_id AND b.course_id = ?) \
    INNER JOIN texts c ON a.text = c.text_id \
    INNER JOIN glosses e ON e.gloss_id = a.gloss_id \
    LEFT JOIN arrowed_words d ON (d.course_id=? AND d.gloss_id=? AND d.word_id = a.word_id) \
    WHERE a.gloss_id = ? \
    ORDER BY b.text_order, a.seq \
    LIMIT 2000;".to_string();

    let mut res: Vec<GlossOccurrence> = sqlx::query(&query)
        .bind(course_id)
        .bind(course_id)
        .bind(gloss_id)
        .bind(gloss_id)
        .map(|rec: SqliteRow| {
            GlossOccurrence {
                name: rec.get("name"),
                word_id: rec.get("word_id"),
                word: rec.get("word"),
                arrowed: rec.get("arrowed"),
                unit: rec.get("unit"),
                lemma: rec.get("lemma"),
            }
        })
        .fetch_all(pool)
        .await?;

    if !res.is_empty() && res[0].unit.is_some() && res[0].unit.unwrap() > 0 && res[0].unit.unwrap() < 21 {
      res.insert(0, GlossOccurrence {
            name: format!("H&Q Unit {}", res[0].unit.unwrap()),
            word_id: 1,
            word: res[0].lemma.to_owned(),
            arrowed: Some(1),
            unit: res[0].unit,
            lemma: res[0].lemma.to_owned(),
        } )
    }

    Ok(res)
}
use crate::AssignmentTree;
pub async fn get_update_log(
    pool: &SqlitePool,
    _course_id: u32,
) -> Result<Vec<AssignmentTree>, sqlx::Error> {
    let query = "SELECT strftime('%Y-%m-%d %H:%M:%S', DATETIME(updated, 'unixepoch')) as timestamp, a.update_id, \
    b.update_type, c.initials, update_desc \
    FROM update_log a \
    INNER JOIN update_types b ON a.update_type = b.update_type_id \
    INNER JOIN users c ON a.user_id = c.user_id \
    ORDER BY updated DESC \
    LIMIT 20000;".to_string();

    let res: Vec<(String, String, String, String,u32)> = sqlx::query(&query)
        .map(|rec: SqliteRow| {
            (
                rec.get("timestamp"),
                rec.get("update_type"),
                rec.get("initials"),
                rec.get("update_desc"),
                rec.get("update_id"),
            )
        })
        .fetch_all(pool)
        .await?;

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

pub async fn get_before(
    pool: &SqlitePool,
    searchprefix: &str,
    page: i32,
    limit: u32,
) -> Result<Vec<(String, u32, String, u32)>, sqlx::Error> {
    let query = format!("SELECT a.gloss_id,a.lemma,a.def,b.total_count FROM glosses a LEFT JOIN total_counts_by_course b ON a.gloss_id=b.gloss_id WHERE a.sortalpha COLLATE PolytonicGreek < '{}' and status > 0 and pos != 'gloss' ORDER BY a.sortalpha COLLATE PolytonicGreek DESC LIMIT {},{};", searchprefix, -page * limit as i32, limit);
    let res: Result<Vec<(String, u32, String, u32)>, sqlx::Error> = sqlx::query(&query)
        .map(|rec: SqliteRow| {
            (
                rec.get("lemma"),
                rec.get("gloss_id"),
                rec.get("def"),
                rec.get("total_count"),
            )
        })
        .fetch_all(pool)
        .await;

    res
}

pub async fn get_equal_and_after(
    pool: &SqlitePool,
    searchprefix: &str,
    page: i32,
    limit: u32,
) -> Result<Vec<(String, u32, String, u32)>, sqlx::Error> {
    let query = format!("SELECT a.gloss_id,a.lemma,a.def,b.total_count FROM glosses a LEFT JOIN total_counts_by_course b ON a.gloss_id=b.gloss_id WHERE a.sortalpha COLLATE PolytonicGreek >= '{}' and status > 0 and pos != 'gloss' ORDER BY a.sortalpha COLLATE PolytonicGreek LIMIT {},{};", searchprefix, page * limit as i32, limit);
    let res: Result<Vec<(String, u32, String, u32)>, sqlx::Error> = sqlx::query(&query)
        .map(|rec: SqliteRow| {
            (
                rec.get("lemma"),
                rec.get("gloss_id"),
                rec.get("def"),
                rec.get("total_count"),
            )
        })
        .fetch_all(pool)
        .await;

    res
}

/*
CREATE TABLE IF NOT EXISTS update_types (update_type_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, update_type text NOT NULL);
CREATE TABLE IF NOT EXISTS update_log (update_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, update_type INTEGER REFERENCES update_types(update_type_id), update_desc TEXT, comment TEXT, updated INTEGER NOT NULL, user_id INTEGER REFERENCES users(user_id), ip TEXT, user_agent TEXT );

CREATE TABLE IF NOT EXISTS users (user_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, name text NOT NULL, initials NOT NULL, user_type INTEGER NOT NULL);

CREATE TABLE IF NOT EXISTS courses (course_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, name text NOT NULL);
CREATE TABLE IF NOT EXISTS texts (text_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, name text NOT NULL);

CREATE TABLE IF NOT EXISTS arrowed_words (course_id INTEGER NOT NULL REFERENCES text_sequences(course_id), gloss_id INTEGER NOT NULL REFERENCES glosses(gloss_id), word_id INTEGER NOT NULL REFERENCES words(word_id), updated INTEGER, user_id INTEGER REFERENCES users(user_id), comment text, PRIMARY KEY(course_id, gloss_id, word_id));
INSERT INTO arrowed_words SELECT 1, hqid, arrowedID,0,NULL,NULL from hqvocab where arrowedid is not null;
CREATE TABLE IF NOT EXISTS arrowed_words_history (history_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, seq_id INTEGER NOT NULL REFERENCES text_sequences(seq_id), lemma_id INTEGER NOT NULL REFERENCES hqvocab(hqid), word_id INTEGER, updated INTEGER, user_id INTEGER REFERENCES users(user_id), comment text);
CREATE INDEX IF NOT EXISTS arrowed_words_history_idx ON arrowed_words (seq_id, lemma_id);

CREATE TABLE IF NOT EXISTS course_x_text (seq_id INTEGER NOT NULL REFERENCES text_sequences(seq_id), text_id INTEGER NOT NULL REFERENCES texts(text_id), text_order INTEGER NOT NULL, PRIMARY KEY (seq_id,text_id));

CREATE TABLE IF NOT EXISTS running_counts_by_course (seq_id INTEGER NOT NULL REFERENCES text_sequences(seq_id), word_id INTEGER NOT NULL REFERENCES gkvocabdb(wordid), running_count INTEGER, PRIMARY KEY (seq_id,word_id));
CREATE TABLE IF NOT EXISTS total_counts_by_course (course_id INTEGER NOT NULL REFERENCES text_sequences(course_id), gloss_id INTEGER NOT NULL REFERENCES glosses(gloss_id), total_count INTEGER, PRIMARY KEY (seq_id,lemma_id));

to add:
gkvocabdb text references text_id, lemma_id references hqid, seq, type references types table?,
gkvocabassignments start,end references wordid?

add PolytonicGreek collation to hqvocabdb sortalpha


CREATE TABLE IF NOT EXISTS words_history (word_history_id integer not null PRIMARY KEY AUTOINCREMENT, word_id integer NOT NULL, seq integer NOT NULL, text integer NOT NULL, section varchar (255) DEFAULT NULL, line varchar (255) DEFAULT NULL, word varchar (255) NOT NULL, gloss_id integer DEFAULT NULL REFERENCES glosses (gloss_id), lemma1 varchar (255) NOT NULL, lemma2 varchar (255) NOT NULL, o varchar (255) NOT NULL, runningcount integer NOT NULL, type integer DEFAULT NULL, arrow integer NOT NULL DEFAULT 0, flagged integer NOT NULL DEFAULT 0, updated timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP, updatedUserAgent varchar (255) NOT NULL DEFAULT '', updatedIP varchar (255) NOT NULL DEFAULT '', updatedUser varchar (255) NOT NULL DEFAULT '', isFlagged integer NOT NULL DEFAULT 0, note varchar (1024) NOT NULL DEFAULT '');
*/

/*

.mode ascii
.separator "," "\n"
.import /Users/jeremy/Downloads/gkvocabdbxxx.csv gkvocabdb

//https://github.com/dumblob/mysql2sqlite
mysqldump --skip-extended-insert --compact philolog_us gkvocabdb hqvocab gkvocabAssignments appcrit > gkvocabdbxxxx.sql
./mysql2sqlite gkvocabdbxxxx.sql | sqlite3 gkvocabdb2.sqlite

  CREATE TABLE IF NOT EXISTS gkvocabdb (
    wordid INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    seq int(10) NOT NULL,
    text int(11) NOT NULL,
    section varchar(255) DEFAULT NULL,
    line varchar(255)  DEFAULT NULL,
    word varchar(255)  NOT NULL,
    lemmaid int(10)  DEFAULT NULL,
    lemmaa varchar(255)  NOT NULL,
    lemmab varchar(255)  NOT NULL,
    o varchar(255)  NOT NULL,
    runningcount int(10)  NOT NULL,
    type tinyint(4) DEFAULT NULL,
    arrow tinyint(1) NOT NULL DEFAULT 0,
    flagged tinyint(1) NOT NULL DEFAULT 0,
    updated timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updatedUserAgent varchar(255)  NOT NULL DEFAULT '',
    updatedIP varchar(255)  NOT NULL DEFAULT '',
    updatedUser varchar(255)  NOT NULL DEFAULT '',
    isFlagged tinyint(4) NOT NULL DEFAULT 0,
    note varchar(1024)  NOT NULL DEFAULT ''
  )
    CREATE TABLE gkvocabdb (wordid INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,seq int(10) NOT NULL,text int(11) NOT NULL,section varchar(255) DEFAULT NULL,line varchar(255)  DEFAULT NULL,word varchar(255)  NOT NULL,lemmaid int(10)  DEFAULT NULL,lemma1 varchar(255) NOT NULL,lemma2 varchar(255)  NOT NULL,o varchar(255)  NOT NULL,runningcount int(10)  NOT NULL,type tinyint(4) DEFAULT NULL,arrow tinyint(1) NOT NULL DEFAULT 0,flagged tinyint(1) NOT NULL DEFAULT 0,updated timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP,updatedUserAgent varchar(255)  NOT NULL DEFAULT '',updatedIP varchar(255)  NOT NULL DEFAULT '',updatedUser varchar(255)  NOT NULL DEFAULT '',isFlagged tinyint(4) NOT NULL DEFAULT 0,note varchar(1024)  NOT NULL DEFAULT '',KEY lemma1 (lemma1),KEY seq (seq),KEY lemmaid (lemmaid))

  CREATE TABLE hqvocab (
    hqid int(11)  NOT NULL AUTOINCREMENT,
    seqold smallint(5)  NOT NULL DEFAULT 0,
    seq smallint(10)  NOT NULL DEFAULT 0,
    unit smallint(6)  NOT NULL,
    lemma varchar(256) NOT NULL,
    lemma2 varchar(255) NOT NULL DEFAULT '',
    sortalpha varchar(255)  NOT NULL DEFAULT '',
    sortkey varchar(255)  NOT NULL,
    present varchar(256)  NOT NULL,
    future varchar(256)  NOT NULL,
    aorist varchar(256)  NOT NULL,
    perfect varchar(256)  NOT NULL,
    perfectmid varchar(256)  NOT NULL,
    aoristpass varchar(256)  NOT NULL,
    def varchar(1024)  NOT NULL,
    pos varchar(256)  NOT NULL,
    link varchar(256)  NOT NULL,
    freq smallint(6) NOT NULL,
    note varchar(256)  NOT NULL,
    verbClass int(10)  NOT NULL DEFAULT 0,
    updated timestamp NOT NULL DEFAULT current_timestamp() ON UPDATE current_timestamp(),
    arrowedDay smallint(5)  DEFAULT NULL,
    arrowedID int(11) DEFAULT NULL,
    pageLine varchar(255)  DEFAULT NULL,
    parentid int(11)  DEFAULT NULL,
    status tinyint(4) NOT NULL DEFAULT 1,
    updatedUserAgent varchar(255)  NOT NULL DEFAULT '',
    updatedIP varchar(255)  NOT NULL DEFAULT '',
    updatedUser varchar(255)  NOT NULL DEFAULT '',
    PRIMARY KEY (hqid),
    KEY updated (updated),
    KEY seq (seqold),
    KEY sortkey (sortkey),
    KEY sortalpha (sortalpha),
    KEY lemma (lemma(255)),
    KEY parentididx (parentid)
  )

  CREATE TABLE gkvocabAssignments (
    id int(11) NOT NULL AUTO_INCREMENT,
    sort int(11) NOT NULL,
    title varchar(255) NOT NULL,
    start int(10)  NOT NULL,
    end int(10)  NOT NULL,
    wordcount int(10)  DEFAULT NULL,
    PRIMARY KEY (id)
  )

  CREATE TABLE appcrit (
    wordid int(10)  NOT NULL,
    entry varchar(1024) DEFAULT NULL,
    PRIMARY KEY (wordid)
  )
*/

#[allow(dead_code)]
pub async fn insert_user(db:&SqlitePool, name:&str, initials:&str, user_type:u32, password:&str, email:&str) -> Result<i64, sqlx::Error> {
    let mut tx = db.begin().await?;

    let query = r#"INSERT INTO users VALUES (NULL, ?, ?, ?, ?, ?);"#;
    let user_id = sqlx::query(query)
        .bind(name)
        .bind(initials)
        .bind(user_type)
        .bind(password)
        .bind(email)
        .execute(&mut tx)
        .await?
        .last_insert_rowid();

    tx.commit().await?;
    Ok(user_id)
}

pub async fn create_db(db:&SqlitePool) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;

    let query = r#"
        CREATE TABLE IF NOT EXISTS courses (course_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, name text NOT NULL);
        /*CREATE TABLE sqlite_sequence(name,seq);*/
        CREATE TABLE IF NOT EXISTS course_x_text (course_id INTEGER NOT NULL REFERENCES courses (course_id), text_id INTEGER NOT NULL REFERENCES texts (text_id), text_order INTEGER NOT NULL, PRIMARY KEY (course_id, text_id));
        CREATE TABLE IF NOT EXISTS glosses (gloss_id integer NOT NULL PRIMARY KEY AUTOINCREMENT, seqold integer NOT NULL DEFAULT 0, seq integer NOT NULL DEFAULT 0, unit integer NOT NULL, lemma varchar (256) NOT NULL, lemma2 varchar (255) NOT NULL DEFAULT '', sortalpha varchar (255) NOT NULL DEFAULT '', sortkey varchar (255) NOT NULL, present varchar (256) NOT NULL, future varchar (256) NOT NULL, aorist varchar (256) NOT NULL, perfect varchar (256) NOT NULL, perfectmid varchar (256) NOT NULL, aoristpass varchar (256) NOT NULL, def varchar (1024) NOT NULL, pos varchar (256) NOT NULL, link varchar (256) NOT NULL, freq integer NOT NULL, note varchar (256) NOT NULL, verbClass integer NOT NULL DEFAULT 0, updated timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP, arrowedDay integer DEFAULT NULL, arrowedID integer DEFAULT NULL, pageLine varchar (255) DEFAULT NULL, parentid integer DEFAULT NULL, status integer NOT NULL DEFAULT 1, updatedUserAgent varchar (255) NOT NULL DEFAULT '', updatedIP varchar (255) NOT NULL DEFAULT '', updatedUser varchar (255) NOT NULL DEFAULT '');
        CREATE TABLE IF NOT EXISTS total_counts_by_course (course_id INTEGER NOT NULL REFERENCES courses (course_id), gloss_id INTEGER NOT NULL REFERENCES glosses (gloss_id), total_count INTEGER, PRIMARY KEY (course_id, gloss_id));
        CREATE TABLE IF NOT EXISTS running_counts_by_course (course_id INTEGER NOT NULL REFERENCES courses (course_id), word_id INTEGER NOT NULL REFERENCES words (word_id), running_count INTEGER, PRIMARY KEY (course_id, word_id));
        CREATE TABLE IF NOT EXISTS assignments (id integer NOT NULL PRIMARY KEY AUTOINCREMENT, sort integer NOT NULL, title varchar (255) NOT NULL, start integer NOT NULL, "end" integer NOT NULL, wordcount integer DEFAULT NULL);
        CREATE TABLE IF NOT EXISTS arrowed_words (course_id INTEGER NOT NULL REFERENCES courses (course_id), gloss_id INTEGER NOT NULL REFERENCES glosses (gloss_id), word_id INTEGER NOT NULL REFERENCES words (word_id), updated INTEGER, user_id INTEGER REFERENCES users (user_id), comment text, PRIMARY KEY (course_id, gloss_id, word_id));
        CREATE TABLE IF NOT EXISTS arrowed_words_history (history_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, course_id INTEGER NOT NULL REFERENCES courses (course_id), gloss_id INTEGER NOT NULL REFERENCES glosses (gloss_id), word_id INTEGER, updated INTEGER, user_id INTEGER REFERENCES users (user_id), comment text);
        CREATE TABLE IF NOT EXISTS appcrit (word_id integer NOT NULL, entry varchar (1024) DEFAULT NULL, PRIMARY KEY (word_id));
        CREATE TABLE IF NOT EXISTS words (word_id integer NOT NULL PRIMARY KEY AUTOINCREMENT, seq integer NOT NULL, text integer NOT NULL, section varchar (255) DEFAULT NULL, line varchar (255) DEFAULT NULL, word varchar (255) NOT NULL, gloss_id integer DEFAULT NULL REFERENCES glosses (gloss_id), lemma1 varchar (255) NOT NULL, lemma2 varchar (255) NOT NULL, o varchar (255) NOT NULL, runningcount integer NOT NULL, type integer DEFAULT NULL, arrow integer NOT NULL DEFAULT 0, flagged integer NOT NULL DEFAULT 0, updated timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP, updatedUserAgent varchar (255) NOT NULL DEFAULT '', updatedIP varchar (255) NOT NULL DEFAULT '', updatedUser varchar (255) NOT NULL DEFAULT '', isFlagged integer NOT NULL DEFAULT 0, note varchar (1024) NOT NULL DEFAULT '');
        CREATE TABLE IF NOT EXISTS words_history (word_history_id integer not null PRIMARY KEY AUTOINCREMENT, word_id integer NOT NULL, seq integer NOT NULL, text integer NOT NULL, section varchar (255) DEFAULT NULL, line varchar (255) DEFAULT NULL, word varchar (255) NOT NULL, gloss_id integer DEFAULT NULL REFERENCES glosses (gloss_id), lemma1 varchar (255) NOT NULL, lemma2 varchar (255) NOT NULL, o varchar (255) NOT NULL, runningcount integer NOT NULL, type integer DEFAULT NULL, arrow integer NOT NULL DEFAULT 0, flagged integer NOT NULL DEFAULT 0, updated timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP, updatedUserAgent varchar (255) NOT NULL DEFAULT '', updatedIP varchar (255) NOT NULL DEFAULT '', updatedUser varchar (255) NOT NULL DEFAULT '', isFlagged integer NOT NULL DEFAULT 0, note varchar (1024) NOT NULL DEFAULT '');
        CREATE TABLE IF NOT EXISTS glosses_history (gloss_history_id integer NOT NULL PRIMARY KEY AUTOINCREMENT, gloss_id integer NOT NULL, seqold integer NOT NULL DEFAULT 0, seq integer NOT NULL DEFAULT 0, unit integer NOT NULL, lemma varchar (256) NOT NULL, lemma2 varchar (255) NOT NULL DEFAULT '', sortalpha varchar (255) NOT NULL DEFAULT '', sortkey varchar (255) NOT NULL, present varchar (256) NOT NULL, future varchar (256) NOT NULL, aorist varchar (256) NOT NULL, perfect varchar (256) NOT NULL, perfectmid varchar (256) NOT NULL, aoristpass varchar (256) NOT NULL, def varchar (1024) NOT NULL, pos varchar (256) NOT NULL, link varchar (256) NOT NULL, freq integer NOT NULL, note varchar (256) NOT NULL, verbClass integer NOT NULL DEFAULT 0, updated timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP, arrowedDay integer DEFAULT NULL, arrowedID integer DEFAULT NULL, pageLine varchar (255) DEFAULT NULL, parentid integer DEFAULT NULL, status integer NOT NULL DEFAULT 1, updatedUserAgent varchar (255) NOT NULL DEFAULT '', updatedIP varchar (255) NOT NULL DEFAULT '', updatedUser varchar (255) NOT NULL DEFAULT '');
        CREATE TABLE IF NOT EXISTS update_types (update_type_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, update_type text NOT NULL);
        CREATE TABLE IF NOT EXISTS "texts" (text_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, name text NOT NULL, parent_id integer references texts (text_id) default null, display integer default 1);
        CREATE TABLE IF NOT EXISTS update_log (update_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, update_type INTEGER REFERENCES update_types(update_type_id), object_id INTEGER, history_id INTEGER, course_id INTEGER, update_desc TEXT, comment TEXT, updated INTEGER NOT NULL, user_id INTEGER REFERENCES users(user_id), ip TEXT, user_agent TEXT );
        CREATE TABLE IF NOT EXISTS users (user_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, name text NOT NULL, initials NOT NULL, user_type INTEGER NOT NULL, password text NOT NULL DEFAULT "81237698562398", email TEXT);
        CREATE INDEX IF NOT EXISTS idx_hqvocab_lemma ON glosses (lemma);
        CREATE INDEX IF NOT EXISTS idx_hqvocab_parentididx ON glosses (parentid);
        CREATE INDEX IF NOT EXISTS idx_hqvocab_seq ON glosses (seqold);
        CREATE INDEX IF NOT EXISTS idx_hqvocab_sortalpha ON glosses (sortalpha);
        CREATE INDEX IF NOT EXISTS idx_hqvocab_sortkey ON glosses (sortkey);
        CREATE INDEX IF NOT EXISTS idx_hqvocab_updated ON glosses (updated);
        CREATE INDEX IF NOT EXISTS arrowed_words_history_idx ON arrowed_words (course_id, gloss_id);
        CREATE INDEX IF NOT EXISTS idx_gkvocabdb_lemma1 ON words (lemma1);
        CREATE INDEX IF NOT EXISTS idx_gkvocabdb_lemmaid ON words (gloss_id);
        CREATE INDEX IF NOT EXISTS idx_gkvocabdb_seq ON words (seq);
        "#;

    let _res = sqlx::query(query)
        .execute(&mut tx)
        .await?;

    //create default course
    let query = r#"REPLACE INTO courses VALUES (1,'Greek');"#;
    sqlx::query(query)
        .execute(&mut tx)
        .await?;

    //insert update types
    let query = r#"REPLACE INTO update_types VALUES (?,?);"#;
    let update_types = vec![(1,"Arrow word"), (2,"Unarrow word"), (3,"New gloss"), (4,"Edit gloss"), (5,"Set gloss"), (6,"Import text"), (7,"Delete gloss")];
    for t in update_types {
        sqlx::query(query)
            .bind(t.0)
            .bind(t.1)
            .execute(&mut tx)
            .await?;
    }

    tx.commit().await?;
    Ok(())
}
