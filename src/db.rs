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

/*
//seq_id, lemma_id, word_id
pub async fn arrow_word(pool: &SqlitePool, textid:i32) -> Result<Vec<WordRow>, sqlx::Error> {

  REPLACE INTO table(column_list) VALUES(value_list);

  let query = format!("UPDATE %s SET arrowedID={word_id}, updatedIP='{ip}', updatedUserAgent='{user_agent}', updatedUser='{user}' WHERE hqid={lemma_id};", 
    start=start,end=end);


if ( isset($_POST['forLemmaID']) && isset($_POST['setArrowedIDTo']) )
{
    $arrowedVal = ($_POST['setArrowedIDTo'] < 1) ? "NULL" : $_POST['setArrowedIDTo'] . "";

    $query = sprintf("UPDATE %s SET arrowedID=%s, updatedIP='%s', updatedUserAgent='%s', updatedUser='%s' WHERE hqid=%s;", 
      LEMMA_TABLE, 
      $arrowedVal . "", 
      $updatedIP, 
      $updatedUserAgent, 
      $updatedUser, 
      $_POST['forLemmaID'] . "" );
    
    if ( $conn->query($query) === TRUE)
    {
      $j->success = TRUE;
      $j->affectedRows = $conn->affected_rows;
      $j->arrowedValue = $arrowedVal;
      $j->lemmaid = $_POST['forLemmaID'];
    sendJSON($j);
    }
  }
}
*/

pub async fn get_words(pool: &SqlitePool, textid:i32) -> Result<Vec<WordRow>, sqlx::Error> {

    let (start,end) = get_start_end(pool, textid).await?;
/*
    let query2 = format!("SELECT A.wordid,A.word,A.type,B.lemma,A.lemma1,B.def,B.unit,pos,B.arrowedID,B.hqid,A.seq,C.seq AS arrowedSeq, \
    B.freq, A.runningcount,A.isFlagged \
    FROM gkvocabdb A \
    LEFT JOIN hqvocab B ON A.lemmaid = B.hqid \
    LEFT JOIN gkvocabdb C on B.arrowedID = C.wordid \
    WHERE A.seq >= {start} AND A.seq <= {end} AND A.type > -1 \
    ORDER BY A.seq \
    LIMIT 55000;", 
    start=start,end=end);
*/

    //need to add joins for the running and total count tables and pull from those
    let query = format!("SELECT A.wordid,A.word,A.type,B.lemma,A.lemma1,B.def,B.unit,pos,D.word_id as arrowedID,B.hqid,A.seq,E.seq AS arrowedSeq, \
    B.freq, A.runningcount,A.isFlagged, G.text_order,F.text_order AS arrowed_text_order \
    FROM gkvocabdb A \
    LEFT JOIN hqvocab B ON A.lemmaid = B.hqid \
    LEFT JOIN arrowed_words D on (A.lemmaid = D.lemma_id AND D.seq_id = {seq_id}) \
    LEFT JOIN gkvocabdb E on E.wordid = D.word_id \
    LEFT JOIN text_sequence_x_text F on (E.text = F.text_id and F.seq_id = {seq_id}) \
    LEFT JOIN text_sequence_x_text G on (A.text = G.text_id and G.seq_id = {seq_id}) \
    LEFT JOIN running_counts_by_sequence H ON (H.seq_id = {seq_id} AND H.word_id = A.wordid) \
    LEFT JOIN total_counts_by_sequence I ON (I.seq_id = {seq_id} AND I.lemma_id = A.lemmaid) \
    WHERE A.seq >= {start} AND A.seq <= {end} AND A.type > -1 \
    ORDER BY A.seq \
    LIMIT 55000;", start = start, end = end, seq_id = 1);

    let res: Result<Vec<WordRow>, sqlx::Error> = sqlx::query(&query)
    .map(|rec: SqliteRow| 
        WordRow {
            wordid: rec.get("wordid"),
            word: rec.get("word"),
            word_type: rec.get("type"),
            lemma: rec.get("lemma"),
            lemma1: rec.get("lemma1"),
            def: rec.get("def"),
            unit: rec.get("unit"),
            pos: rec.get("pos"),
            arrowed_id: rec.get("arrowedID"),
            hqid: rec.get("hqid"),
            seq: rec.get("seq"),
            arrowed_seq: rec.get("arrowedSeq"),
            freq: rec.get("freq"), 
            runningcount: rec.get("runningcount"),
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
  let query = format!("SELECT id,title,wordcount FROM gkvocabAssignments ORDER BY id;");
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
  let query = "SELECT A.id FROM gkvocabAssignments A INNER JOIN gkvocabdb B ON A.start = B.wordid INNER JOIN gkvocabdb C ON A.end = C.wordid WHERE B.seq <= (SELECT seq FROM gkvocabdb WHERE wordid = $wordid) AND C.seq >= (SELECT seq FROM gkvocabdb WHERE wordid = $wordid) LIMIT 1;";
  
  let rec: (i32,) = sqlx::query_as(&query)
  .fetch_one(pool)
  .await?; //else 0
  
  Ok(rec.0)
}

pub async fn get_start_end(pool: &SqlitePool, textid:i32) -> Result<(u32,u32), sqlx::Error> {
  let query = format!("SELECT b.seq, c.seq FROM gkvocabAssignments a INNER JOIN gkvocabdb b ON a.start = b.wordid INNER JOIN gkvocabdb c ON a.end = c.wordid WHERE a.id = {};", textid);
  
  let rec: (u32,u32) = sqlx::query_as(&query)
  .fetch_one(pool)
  .await?;

  Ok(rec)
}
    
/*
CREATE TABLE IF NOT EXISTS text_sequences (seq_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, name text NOT NULL);
CREATE TABLE IF NOT EXISTS texts (text_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, name text NOT NULL);

CREATE TABLE IF NOT EXISTS arrowed_words (seq_id INTEGER NOT NULL REFERENCES text_sequences(seq_id), lemma_id INTEGER NOT NULL REFERENCES hqvocab(hqid), word_id INTEGER NOT NULL REFERENCES gkvocabdb(wordid), PRIMARY KEY(seqid, lemma_id, word_id));
INSERT INTO arrowed_words SELECT 1, hqid, arrowedID from hqvocab where arrowedid is not null;

CREATE TABLE IF NOT EXISTS text_sequence_x_text (seq_id INTEGER NOT NULL REFERENCES text_sequences(seq_id), text_id INTEGER NOT NULL REFERENCES texts(text_id), text_order INTEGER NOT NULL, PRIMARY KEY (seq_id,text_id));

CREATE TABLE IF NOT EXISTS running_counts_by_sequence (seq_id INTEGER NOT NULL REFERENCES text_sequences(seq_id), word_id INTEGER NOT NULL REFERENCES gkvocabdb(wordid), running_count INTEGER, PRIMARY KEY (seq_id,word_id));
CREATE TABLE IF NOT EXISTS total_counts_by_sequence (seq_id INTEGER NOT NULL REFERENCES text_sequences(seq_id), lemma_id INTEGER NOT NULL REFERENCES hqvocab(hqid), total_count INTEGER, PRIMARY KEY (seq_id,lemma_id));
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


