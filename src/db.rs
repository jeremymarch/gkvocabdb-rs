/*
gkvocabdb

Copyright (C) 2021  Jeremy March

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <https://www.gnu.org/licenses/>. 
*/

use sqlx::sqlite::SqliteRow;
use sqlx::{FromRow, Row, SqlitePool };
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct WordRow {
    #[serde(rename(serialize = "i"))]
    pub wordid:u32,
    #[serde(rename(serialize = "w"))]
    pub word:String,
    #[serde(rename(serialize = "t"))]
    pub word_type:u8,
    #[serde(rename(serialize = "l"))]
    pub lemma:String,
    #[serde(rename(serialize = "l1"))]
    pub lemma1:String,
    pub def:String,
    #[serde(rename(serialize = "u"))]
    pub unit:u8,
    pub pos:String,
    #[serde(rename(serialize = "a"))]
    pub arrowed_id: Option<u32>,
    pub hqid:u32,
    #[serde(rename(serialize = "s"))]
    pub seq:u32,
    #[serde(rename(serialize = "s2"))]
    pub arrowed_seq: Option<u32>,
    #[serde(rename(serialize = "c"))]
    pub freq: u32, 
    #[serde(rename(serialize = "rc"))]
    pub runningcount: u32,
    #[serde(rename(serialize = "if"))]
    pub is_flagged: bool,
    pub word_text_seq: u32,
    pub arrowed_text_seq: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct AssignmentRow {
  pub id:u32,
  pub assignment:String
}

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
pub struct DefRow {
    pub word: String,
    pub sortword: String,
    pub def: String,
    pub seq: u32
}

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

pub async fn get_before(pool: &SqlitePool, searchprefix: &str, page: i32, limit: u32) -> Result<Vec<(String, u32, String, u32)>, sqlx::Error> {
  let query = format!("SELECT a.gloss_id,a.lemma,a.def,b.total_count FROM glosses a INNER JOIN total_counts_by_course b ON a.gloss_id=b.gloss_id WHERE a.sortalpha COLLATE PolytonicGreek < '{}' and status > 0 and pos != 'gloss' ORDER BY a.sortalpha COLLATE PolytonicGreek DESC LIMIT {},{};", searchprefix, -page * limit as i32, limit);
  let res: Result<Vec<(String, u32, String, u32)>, sqlx::Error> = sqlx::query(&query)
  .map(|rec: SqliteRow| (rec.get("lemma"),rec.get("gloss_id"),rec.get("def"),rec.get("total_count") ) )
  .fetch_all(pool)
  .await;

  res
}

pub async fn get_equal_and_after(pool: &SqlitePool, searchprefix: &str, page: i32, limit: u32) -> Result<Vec<(String, u32, String, u32)>, sqlx::Error> {
  let query = format!("SELECT a.gloss_id,a.lemma,a.def,b.total_count FROM glosses a INNER JOIN total_counts_by_course b ON a.gloss_id=b.gloss_id WHERE a.sortalpha COLLATE PolytonicGreek >= '{}' and status > 0 and pos != 'gloss' ORDER BY a.sortalpha COLLATE PolytonicGreek LIMIT {},{};", searchprefix, page * limit as i32, limit);
  let res: Result<Vec<(String, u32, String, u32)>, sqlx::Error> = sqlx::query(&query)
  .map(|rec: SqliteRow| (rec.get("lemma"),rec.get("gloss_id"),rec.get("def"),rec.get("total_count") ) )
  .fetch_all(pool)
  .await;

  res
}

pub async fn arrow_word(pool: &SqlitePool, course_id:u32, gloss_id:u32, word_id: u32) -> Result<u32, sqlx::Error> {

  //get old values
  //let query = format!("SELECT seq_id,lemma_id,word_id FROM arrowed_words WHERE seq_id = {seq} AND lemma_id={lemma_id};", seq=seq, lemma_id=lemma_id);
  //sqlx::query(&query).execute(pool).await?;
  //save history
  
  let mut tx = pool.begin().await?;

  let query = format!("INSERT INTO arrowed_words_history \
    SELECT NULL,course_id,gloss_id,word_id,updated,user_id,comment FROM arrowed_words WHERE course_id = {course_id} AND gloss_id={gloss_id};", course_id=course_id, gloss_id=gloss_id);
  let r = sqlx::query(&query).execute(&mut tx).await?;

  //println!("rows: {}",r.rows_affected());

  if r.rows_affected() < 1 {
    let query = format!("INSERT INTO arrowed_words_history VALUES ( \
    NULL,{course_id},{gloss_id},NULL,0,NULL,NULL);", course_id=course_id, gloss_id=gloss_id);
  let r = sqlx::query(&query).execute(&mut tx).await?;
  }

  //$arrowedVal = ($_POST['setArrowedIDTo'] < 1) ? "NULL" : $_POST['setArrowedIDTo'] . "";

  if word_id > 0 {
    let query = format!("REPLACE INTO arrowed_words VALUES ({course_id}, {gloss_id}, {word_id},0,NULL,NULL);", course_id=course_id, gloss_id=gloss_id, word_id=word_id);
    sqlx::query(&query).execute(&mut tx).await?;
  }
  else {
    let query = format!("DELETE FROM arrowed_words WHERE course_id = {course_id} AND gloss_id={gloss_id};", course_id=course_id, gloss_id=gloss_id);
    sqlx::query(&query).execute(&mut tx).await?;
  }

  tx.commit().await?;

  //INSERT INTO arrowed_words_history SELECT NULL,seq_id,lemma_id,word_id FROM arrowed_words WHERE seq_id = 1 AND lemma_id=20;

  Ok(1)
/*  
    if ( $conn->query($query) === TRUE)
    {
      $j->success = TRUE;
      $j->affectedRows = $conn->affected_rows;
      $j->arrowedValue = $arrowedVal;
      $j->lemmaid = $_POST['forLemmaID'];
    sendJSON($j);
    }
  }
  */
}

pub async fn set_lemma_id(pool: &SqlitePool, gloss_id:u32, word_id:u32) -> Result<u32, sqlx::Error> {
  let mut tx = pool.begin().await?;
  let course_id = 1;
  let query = format!("SELECT gloss_id FROM words WHERE word_id = {word_id};", word_id=word_id);
  let old_gloss_id:(Option<u32>,) = sqlx::query_as(&query)
  .fetch_one(&mut tx)
  .await?;

  let query = format!("UPDATE words SET gloss_id = {gloss_id} WHERE word_id={word_id};", gloss_id=gloss_id, word_id=word_id);
  sqlx::query(&query).execute(&mut tx).await?;

  update_counts_for_gloss_id(&mut tx, course_id, gloss_id).await;
  if old_gloss_id.0.is_some() {
    update_counts_for_gloss_id(&mut tx, course_id, old_gloss_id.0.unwrap() ).await;
  }

  /*
if ($oldLemmaId !== NULL) {
					updateRunningCount($conn, $_POST['textwordid'], $oldLemmaId);
				}
				updateRunningCount($conn, $_POST['textwordid'], $_POST['lemmaid']);
				$conn->query("COMMIT;");

        				//add AND A.seq between start and stop of page to make it more efficient?
				$query2 = sprintf("SELECT A.hqid as hqid,A.lemma as lemma,A.pos as pos,A.def as def,A.freq as total,B.seq as seq,B.runningcount as rc,B.wordid as wordid,C.seq as lemmaseq,B.isFlagged as flagged FROM %s A LEFT JOIN %s B on A.hqid=B.lemmaid LEFT JOIN %s C on A.arrowedID = C.wordid WHERE A.hqid = %s;", 
					LEMMA_TABLE,
					TEXT_WORDS_TABLE, 
					TEXT_WORDS_TABLE, 
					$_POST['lemmaid'] );

				$res2A = $conn->query($query2);// or die( mysqli_error( $conn ) );
				$words = [];
				if ($res2A) {
					while ($row = mysqli_fetch_array($res2A))
					{
						$a = new \stdClass();
						$a->i = $row["wordid"];//$_POST['textwordid'];
		        		$a->hqid = $row["hqid"];
		        		$a->l = $row["lemma"];
		        		$a->pos = $row["pos"];
		        		$a->g = $row["def"];
		        		$a->rc = $row["rc"];//$runningcount;
		        		$a->ls = $row["lemmaseq"];
		        		$a->fr = $row["total"];
		        		$a->ws = $row["seq"];
		        		$a->fl = $row["flagged"];
		        		array_push($words, $a);
					}
				}
	    		$j->words = $words;
	    		$j->success = TRUE;
	    		$j->affectedRows = $affected;
  */

  tx.commit().await?;
  Ok(1)
}

pub async fn update_counts_all<'a>(tx: &'a mut sqlx::Transaction<'a, sqlx::Sqlite>, course_id:u32) -> Result<u32, sqlx::Error> {
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
  Ok(1)
}

pub async fn update_counts_for_gloss_id<'a,'b>(tx: &'a mut sqlx::Transaction<'b, sqlx::Sqlite>, course_id:u32, gloss_id:u32) -> Result<u32, sqlx::Error> {
  //select count(*) as c,b.lemma from words a inner join glosses b on a.gloss_id=b.gloss_id group by a.gloss_id order by c;
  //REPLACE INTO total_counts_by_course SELECT 1,gloss_id,COUNT(*) FROM words WHERE gloss_id = 3081 GROUP BY gloss_id;
  // to update all total counts
  let query = format!("REPLACE INTO total_counts_by_course \
    SELECT {course_id},gloss_id,COUNT(*) \
    FROM words \
    WHERE gloss_id = {gloss_id} \
    GROUP BY gloss_id;", course_id=course_id, gloss_id=gloss_id);
  sqlx::query(&query).execute(&mut *tx).await?;

  let query = format!("REPLACE INTO running_counts_by_course \
    SELECT {course_id},a.word_id,count(*) AS running_count \
    FROM words a \
    INNER JOIN words b ON a.gloss_id=b.gloss_id \
    INNER JOIN course_x_text c ON a.text = c.text_id \
    INNER JOIN course_x_text d ON b.text = d.text_id \
    WHERE d.text_order <= c.text_order AND b.seq <= a.seq AND a.gloss_id = {gloss_id} \
    GROUP BY a.word_id \
    ORDER BY a.gloss_id, running_count;", course_id=course_id, gloss_id=gloss_id);
  sqlx::query(&query).execute(&mut *tx).await?;

  //to select running counts
  //select a.gloss_id,a.word_id,count(*) as num from words a INNER JOIN words b ON a.gloss_id=b.gloss_id inner join course_x_text c on a.text = c.text_id inner join course_x_text d on b.text = d.text_id where c.text_order <= d.text_order and a.seq <= b.seq and a.gloss_id=4106 group by a.word_id order by a.gloss_id, num;

  //when updating running count of just one we only need to update the words equal and after this one?
  Ok(1)
}

pub async fn get_words(pool: &SqlitePool, text_id:i32) -> Result<Vec<WordRow>, sqlx::Error> {
    let course_id = 1;
    let (start,end) = get_start_end(pool, text_id).await?;

    //need to add joins for the running and total count tables and pull from those
    let query = format!("SELECT A.word_id,A.word,A.type,B.lemma,A.lemma1,B.def,B.unit,pos,D.word_id as arrowedID,B.gloss_id,A.seq,E.seq AS arrowedSeq, \
    I.total_count, H.running_count,A.isFlagged, G.text_order,F.text_order AS arrowed_text_order \
    FROM words A \
    LEFT JOIN glosses B ON A.gloss_id = B.gloss_id \
    LEFT JOIN arrowed_words D ON (A.gloss_id = D.gloss_id AND D.course_id = {course_id}) \
    LEFT JOIN words E ON E.word_id = D.word_id \
    LEFT JOIN course_x_text F ON (E.text = F.text_id AND F.course_id = {course_id}) \
    LEFT JOIN course_x_text G ON (A.text = G.text_id AND G.course_id = {course_id}) \
    LEFT JOIN running_counts_by_course H ON (H.course_id = {course_id} AND H.word_id = A.word_id) \
    LEFT JOIN total_counts_by_course I ON (I.course_id = {course_id} AND I.gloss_id = A.gloss_id) \
    WHERE A.seq >= {start_seq} AND A.seq <= {end_seq} AND A.type > -1 \
    ORDER BY A.seq \
    LIMIT 55000;", start_seq = start, end_seq = end, course_id = course_id);

    let res: Result<Vec<WordRow>, sqlx::Error> = sqlx::query(&query)
    .map(|rec: SqliteRow| 
        WordRow {
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
        }    
    )
    .fetch_all(pool)
    .await;

    res
}

pub async fn get_assignment_rows(pool: &SqlitePool) -> Result<Vec<AssignmentRow>, sqlx::Error> {
  let query = format!("SELECT id,title,wordcount FROM assignments ORDER BY id;");
  let res: Result<Vec<AssignmentRow>, sqlx::Error> = sqlx::query(&query)
  .map(|rec: SqliteRow| AssignmentRow {id: rec.get("id"), assignment: rec.get("title")} )
  .fetch_all(pool)
  .await;

  res
}

pub async fn get_titles(pool: &SqlitePool) -> Result<Vec<(String,u32)>, sqlx::Error> {
    let query = format!("SELECT id,title FROM titles ORDER BY title;");
    let res: Result<Vec<(String,u32)>, sqlx::Error> = sqlx::query(&query)
    .map(|rec: SqliteRow| (rec.get("id"),rec.get("title")) )
    .fetch_all(pool)
    .await;

    res
}

pub async fn get_text_id_for_word_id(pool: &SqlitePool, wordid:i32) -> Result<i32, sqlx::Error> {
  let query = "SELECT A.id FROM assignments A INNER JOIN words B ON A.start = B.word_id INNER JOIN words C ON A.end = C.word_id WHERE B.seq <= (SELECT seq FROM words WHERE word_id = $wordid) AND C.seq >= (SELECT seq FROM words WHERE word_id = $wordid) LIMIT 1;";
  
  let rec: (i32,) = sqlx::query_as(&query)
  .fetch_one(pool)
  .await?; //else 0
  
  Ok(rec.0)
}

pub async fn get_start_end(pool: &SqlitePool, text_id:i32) -> Result<(u32,u32), sqlx::Error> {
  let query = format!("SELECT b.seq, c.seq FROM assignments a INNER JOIN words b ON a.start = b.word_id INNER JOIN words c ON a.end = c.word_id WHERE a.id = {};", text_id);
  
  let rec: (u32,u32) = sqlx::query_as(&query)
  .fetch_one(pool)
  .await?;

  Ok(rec)
}
    
/*
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


