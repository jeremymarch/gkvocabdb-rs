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

/*
vscode find/replace regex to change "...".to_string() to String::from("...")
"([^"]*)".to_string[(][)]
String::from("$1")
*/
#[cfg(feature = "postgres")]
pub mod dbpostgres;
#[cfg(not(feature = "postgres"))]
pub mod dbsqlite;
pub mod export_text;
pub mod import_text;

use argon2::password_hash::SaltString;
use argon2::Algorithm;
use argon2::Argon2;
use argon2::Params;
use argon2::PasswordHash;
use argon2::PasswordHasher;
use argon2::PasswordVerifier;
use argon2::Version;
use chrono::Utc;
use secrecy::ExposeSecret;
use secrecy::Secret;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use tokio::task::spawn_blocking;

#[derive(Clone)]
pub struct Credentials {
    pub username: String,
    pub password: Secret<String>,
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum GlosserError {
    Database(String),
    XmlError(String),
    JsonError(String),
    ImportError(String),
    AuthenticationError,
    UnknownError,
}

#[derive(Debug, Deserialize)]
pub struct LemmatizerRecord {
    pub form: String,
    pub gloss_id: u32,
}

impl std::fmt::Display for GlosserError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            GlosserError::Database(s) => write!(fmt, "GlosserError: database: {}", s),
            GlosserError::XmlError(s) => write!(fmt, "GlosserError: xml: {}", s),
            GlosserError::JsonError(s) => write!(fmt, "GlosserError: json error: {}", s),
            GlosserError::ImportError(s) => write!(fmt, "GlosserError: import error: {}", s),
            GlosserError::AuthenticationError => write!(fmt, "GlosserError: authentication error"),
            GlosserError::UnknownError => write!(fmt, "GlosserError: unknown error"),
        }
    }
}

pub enum UpdateType {
    ArrowWord,
    UnarrowWord,
    NewGloss,
    EditGloss,
    SetGlossId,
    ImportText,
    DeleteGloss,
    AddPageBreak,
    RemovePageBreak,
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
            UpdateType::AddPageBreak | UpdateType::RemovePageBreak => todo!(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct WordRow {
    #[serde(rename(serialize = "i"), rename(deserialize = "i"))]
    pub wordid: u32,
    #[serde(rename(serialize = "w"), rename(deserialize = "w"))]
    pub word: String,
    #[serde(rename(serialize = "t"), rename(deserialize = "t"))]
    pub word_type: u8,
    #[serde(rename(serialize = "l"), rename(deserialize = "l"))]
    pub lemma: Option<String>,
    pub def: Option<String>,
    #[serde(rename(serialize = "u"), rename(deserialize = "u"))]
    pub unit: Option<u8>,
    pub pos: Option<String>,
    #[serde(rename(serialize = "a"), rename(deserialize = "a"))]
    pub arrowed_id: Option<u32>,
    pub hqid: Option<u32>,
    #[serde(rename(serialize = "s"), rename(deserialize = "s"))]
    pub seq: u32,
    #[serde(rename(serialize = "s2"), rename(deserialize = "s2"))]
    pub arrowed_seq: Option<u32>,
    #[serde(rename(serialize = "c"), rename(deserialize = "c"))]
    pub freq: Option<u32>,
    #[serde(rename(serialize = "rc"), rename(deserialize = "rc"))]
    pub runningcount: Option<u32>,
    #[serde(rename(serialize = "if"), rename(deserialize = "if"))]
    pub is_flagged: bool,
    pub word_text_seq: u32,
    pub arrowed_text_seq: Option<u32>,
    pub sort_alpha: Option<String>,
    pub last_word_of_page: bool,
    pub app_crit: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TextWord {
    pub word: String,
    pub word_type: u32,
    pub gloss_id: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssignmentRow {
    pub text_id: u32,
    pub text: String,
    pub container_id: Option<u32>,
    pub course_id: Option<u32>,
    pub container: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GlossOccurrence {
    pub name: String,
    pub word_id: u32,
    pub word: String,
    pub arrowed: Option<u32>,
    pub unit: Option<u32>,
    pub lemma: String,
}

#[derive(Deserialize)]
pub struct ExportRequest {
    pub text_ids: String, //comma separated text_ids "133" or "133,134,135"
}

#[derive(Deserialize)]
pub struct SetGlossRequest {
    pub qtype: String,
    pub word_id: u32,
    pub gloss_id: u32,
}

#[derive(Deserialize)]
pub struct GetGlossRequest {
    pub qtype: String,
    pub lemmaid: u32,
}

#[derive(Deserialize)]
pub struct UpdateGlossRequest {
    pub qtype: String,
    pub hqid: Option<u32>,
    pub lemma: String,
    #[serde(
        rename(serialize = "strippedLemma"),
        rename(deserialize = "strippedLemma")
    )]
    pub stripped_lemma: String,
    pub pos: String,
    pub def: String,
    pub note: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConnectionInfo {
    pub user_id: u32,
    pub timestamp: i64,
    pub ip_address: String,
    pub user_agent: String,
}

#[derive(Deserialize, Serialize)]
pub struct QueryRequest {
    pub text: u32,
    pub wordid: u32,
}

#[derive(Deserialize)]
pub struct WordtreeQueryRequest {
    pub n: u32,
    pub idprefix: String,
    pub x: String,
    #[serde(rename(deserialize = "requestTime"))]
    pub request_time: u64,
    pub page: i32, //can be negative for pages before
    pub mode: String,
    pub query: String, //WordQuery,
    pub lex: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlossEntry {
    pub hqid: u32,
    pub l: String,
    pub pos: String,
    pub g: String,
    pub n: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GetGlossResponse {
    pub success: bool,
    #[serde(
        rename(deserialize = "affectedRows"),
        rename(serialize = "affectedRows")
    )]
    pub affected_rows: u64,
    pub words: Vec<GlossEntry>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct MiscErrorResponse {
    #[serde(rename(serialize = "thisText"), rename(deserialize = "thisText"))]
    pub this_text: u32,
    pub text_name: String,
    pub words: Vec<WordRow>,
    #[serde(rename(serialize = "selectedid"), rename(deserialize = "selectedid"))]
    pub selected_id: Option<u32>,
    pub error: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct AssignmentTree {
    pub i: u32,
    pub col: Vec<String>,
    pub c: Vec<AssignmentTree>,
    pub h: bool,
}

#[derive(Deserialize)]
pub struct WordQuery {
    pub regex: Option<String>,
    pub lexicon: String,
    pub tag_id: Option<u32>,
    pub root_id: Option<u32>,
    pub wordid: Option<String>,
    pub w: String,
}

#[allow(dead_code)]
#[derive(PartialEq)]
pub enum WordType {
    Word = 0,
    Punctuation = 1,
    Speaker = 2,
    Section = 4,
    VerseLine = 5, //for verse #
    ParaWithIndent = 6,
    WorkTitle = 7,
    SectionTitle = 8,
    InlineSpeaker = 9,
    ParaNoIndent = 10,
    PageBreak = 11, //not used: we now use separate table called latex_page_breaks
    Desc = 12,
    InvalidType = 13,
    InlineVerseSpeaker = 14,
    //0 word
    //1 punct
    //2 speaker
    //4 section
    //5 new line for verse #
    //6 new para with indent
    //7 work title
    //8 section title centered
    //9 inline speaker, so 2, but inline
    //10 new para without indent
}

impl WordType {
    pub fn from_i32(num: i32) -> Self {
        match num {
            0 => Self::Word,
            1 => Self::Punctuation,
            2 => Self::Speaker,
            4 => Self::Section,
            5 => Self::VerseLine,
            6 => Self::ParaWithIndent,
            7 => Self::WorkTitle,
            8 => Self::SectionTitle,
            9 => Self::InlineSpeaker,
            10 => Self::ParaNoIndent,
            11 => Self::PageBreak,
            12 => Self::Desc,
            14 => Self::InlineVerseSpeaker,
            _ => Self::InvalidType,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct UpdateGlossResponse {
    pub qtype: String,
    pub success: bool,
    pub affectedrows: u64,
    pub inserted_id: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct WordtreeQueryResponse {
    #[serde(rename(serialize = "selectId"), rename(deserialize = "selectId"))]
    pub select_id: Option<u32>,
    pub error: String,
    pub wtprefix: String,
    pub nocache: u8,
    pub container: String,
    #[serde(rename(serialize = "requestTime"), rename(deserialize = "requestTime"))]
    pub request_time: u64,
    pub page: i32, //can be negative for pages before
    #[serde(rename(serialize = "lastPage"), rename(deserialize = "lastPage"))]
    pub last_page: u8,
    #[serde(rename(serialize = "lastPageUp"), rename(deserialize = "lastPageUp"))]
    pub lastpage_up: u8,
    pub scroll: String,
    pub query: String,
    #[serde(rename(serialize = "arrOptions"), rename(deserialize = "arrOptions"))]
    pub arr_options: Vec<AssignmentTree>, //Vec<(String,u32)>
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ArrowWordResponse {
    pub success: bool,
    #[serde(
        rename(serialize = "affectedRows"),
        rename(deserialize = "affectedRows")
    )]
    pub affected_rows: u32,
    #[serde(
        rename(serialize = "arrowedValue"),
        rename(deserialize = "arrowedValue")
    )]
    pub arrowed_value: u32,
    pub lemmaid: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct UpdateGlossIdResponse {
    pub qtype: String,
    pub words: Vec<SmallWord>,
    pub success: bool,
    pub affectedrows: u32,
}

#[derive(Deserialize)]
pub struct ArrowWordRequest {
    pub qtype: String,
    #[serde(rename(serialize = "forLemmaID"), rename(deserialize = "forLemmaID"))]
    pub for_lemma_id: Option<u32>,
    #[serde(
        rename(serialize = "setArrowedIDTo"),
        rename(deserialize = "setArrowedIDTo")
    )]
    pub set_arrowed_id_to: Option<u32>,

    pub textwordid: Option<u32>,
    pub lemmaid: Option<u32>,
    pub lemmastr: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ImportResponse {
    pub success: bool,
    pub text_id: i32,
    pub words_inserted: u64,
    pub error: String,
}

use async_trait::async_trait;
#[async_trait]
pub trait GlosserDb {
    async fn begin_tx(&self) -> Result<Box<dyn GlosserDbTrx>, GlosserError>;
}

#[async_trait]
pub trait GlosserDbTrx {
    async fn commit_tx(self: Box<Self>) -> Result<(), GlosserError>;
    async fn rollback_tx(self: Box<Self>) -> Result<(), GlosserError>;

    async fn insert_pagebreak(&mut self, word_id: u32) -> Result<(), GlosserError>;

    async fn delete_pagebreak(&mut self, word_id: u32) -> Result<(), GlosserError>;

    async fn load_lemmatizer(&mut self) -> Result<(), GlosserError>;

    async fn insert_word(
        &mut self,
        before_word_id: u32,
        word_type: u32,
        word: &str,
    ) -> Result<i64, GlosserError>;

    async fn insert_lemmatizer_form(
        &mut self,
        form: &str,
        gloss_id: u32,
    ) -> Result<(), GlosserError>;

    async fn get_lemmatizer(&mut self) -> Result<HashMap<String, u32>, GlosserError>;

    async fn get_hqvocab_column(
        &mut self,
        pos: &str,
        lower_unit: u32,
        unit: u32,
        sort: &str,
    ) -> Result<Vec<(String, u32, String)>, GlosserError>;

    async fn arrow_word_trx(
        &mut self,
        course_id: u32,
        gloss_id: u32,
        word_id: u32,
        info: &ConnectionInfo,
    ) -> Result<(), GlosserError>;

    async fn set_gloss_id(
        &mut self,
        course_id: u32,
        gloss_id: u32,
        word_id: u32,
        info: &ConnectionInfo,
    ) -> Result<Vec<SmallWord>, GlosserError>;

    async fn add_text(
        &mut self,
        course_id: u32,
        text_name: &str,
        words: Vec<TextWord>,
        info: &ConnectionInfo,
    ) -> Result<(u64, i32), GlosserError>;

    async fn insert_gloss(
        &mut self,
        gloss: &str,
        pos: &str,
        def: &str,
        stripped_lemma: &str,
        note: &str,
        info: &ConnectionInfo,
    ) -> Result<(i64, u64), GlosserError>;

    async fn update_log_trx(
        &mut self,
        update_type: UpdateType,
        object_id: Option<i64>,
        history_id: Option<i64>,
        course_id: Option<i64>,
        update_desc: &str,
        info: &ConnectionInfo,
    ) -> Result<(), GlosserError>;

    async fn delete_gloss(
        &mut self,
        gloss_id: u32,
        info: &ConnectionInfo,
    ) -> Result<u64, GlosserError>;

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
    ) -> Result<u64, GlosserError>;

    async fn get_words_for_export(
        &mut self,
        text_id: u32,
        course_id: u32,
    ) -> Result<Vec<WordRow>, GlosserError>;

    async fn get_words(
        &mut self,
        text_id: u32,
        course_id: u32,
    ) -> Result<Vec<WordRow>, GlosserError>;

    async fn get_text_name(&mut self, text_id: u32) -> Result<String, GlosserError>;
    async fn get_text_title(&mut self, text_id: u32) -> Result<String, GlosserError>;
    async fn get_sibling_texts(&mut self, text_id: u32) -> Result<Vec<u32>, GlosserError>;

    async fn update_text_order_db(
        &mut self,
        course_id: u32,
        text_id: u32,
        step: i32,
    ) -> Result<(), GlosserError>;

    async fn get_texts_db(&mut self, course_id: u32) -> Result<Vec<AssignmentRow>, GlosserError>;

    async fn get_text_id_for_word_id(&mut self, word_id: u32) -> Result<u32, GlosserError>;

    async fn get_glossdb(&mut self, gloss_id: u32) -> Result<GlossEntry, GlosserError>;

    async fn get_gloss_occurrences(
        &mut self,
        course_id: u32,
        gloss_id: u32,
    ) -> Result<Vec<GlossOccurrence>, GlosserError>;

    async fn get_update_log(&mut self, course_id: u32)
        -> Result<Vec<AssignmentTree>, GlosserError>;

    async fn get_before(
        &mut self,
        searchprefix: &str,
        page: i32,
        limit: u32,
        course_id: u32,
    ) -> Result<Vec<(String, u32, String, u32)>, GlosserError>;

    async fn get_equal_and_after(
        &mut self,
        searchprefix: &str,
        page: i32,
        limit: u32,
        course_id: u32,
    ) -> Result<Vec<(String, u32, String, u32)>, GlosserError>;

    async fn create_user(
        &mut self,
        name: &str,
        initials: &str,
        user_type: u32,
        password: Secret<String>,
        email: &str,
    ) -> Result<i64, GlosserError>;

    async fn get_credentials(
        &mut self,
        username: &str,
    ) -> Result<Option<(u32, Secret<String>)>, GlosserError>;

    async fn create_db(&mut self) -> Result<(), GlosserError>;
}
/*
pub async fn insert_thuc_paras(db: &dyn GlosserDb) {
    println!("insert thuc paras start");
    let a = [
        47401, 47440, 48651, 48994, 49204, 49301, 49674, 49851, 50413, 51110, 51604, 52047, 52174,
        52291, 52527, 52581, 52697, 52730, 53024, 53113, 53266, 53303, 53435, 53679, 53868, 54058,
        54237, 54303, 54531, 54758, 54952, 55190, 55476, 55791, 55862, 56177, 56663, 56948, 57108,
        57278, 57470, 57719, 57809, 57957, 58078, 58320, 58519, 58974, 59302,
    ];
    let mut tx = db.begin_tx().await.unwrap();
    for id in a {
        tx.insert_word(id, 6, "").await.unwrap();
    }
    tx.commit_tx().await.unwrap();
    println!("insert thuc paras success");
}
*/

pub async fn gkv_arrow_word(
    db: &dyn GlosserDb,
    post: &ArrowWordRequest,
    info: &ConnectionInfo,
    course_id: u32,
) -> Result<ArrowWordResponse, GlosserError> {
    let mut tx = db.begin_tx().await?;
    tx.arrow_word_trx(
        course_id,
        post.for_lemma_id.unwrap(),
        post.set_arrowed_id_to.unwrap(),
        info,
    )
    .await?;
    tx.commit_tx().await?;
    Ok(ArrowWordResponse {
        success: true,
        affected_rows: 1,
        arrowed_value: 1,
        lemmaid: 1,
    })
}

pub async fn gkv_update_gloss_id(
    db: &dyn GlosserDb,
    gloss_id: u32,
    text_word_id: u32,
    info: &ConnectionInfo,
    course_id: u32,
) -> Result<UpdateGlossIdResponse, GlosserError> {
    let mut tx = db.begin_tx().await?;
    let words = tx
        .set_gloss_id(course_id, gloss_id, text_word_id, info)
        .await?;
    tx.commit_tx().await?;

    Ok(UpdateGlossIdResponse {
        qtype: String::from("set_gloss"),
        words,
        success: true,
        affectedrows: 1,
    })
}

pub async fn gkv_update_or_add_gloss(
    db: &dyn GlosserDb,
    post: &UpdateGlossRequest,
    info: &ConnectionInfo,
) -> Result<UpdateGlossResponse, GlosserError> {
    match post.qtype.as_str() {
        "newlemma" => {
            let mut tx = db.begin_tx().await?;
            let (inserted_id, rows_affected) = tx
                .insert_gloss(
                    &post.lemma,
                    &post.pos,
                    &post.def,
                    &post.stripped_lemma,
                    &post.note,
                    info,
                )
                .await?;
            tx.commit_tx().await?;

            return Ok(UpdateGlossResponse {
                qtype: post.qtype.to_string(),
                success: true,
                affectedrows: rows_affected,
                inserted_id: Some(inserted_id),
            });
        }
        "editlemma" => {
            if post.hqid.is_some() {
                let mut tx = db.begin_tx().await?;
                let rows_affected = tx
                    .update_gloss(
                        post.hqid.unwrap(),
                        &post.lemma,
                        &post.pos,
                        &post.def,
                        &post.stripped_lemma,
                        &post.note,
                        info,
                    )
                    .await?;
                tx.commit_tx().await?;

                // let id = post.hqid.unwrap();

                return Ok(UpdateGlossResponse {
                    qtype: post.qtype.to_string(),
                    success: true,
                    affectedrows: rows_affected,
                    inserted_id: None, // Some(id.into()),  //change to affected_id
                });
            }
        }
        "deletegloss" => {
            if post.hqid.is_some() {
                let mut tx = db.begin_tx().await?;
                let rows_affected = tx.delete_gloss(post.hqid.unwrap(), info).await?;
                tx.commit_tx().await?;

                // let id = post.hqid.unwrap();

                return Ok(UpdateGlossResponse {
                    qtype: post.qtype.to_string(),
                    success: true,
                    affectedrows: rows_affected,
                    inserted_id: None, // Some(id.into()),  //change to affected_id
                });
            }
        }
        _ => {
            return Ok(UpdateGlossResponse {
                qtype: post.qtype.to_string(),
                success: false,
                affectedrows: 0,
                inserted_id: None,
            })
        }
    }
    Ok(UpdateGlossResponse {
        qtype: post.qtype.to_string(),
        success: false,
        affectedrows: 0,
        inserted_id: None,
    })
}

pub async fn gkv_get_gloss(
    db: &dyn GlosserDb,
    post: &GetGlossRequest,
) -> Result<GetGlossResponse, GlosserError> {
    let mut tx = db.begin_tx().await?;
    let gloss = tx.get_glossdb(post.lemmaid).await?;
    tx.commit_tx().await?;

    /*
    $a = new \stdClass();
    $a->hqid = $row[0];
    $a->l = $row[1];
    $a->pos = $row[2];
    $a->g = $row[3];
    $a->n = $row[4];
    array_push($words, $a);
    */
    Ok(GetGlossResponse {
        success: true,
        affected_rows: 0,
        words: vec![gloss],
    })
}

pub async fn gkv_get_glosses(
    db: &dyn GlosserDb,
    info: &WordtreeQueryRequest,
    course_id: u32,
) -> Result<WordtreeQueryResponse, GlosserError> {
    let query_params: WordQuery = serde_json::from_str(&info.query).map_err(map_json_error)?;

    //let seq = get_seq_by_prefix(db, table, &query_params.w).await?;

    let mut tx = db.begin_tx().await?;

    let mut before_rows = vec![];
    let mut after_rows = vec![];
    if info.page <= 0 {
        before_rows = tx
            .get_before(&query_params.w, info.page, info.n, course_id)
            .await?;
        if info.page == 0 {
            //only reverse if page 0. if < 0, each row is inserted under top of container one-by-one in order
            before_rows.reverse();
        }
    }
    if info.page >= 0 {
        after_rows = tx
            .get_equal_and_after(&query_params.w, info.page, info.n, course_id)
            .await?;
    }
    tx.commit_tx().await?;

    //only check page 0 or page less than 0
    let vlast_page_up = u8::from(before_rows.len() < info.n as usize && info.page <= 0);
    //only check page 0 or page greater than 0
    let vlast_page = u8::from(after_rows.len() < info.n as usize && info.page >= 0);

    let seq = if !after_rows.is_empty() {
        after_rows[0].1
    } else {
        0
    };

    let result_rows = [before_rows, after_rows].concat();

    //strip any numbers from end of string
    //let re = Regex::new(r"[0-9]").unwrap();
    let result_rows_stripped: Vec<(String, u32)> = result_rows
        .into_iter()
        .map(|mut row| {
            row.0 = format!("<b>{}</b> {} <a class='listfrequency' href='javascript:showGlossOccurrencesList({})'>({})</a>",
                row.0, row.2, if row.3 > 0 { row.1 } else { 0 /* set to 0 if count is 0 */ }, row.3);
            (row.0, row.1)
        })
        .collect();

    let mut gloss_rows: Vec<AssignmentTree> = vec![];
    for r in &result_rows_stripped {
        gloss_rows.push(AssignmentTree {
            i: r.1,
            col: vec![r.0.clone(), r.1.to_string()],
            h: false,
            c: vec![],
        });
    }

    Ok(WordtreeQueryResponse {
        select_id: Some(seq),
        error: "".to_owned(),
        wtprefix: info.idprefix.clone(),
        nocache: u8::from(query_params.wordid.is_some()), //prevents caching when queried by wordid in url
        container: format!("{}Container", info.idprefix),
        request_time: info.request_time,
        page: info.page,
        last_page: vlast_page,
        lastpage_up: vlast_page_up,
        scroll: if query_params.w.is_empty() && info.page == 0 && seq == 1 {
            String::from("top")
        } else {
            String::from("")
        }, //scroll really only needs to return top
        query: query_params.w.to_owned(),
        arr_options: gloss_rows, //result_rows_stripped//result_rows
    })
}

pub async fn gkv_get_occurrences(
    db: &dyn GlosserDb,
    info: &WordtreeQueryRequest,
    course_id: u32,
) -> Result<WordtreeQueryResponse, GlosserError> {
    let query_params: WordQuery = serde_json::from_str(&info.query).map_err(map_json_error)?;

    //only check page 0 or page less than 0
    let vlast_page_up = 1;
    //only check page 0 or page greater than 0
    let vlast_page = 1;

    let gloss_id = query_params.tag_id.unwrap_or(0);
    let mut tx = db.begin_tx().await?;
    let result_rows = tx.get_gloss_occurrences(course_id, gloss_id).await?;
    tx.commit_tx().await?;

    //start numbering at 0 if H&Q, so running_count is correct
    let start_idx =
        usize::from(result_rows.is_empty() || !result_rows[0].name.starts_with("H&Q Unit"));

    let result_rows_formatted: Vec<(String, u32)> = result_rows
        .into_iter()
        .enumerate()
        .map(|(i, mut row)| {
            row.name = format!(
                "{}. <b class='occurrencesarrow'>{}</b> {} - {}",
                i + start_idx,
                if row.arrowed.is_some() { "→" } else { "" },
                row.name,
                row.word
            );
            (row.name, row.word_id)
        })
        .collect();

    let mut gloss_rows: Vec<AssignmentTree> = vec![];
    for r in &result_rows_formatted {
        gloss_rows.push(AssignmentTree {
            i: r.1,
            col: vec![r.0.clone(), r.1.to_string()],
            h: false,
            c: vec![],
        });
    }

    Ok(WordtreeQueryResponse {
        select_id: None,
        error: "".to_owned(),
        wtprefix: info.idprefix.clone(),
        nocache: 1, //prevents caching when queried by wordid in url
        container: format!("{}Container", info.idprefix),
        request_time: info.request_time,
        page: info.page,
        last_page: vlast_page,
        lastpage_up: vlast_page_up,
        scroll: String::from("top"), //scroll really only needs to return top
        query: query_params.w.to_owned(),
        arr_options: gloss_rows, //result_rows_stripped//result_rows
    })
}

pub async fn gkv_update_log(
    db: &dyn GlosserDb,
    info: &WordtreeQueryRequest,
    course_id: u32,
) -> Result<WordtreeQueryResponse, GlosserError> {
    let query_params: WordQuery = serde_json::from_str(&info.query).map_err(map_json_error)?;

    let mut tx = db.begin_tx().await?;
    let log = tx.get_update_log(course_id).await?;
    tx.commit_tx().await?;

    Ok(WordtreeQueryResponse {
        select_id: None,
        error: "".to_owned(),
        wtprefix: info.idprefix.clone(),
        nocache: 1, //prevents caching when queried by wordid in url
        container: format!("{}Container", info.idprefix),
        request_time: info.request_time,
        page: info.page,
        last_page: 1,
        lastpage_up: 1,
        scroll: String::from("top"),
        query: query_params.w.to_owned(),
        arr_options: log,
    })
}

pub async fn gkv_get_texts(
    db: &dyn GlosserDb,
    info: &WordtreeQueryRequest,
    course_id: u32,
) -> Result<WordtreeQueryResponse, GlosserError> {
    let query_params: WordQuery = serde_json::from_str(&info.query).map_err(map_json_error)?;

    //let seq = get_seq_by_prefix(db, table, &query_params.w).await?;

    //only check page 0 or page less than 0
    let vlast_page_up = 1;
    //only check page 0 or page greater than 0
    let vlast_page = 1;

    let seq = 0;

    //let result_rows = [before_rows, after_rows].concat();

    //strip any numbers from end of string
    //let re = Regex::new(r"[0-9]").unwrap();
    //let result_rows_stripped:Vec<TreeRow> = vec![TreeRow{v:String::from("abc"), i:1, c:None}, TreeRow{v:String::from("def"), i:2, c:Some(vec![TreeRow{v:String::from("def2"), i:1, c:None}, TreeRow{v:String::from("def3"), i:3, c:None}])}];
    let mut tx = db.begin_tx().await?;
    let w = tx.get_texts_db(course_id).await?;
    tx.commit_tx().await?;

    let mut assignment_rows: Vec<AssignmentTree> = vec![];
    let mut last_container_id: i64 = -1;

    let use_containers = false;

    for r in &w {
        if use_containers {
            if r.container_id.is_some()
                && r.container.is_some()
                && r.container_id.unwrap() != last_container_id as u32
            {
                last_container_id = r.container_id.unwrap() as i64;
                //add container
                let mut a = AssignmentTree {
                    i: r.container_id.unwrap(),
                    col: vec![
                        r.container.as_ref().unwrap().clone(),
                        r.container_id.unwrap().to_string(),
                    ],
                    h: false,
                    c: vec![],
                };
                //container's children
                for r2 in &w {
                    if r2.container_id.is_some() && r2.container_id.unwrap() == a.i {
                        a.h = true;
                        a.c.push(AssignmentTree {
                            i: r2.text_id,
                            col: vec![r2.text.clone(), r2.text_id.to_string()],
                            h: false,
                            c: vec![],
                        });
                    }
                }
                assignment_rows.push(a);
            }
            //texts without containers
            else if r.container_id.is_none() {
                let a = AssignmentTree {
                    i: r.text_id,
                    col: vec![r.text.clone(), r.text_id.to_string()],
                    h: false,
                    c: vec![],
                };
                assignment_rows.push(a);
            }
        } else {
            let a = AssignmentTree {
                i: r.text_id,
                col: vec![r.text.clone(), r.text_id.to_string()],
                h: false,
                c: vec![],
            };
            assignment_rows.push(a);
        }
    }

    Ok(WordtreeQueryResponse {
        select_id: Some(seq),
        error: "".to_owned(),
        wtprefix: info.idprefix.clone(),
        nocache: u8::from(query_params.wordid.is_some()), //prevents caching when queried by wordid in url
        container: format!("{}Container", info.idprefix),
        request_time: info.request_time,
        page: info.page,
        last_page: vlast_page,
        lastpage_up: vlast_page_up,
        scroll: if query_params.w.is_empty() && info.page == 0 && seq == 1 {
            String::from("top")
        } else {
            String::from("")
        }, //scroll really only needs to return top
        query: query_params.w.to_owned(),
        arr_options: assignment_rows, //result_rows_stripped//result_rows
    })
}

pub async fn gkv_move_text(
    db: &dyn GlosserDb,
    text_id: u32,
    step: i32,
    _info: &ConnectionInfo,
    course_id: u32,
) -> Result<(), GlosserError> {
    let mut tx = db.begin_tx().await?;
    let res = tx.update_text_order_db(course_id, text_id, step).await;
    tx.commit_tx().await?;
    res
}

pub async fn gkv_get_text_words(
    db: &dyn GlosserDb,
    info: &QueryRequest,
    selected_word_id: Option<u32>,
    course_id: u32,
) -> Result<MiscErrorResponse, GlosserError> {
    let mut tx = db.begin_tx().await?;

    //let query_params: WordQuery = serde_json::from_str(&info.query)?;

    let text_id = match info.wordid {
        0 => info.text,
        _ => tx.get_text_id_for_word_id(info.wordid).await?,
    };

    let w = tx.get_words(text_id, course_id).await?;

    let text_name = tx.get_text_name(text_id).await?;
    tx.commit_tx().await?;
    /*
        $j = new \stdClass();
        if ($words == "WordAssignmentError" ) {
            $j->error = "Error getting text assignments.";
        }
        $j->thisText = $textid;
        $j->words = $words;
        $j->selectedid = $selectedid;
    */

    //{"thisText":"1","words":[{"i":"1","w":"530a","t":"4","l":null,"pos":null,"l1":"","def":null,"u":null,"a":null,"hqid":null,"s":"1","s2":null,"c":null,"rc":"0","if":"0"},
    //{"i":"2","w":"ΣΩ.","t":"2","l":null,"pos":null,"l1":"","def":null,"u":null,"a":null,"hqid":null,"s":"2","s2":null,"c":null,"rc":"0","if":"0"}],"selectedid":0}

    Ok(MiscErrorResponse {
        this_text: text_id,
        text_name,
        words: w,
        selected_id: selected_word_id,
        error: String::from(""),
    })
}

pub async fn gkv_create_db(db: &dyn GlosserDb) -> Result<(), GlosserError> {
    let mut tx = db.begin_tx().await?;
    tx.create_db().await?;
    tx.commit_tx().await?;
    Ok(())
}

pub async fn gkv_create_user(
    db: &dyn GlosserDb,
    name: &str,
    initials: &str,
    user_type: u32,
    password: &str,
    email: &str,
) -> Result<u32, GlosserError> {
    if name.len() < 2
        || name.len() > 30
        || password.len() < 8
        || password.len() > 60
        || email.len() < 6
        || email.len() > 120
    {
        return Err(GlosserError::UnknownError);
    }

    let initials_upper = initials.to_uppercase(); //uppercase to enforce unique regardless of case

    let secret_password = Secret::new(password.to_string());

    let password_hash = spawn_blocking(move || compute_password_hash(secret_password))
        .await
        .map_err(|_| GlosserError::AuthenticationError)??;

    let mut tx = db.begin_tx().await?;
    let user_id = tx
        .create_user(name, &initials_upper, user_type, password_hash, email)
        .await?;
    tx.commit_tx().await?;
    Ok(user_id.try_into().unwrap())
}

fn compute_password_hash(password: Secret<String>) -> Result<Secret<String>, GlosserError> {
    let salt = SaltString::generate(&mut rand::thread_rng());
    let password_hash = Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(15000, 2, 1, None).unwrap(),
    )
    .hash_password(password.expose_secret().as_bytes(), &salt);

    match password_hash {
        Ok(p) => Ok(Secret::new(p.to_string())),
        Err(_e) => Err(GlosserError::AuthenticationError),
    }
}

pub async fn gkv_validate_credentials(
    db: &dyn GlosserDb,
    credentials: Credentials,
) -> Result<u32, GlosserError> {
    let mut user_id = None;
    let mut expected_password_hash = Secret::new(
        "$argon2id$v=19$m=15000,t=2,p=1$\
        gZiV/M1gPc22ElAH/Jh1Hw$\
        CWOrkoo7oJBQ/iyh7uJ0LO2aLEfrHwTWllSAxT0zRno"
            .to_string(),
    );

    let mut tx = db.begin_tx().await?;
    if let Some((stored_user_id, stored_password_hash)) = tx
        .get_credentials(&credentials.username.to_uppercase())
        .await?
    {
        user_id = Some(stored_user_id);
        expected_password_hash = stored_password_hash;
    }
    tx.commit_tx().await?;

    spawn_blocking(move || {
        verify_password_hash(expected_password_hash, &credentials.password) //this will error and return if password does not match
    })
    .await
    .map_err(|_| GlosserError::AuthenticationError)??;

    match user_id {
        Some(id) => Ok(id),
        _ => Err(GlosserError::AuthenticationError),
    }
}

fn verify_password_hash(
    expected_password_hash: Secret<String>,
    password_candidate: &Secret<String>,
) -> Result<(), GlosserError> {
    let expected_password_hash = PasswordHash::new(expected_password_hash.expose_secret());
    match expected_password_hash {
        Ok(p) => Argon2::default()
            .verify_password(password_candidate.expose_secret().as_bytes(), &p)
            .map_err(|_| GlosserError::AuthenticationError),
        Err(_) => Err(GlosserError::AuthenticationError),
    }
}

pub fn get_timestamp() -> i64 {
    let now = Utc::now();
    now.timestamp()
}

fn map_json_error(e: serde_json::Error) -> GlosserError {
    GlosserError::JsonError(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[cfg(not(feature = "postgres"))]
    use crate::dbsqlite::GlosserDbSqlite;
    #[cfg(not(feature = "postgres"))]
    use sqlx::sqlite::SqliteConnectOptions;
    #[cfg(not(feature = "postgres"))]
    use sqlx::SqlitePool;
    #[cfg(not(feature = "postgres"))]
    use std::str::FromStr;

    #[cfg(feature = "postgres")]
    use crate::dbpostgres::GlosserDbPostgres;
    #[cfg(feature = "postgres")]
    use sqlx::postgres::PgPoolOptions;

    #[cfg(not(feature = "postgres"))]
    async fn set_up() -> (GlosserDbSqlite, ConnectionInfo) {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("Could not connect to db.")
            .foreign_keys(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .read_only(false)
            .collation("PolytonicGreek", |l, r| {
                l.to_lowercase().cmp(&r.to_lowercase())
            });
        let db = GlosserDbSqlite {
            db: SqlitePool::connect_with(options)
                .await
                .expect("Could not connect to db."),
        };

        gkv_create_db(&db).await.expect("Could not create db.");

        let user_id = gkv_create_user(&db, "testuser", "tu", 0, "12341234", "tu@blah.com")
            .await
            .unwrap();

        let info = ConnectionInfo {
            user_id,
            timestamp: get_timestamp(),
            ip_address: String::from("0.0.0.0"),
            user_agent: String::from("test_agent"),
        };
        (db, info)
    }

    #[cfg(feature = "postgres")]
    async fn set_up() -> (GlosserDbPostgres, ConnectionInfo) {
        let db_string = "postgres://jwm:1234@localhost/gkvocabdb";

        let db = GlosserDbPostgres {
            db: PgPoolOptions::new()
                .max_connections(5)
                .connect(db_string)
                .await
                .expect("Could not connect to db."),
        };

        let query = r#"DROP DATABASE IF EXISTS gkvocabdbtesting;"#;
        let _res = sqlx::query(query).execute(&db.db).await.unwrap();
        let query = r#"CREATE DATABASE gkvocabdbtesting;"#;
        let _res = sqlx::query(query).execute(&db.db).await.unwrap();
        db.db.close().await;

        let db_string = "postgres://jwm:1234@localhost/gkvocabdbtesting";
        let db = GlosserDbPostgres {
            db: PgPoolOptions::new()
                .max_connections(5)
                .connect(db_string)
                .await
                .expect("Could not connect to db."),
        };

        gkv_create_db(&db).await.expect("Could not create db.");

        let user_id = gkv_create_user(&db, "testuser", "tu", 0, "12341234", "tu@blah.com")
            .await
            .unwrap();

        let info = ConnectionInfo {
            user_id,
            timestamp: get_timestamp(),
            ip_address: String::from("0.0.0.0"),
            user_agent: String::from("test_agent"),
        };
        (db, info)
    }

    async fn setup_text_test(
        db: &dyn GlosserDb,
        course_id: u32,
        user_info: &ConnectionInfo,
    ) -> ImportResponse {
        let title = "testingtext";

        let xml_string = r#"<TEI.2>
            <text lang="greek">
                <head>Θύρσις ἢ ᾠδή</head>
                <speaker>Θύρσις</speaker>
                <lb rend="displayNum" n="5" />αἴκα δ᾽ αἶγα λάβῃ τῆνος γέρας, ἐς τὲ καταρρεῖ
                <pb/>
                <l n="10">ὁσίου γὰρ ἀνδρὸς ὅσιος ὢν ἐτύγχανον</l>
                <desc>This is a test.</desc>
                γὰρ
            </text>
        </TEI.2>"#;

        //add fake glosses so the auto-glossing passes foreign key constraints
        for _n in 1..31 {
            let post = UpdateGlossRequest {
                qtype: String::from("newlemma"),
                hqid: None,
                lemma: String::from("newword"),
                stripped_lemma: String::from("newword"),
                pos: String::from("newpos"),
                def: String::from("newdef"),
                note: String::from("newnote"),
            };

            let _ = gkv_update_or_add_gloss(db, &post, user_info).await;
        }

        import_text::gkv_import_text(db, course_id, user_info, title, xml_string)
            .await
            .unwrap()
    }

    async fn setup_small_text_test(
        db: &dyn GlosserDb,
        course_id: u32,
        user_info: &ConnectionInfo,
    ) -> ImportResponse {
        let title = "testingtext2";

        let xml_string = r#"<TEI.2>
            <text lang="greek">
                ὁσίου γὰρ ὅσιος
            </text>
        </TEI.2>"#;

        //add fake glosses so the auto-glossing passes foreign key constraints
        for _n in 1..2 {
            let post = UpdateGlossRequest {
                qtype: String::from("newlemma"),
                hqid: None,
                lemma: String::from("newword"),
                stripped_lemma: String::from("newword"),
                pos: String::from("newpos"),
                def: String::from("newdef"),
                note: String::from("newnote"),
            };
            let _ = gkv_update_or_add_gloss(db, &post, user_info).await;
        }

        import_text::gkv_import_text(db, course_id, user_info, title, xml_string)
            .await
            .unwrap()
    }

    #[tokio::test]
    #[serial]
    async fn test_hqvocab_query() {
        let (db, _user_info) = set_up().await;
        let mut tx = db.begin_tx().await.unwrap();
        let lower_unit = 1;
        let unit = 20;

        for p in ["noun", "verb", "adjective", "other"] {
            for sort in ["unit", "alpha"] {
                let res = tx.get_hqvocab_column(p, lower_unit, unit, sort).await;
                assert!(res.is_ok());
            }
        }
        tx.rollback_tx().await.unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_delete_gloss() {
        let (db, user_info) = set_up().await;

        let post = UpdateGlossRequest {
            qtype: String::from("newlemma"),
            hqid: None,
            lemma: String::from("newword"),
            stripped_lemma: String::from("newword"),
            pos: String::from("newpos"),
            def: String::from("newdef"),
            note: String::from("newnote"),
        };
        let res = gkv_update_or_add_gloss(&db, &post, &user_info).await;
        assert!(res.is_ok());

        let gloss_id = res.unwrap().inserted_id.unwrap();

        let post = UpdateGlossRequest {
            qtype: String::from("deletegloss"),
            hqid: Some(gloss_id as u32),
            lemma: String::from(""),
            stripped_lemma: String::from(""),
            pos: String::from(""),
            def: String::from(""),
            note: String::from(""),
        };

        let res = gkv_update_or_add_gloss(&db, &post, &user_info).await;
        assert!(res.is_ok());
        assert_eq!(res.unwrap().affectedrows, 1);
    }

    #[tokio::test]
    #[serial]
    async fn test_basic_export() {
        let (db, user_info) = set_up().await;
        let course_id = 1;
        let text1_res = setup_small_text_test(&db, course_id, &user_info).await;
        let text2_res = setup_small_text_test(&db, course_id, &user_info).await;

        let text_ids_to_export = format!("{},{}", text1_res.text_id, text2_res.text_id);
        let bold_glosses = true;
        let res = export_text::gkv_export_texts_as_latex(
            &db,
            &text_ids_to_export,
            course_id,
            bold_glosses,
        )
        .await;

        assert!(res.is_ok());
    }

    #[tokio::test]
    #[serial]
    async fn test_basic_select_glosses() {
        let (db, _user_info) = set_up().await;

        let timestamp = 1667191605; //get_timestamp().try_into().unwrap(),
        let info = WordtreeQueryRequest {
            n: 101,
            idprefix: String::from("test1"),
            x: String::from("0.2813670904164459"),
            request_time: timestamp,
            page: 0, //can be negative for pages before
            mode: String::from("context"),
            query: r#"{"lexicon":"hqvocab","mode":"normal","w":""}"#.to_string(), //WordQuery,
            lex: Some(String::from("hqvocab")),
        };
        let course_id = 1;
        let res = gkv_get_glosses(&db, &info, course_id).await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    #[serial]
    async fn test_login() {
        let (db, _user_info) = set_up().await;

        let user_id = gkv_create_user(&db, "testuser9", "jm", 0, "abcdabcd", "user1@blah.com")
            .await
            .unwrap();

        //failing credentials
        let credentials = Credentials {
            username: String::from("jm"),
            password: Secret::new("abcdabcdx".to_string()),
        };
        let res = gkv_validate_credentials(&db, credentials).await;
        assert_eq!(res, Err(GlosserError::AuthenticationError));

        //passing credentials
        let credentials = Credentials {
            username: String::from("jm"),
            password: Secret::new("abcdabcd".to_string()),
        };
        let res = gkv_validate_credentials(&db, credentials).await;
        assert_eq!(res.unwrap(), user_id);
    }

    #[tokio::test]
    #[serial]
    async fn import_basic_text() {
        let (db, user_info) = set_up().await;
        let course_id = 1;

        //empty title fails
        let title = "";
        let xml_string = "<TEI.2><text>blahblah</text></TEI.2>";
        let res = import_text::gkv_import_text(&db, course_id, &user_info, title, xml_string).await;
        assert!(res.is_err());

        //empty title xml fails
        let xml_string = "";
        let res = import_text::gkv_import_text(&db, course_id, &user_info, title, xml_string).await;
        assert!(res.is_err());

        let title = "testtext";

        //no TEI or TEI.2 tags
        let xml_string = "<TE><text>blahblah</text></TE>";
        let res = import_text::gkv_import_text(&db, course_id, &user_info, title, xml_string).await;
        assert!(res.is_err());

        //xml has tags, but no text fails
        let xml_string = "<TEI.2><text></text></TEI.2>";
        let res = import_text::gkv_import_text(&db, course_id, &user_info, title, xml_string).await;
        assert!(res.is_err());

        //pass with TEI.2
        let xml_string = "<TEI.2><text>blahblah</text></TEI.2>";
        let res = import_text::gkv_import_text(&db, course_id, &user_info, title, xml_string)
            .await
            .unwrap();
        assert!(res.success);

        //pass with TEI
        let xml_string = "<TEI><text>blahblah</text></TEI>";
        let res = import_text::gkv_import_text(&db, course_id, &user_info, title, xml_string)
            .await
            .unwrap();
        assert!(res.success);

        let res = setup_text_test(&db, course_id, &user_info).await;
        assert!(res.success);
    }

    #[tokio::test]
    #[serial]
    async fn lemmatizer_test() {
        let (db, user_info) = set_up().await;
        let course_id = 1;

        //test inserting gloss
        //insert gloss before adding it to the lemmatizer because of foreign key
        let post = UpdateGlossRequest {
            qtype: String::from("newlemma"),
            hqid: None,
            lemma: String::from("newword"),
            stripped_lemma: String::from("newword"),
            pos: String::from("newpos"),
            def: String::from("newdef"),
            note: String::from("newnote"),
        };
        let res = gkv_update_or_add_gloss(&db, &post, &user_info).await;
        assert!(res.is_ok());

        //test updating gloss
        let inserted_id: u32 = res.unwrap().inserted_id.unwrap().try_into().unwrap();
        let post = UpdateGlossRequest {
            qtype: String::from("newlemma"),
            hqid: Some(inserted_id),
            lemma: String::from("newwordnew"),
            stripped_lemma: String::from("newword"),
            pos: String::from("newpos"),
            def: String::from("newdef"),
            note: String::from("newnote"),
        };
        let res = gkv_update_or_add_gloss(&db, &post, &user_info).await;
        assert!(res.is_ok());

        //add to lemmatizer
        let mut tx = db.begin_tx().await.unwrap();
        tx.insert_lemmatizer_form("ὥστε", 1).await.unwrap();
        tx.commit_tx().await.unwrap();

        let title = "title";
        let xml_string = "<TEI.2><text>blah ὥστε δὲ</text></TEI.2>";
        let res = import_text::gkv_import_text(&db, course_id, &user_info, title, xml_string)
            .await
            .unwrap();
        assert!(res.success);

        //check gkv_get_text_words
        let info = QueryRequest { text: 1, wordid: 0 };
        let selected_word_id = None;
        let res = gkv_get_text_words(&db, &info, selected_word_id, course_id).await;

        assert_eq!(
            res.unwrap(),
            MiscErrorResponse {
                this_text: 1,
                text_name: String::from("title"),
                words: [
                    WordRow {
                        wordid: 1,
                        word: String::from("blah"),
                        word_type: 0,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 1,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 2,
                        word: String::from("ὥστε"),
                        word_type: 0,
                        lemma: Some(String::from("newword")),
                        def: Some(String::from("newdef")),
                        unit: Some(0),
                        pos: Some(String::from("newpos")),
                        arrowed_id: None,
                        hqid: Some(1),
                        seq: 2,
                        arrowed_seq: None,
                        freq: Some(1),
                        runningcount: Some(1),
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: Some(String::from("newword")),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 3,
                        word: String::from("δὲ"),
                        word_type: 0,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 3,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    }
                ]
                .to_vec(),
                selected_id: None,
                error: String::from("")
            }
        );
    }

    #[tokio::test]
    #[serial]
    async fn arrow_word() {
        let (db, user_info) = set_up().await;
        let course_id = 1;

        let res = setup_text_test(&db, course_id, &user_info).await;
        assert!(res.success);

        // let info = QueryRequest {
        //     text: 1,
        //     wordid: 0,
        // };
        //let selected_word_id = None;
        //let res = gkv_get_text_words(&db, &info, selected_word_id, course_id).await;
        //println!("words: {:?}", res);

        let post = ArrowWordRequest {
            qtype: String::from("arrowWord"),
            for_lemma_id: Some(30),     //gloss_id
            set_arrowed_id_to: Some(5), //word_id
            textwordid: None,
            lemmaid: None,
            lemmastr: None,
        };

        let res = gkv_arrow_word(&db, &post, &user_info, course_id).await;
        assert_eq!(
            res.unwrap(),
            ArrowWordResponse {
                success: true,
                affected_rows: 1,
                arrowed_value: 1,
                lemmaid: 1
            }
        );
        //println!("arrow: {:?}", res);

        // let res = gkv_get_text_words(&db, &info, selected_word_id, course_id).await;
        // println!("words: {:?}", res);
    }

    #[tokio::test]
    #[serial]
    async fn arrow_word2() {
        let (db, user_info) = set_up().await;
        let course_id = 1;

        let res = setup_text_test(&db, course_id, &user_info).await;
        assert!(res.success);

        //insert gloss
        let post = UpdateGlossRequest {
            qtype: String::from("newlemma"),
            hqid: None,
            lemma: String::from("newword"),
            stripped_lemma: String::from("newword"),
            pos: String::from("newpos"),
            def: String::from("newdef"),
            note: String::from("newnote"),
        };
        let res = gkv_update_or_add_gloss(&db, &post, &user_info).await;
        //println!("words: {:?}", res);

        let gloss_id: u32 = res
            .as_ref()
            .unwrap()
            .inserted_id
            .unwrap()
            .try_into()
            .unwrap();

        //set_gloss on word
        let post = SetGlossRequest {
            qtype: String::from("set_gloss"),
            word_id: 17,
            gloss_id,
        };
        let res =
            gkv_update_gloss_id(&db, post.gloss_id, post.word_id, &user_info, course_id).await;
        //println!("arrow: {:?}", res);
        assert_eq!(
            res.unwrap(),
            UpdateGlossIdResponse {
                qtype: String::from("set_gloss"),
                words: [SmallWord {
                    wordid: 17,
                    hqid: gloss_id,
                    lemma: String::from("newword"),
                    pos: String::from("newpos"),
                    def: String::from("newdef"),
                    runningcount: Some(1),
                    arrowed_seq: None,
                    total: Some(1),
                    seq: 17,
                    is_flagged: false,
                    word_text_seq: 1,
                    arrowed_text_seq: None
                }]
                .to_vec(),
                success: true,
                affectedrows: 1
            }
        );

        let post = SetGlossRequest {
            qtype: String::from("set_gloss"),
            word_id: 20,
            gloss_id,
        };
        let res =
            gkv_update_gloss_id(&db, post.gloss_id, post.word_id, &user_info, course_id).await;
        //println!("arrow: {:?}", res);
        assert_eq!(
            res.unwrap(),
            UpdateGlossIdResponse {
                qtype: String::from("set_gloss"),
                words: [
                    SmallWord {
                        wordid: 17,
                        hqid: gloss_id,
                        lemma: String::from("newword"),
                        pos: String::from("newpos"),
                        def: String::from("newdef"),
                        runningcount: Some(1),
                        arrowed_seq: None,
                        total: Some(2),
                        seq: 17,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None
                    },
                    SmallWord {
                        wordid: 20,
                        hqid: gloss_id,
                        lemma: String::from("newword"),
                        pos: String::from("newpos"),
                        def: String::from("newdef"),
                        runningcount: Some(2),
                        arrowed_seq: None,
                        total: Some(2),
                        seq: 20,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None
                    }
                ]
                .to_vec(),
                success: true,
                affectedrows: 1
            }
        );

        //arrow word
        let post = ArrowWordRequest {
            qtype: String::from("arrowWord"),
            for_lemma_id: Some(gloss_id), //gloss_id
            set_arrowed_id_to: Some(17),  //word_id
            textwordid: None,
            lemmaid: None,
            lemmastr: None,
        };

        let res = gkv_arrow_word(&db, &post, &user_info, course_id).await;
        assert_eq!(
            res.unwrap(),
            ArrowWordResponse {
                success: true,
                affected_rows: 1,
                arrowed_value: 1,
                lemmaid: 1
            }
        );

        //check gkv_get_text_words
        let info = QueryRequest { text: 1, wordid: 0 };
        let selected_word_id = None;
        let res = gkv_get_text_words(&db, &info, selected_word_id, course_id).await;

        assert_eq!(
            res.unwrap(),
            MiscErrorResponse {
                this_text: 1,
                text_name: String::from("testingtext"),
                words: [
                    WordRow {
                        wordid: 1,
                        word: String::from("Θύρσις ἢ ᾠδή"),
                        word_type: 7,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 1,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 2,
                        word: String::from("Θύρσις"),
                        word_type: 2,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 2,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 3,
                        word: String::from("[line]5"),
                        word_type: 5,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 3,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 4,
                        word: String::from("αἴκα"),
                        word_type: 0,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 4,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 5,
                        word: String::from("δ"),
                        word_type: 0,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 5,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 6,
                        word: String::from("᾽"),
                        word_type: 1,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 6,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 7,
                        word: String::from("αἶγα"),
                        word_type: 0,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 7,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 8,
                        word: String::from("λάβῃ"),
                        word_type: 0,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 8,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 9,
                        word: String::from("τῆνος"),
                        word_type: 0,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 9,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 10,
                        word: String::from("γέρας"),
                        word_type: 0,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 10,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 11,
                        word: String::from(","),
                        word_type: 1,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 11,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 12,
                        word: String::from("ἐς"),
                        word_type: 0,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 12,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 13,
                        word: String::from("τὲ"),
                        word_type: 0,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 13,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 14,
                        word: String::from("καταρρεῖ"),
                        word_type: 0,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 14,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 15,
                        word: String::from(""),
                        word_type: 11,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 15,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 16,
                        word: String::from("[line]10"),
                        word_type: 5,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 16,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 17,
                        word: String::from("ὁσίου"),
                        word_type: 0,
                        lemma: Some(String::from("newword")),
                        def: Some(String::from("newdef")),
                        unit: Some(0),
                        pos: Some(String::from("newpos")),
                        arrowed_id: Some(17),
                        hqid: Some(31),
                        seq: 17,
                        arrowed_seq: Some(17),
                        freq: Some(2),
                        runningcount: Some(1),
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: Some(1),
                        sort_alpha: Some(String::from("newword")),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 18,
                        word: String::from("γὰρ"),
                        word_type: 0,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 18,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 19,
                        word: String::from("ἀνδρὸς"),
                        word_type: 0,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 19,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 20,
                        word: String::from("ὅσιος"),
                        word_type: 0,
                        lemma: Some(String::from("newword")),
                        def: Some(String::from("newdef")),
                        unit: Some(0),
                        pos: Some(String::from("newpos")),
                        arrowed_id: Some(17),
                        hqid: Some(31),
                        seq: 20,
                        arrowed_seq: Some(17),
                        freq: Some(2),
                        runningcount: Some(2),
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: Some(1),
                        sort_alpha: Some(String::from("newword")),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 21,
                        word: String::from("ὢν"),
                        word_type: 0,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 21,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 22,
                        word: String::from("ἐτύγχανον"),
                        word_type: 0,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 22,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 23,
                        word: String::from(""),
                        word_type: 10,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 23,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 24,
                        word: String::from("This"),
                        word_type: 12,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 24,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 25,
                        word: String::from("is"),
                        word_type: 12,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 25,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 26,
                        word: String::from("a"),
                        word_type: 12,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 26,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 27,
                        word: String::from("test"),
                        word_type: 12,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 27,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 28,
                        word: String::from("."),
                        word_type: 1,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 28,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 29,
                        word: String::from(""),
                        word_type: 10,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 29,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 30,
                        word: String::from("γὰρ"),
                        word_type: 0,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 30,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                ]
                .to_vec(),
                selected_id: None,
                error: String::from("")
            }
        );

        //add second text
        let res = setup_small_text_test(&db, course_id, &user_info).await;
        assert!(res.success);

        let info = QueryRequest { text: 2, wordid: 0 };
        let selected_word_id = None;
        let res = gkv_get_text_words(&db, &info, selected_word_id, course_id).await;
        assert_eq!(
            *res.as_ref().unwrap(),
            MiscErrorResponse {
                this_text: 2,
                text_name: String::from("testingtext2"),
                words: [
                    WordRow {
                        wordid: 31,
                        word: String::from("ὁσίου"),
                        word_type: 0,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 1,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 2,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 32,
                        word: String::from("γὰρ"),
                        word_type: 0,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 2,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 2,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 33,
                        word: String::from("ὅσιος"),
                        word_type: 0,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 3,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 2,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    }
                ]
                .to_vec(),
                selected_id: None,
                error: String::from("")
            }
        );
        //println!("res {:?}", res);

        //set_gloss
        let post = SetGlossRequest {
            qtype: String::from("set_gloss"),
            word_id: 31,
            gloss_id,
        };
        let res =
            gkv_update_gloss_id(&db, post.gloss_id, post.word_id, &user_info, course_id).await;
        //println!("arrow: {:?}", res);
        assert_eq!(
            res.unwrap(),
            UpdateGlossIdResponse {
                qtype: String::from("set_gloss"),
                words: [
                    SmallWord {
                        wordid: 17,
                        hqid: gloss_id,
                        lemma: String::from("newword"),
                        pos: String::from("newpos"),
                        def: String::from("newdef"),
                        runningcount: Some(1),
                        arrowed_seq: Some(17),
                        total: Some(3),
                        seq: 17,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: Some(1)
                    },
                    SmallWord {
                        wordid: 20,
                        hqid: gloss_id,
                        lemma: String::from("newword"),
                        pos: String::from("newpos"),
                        def: String::from("newdef"),
                        runningcount: Some(2),
                        arrowed_seq: Some(17),
                        total: Some(3),
                        seq: 20,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: Some(1)
                    },
                    SmallWord {
                        wordid: 31,
                        hqid: gloss_id,
                        lemma: String::from("newword"),
                        pos: String::from("newpos"),
                        def: String::from("newdef"),
                        runningcount: Some(3),
                        arrowed_seq: Some(17),
                        total: Some(3),
                        seq: 1,
                        is_flagged: false,
                        word_text_seq: 2,
                        arrowed_text_seq: Some(1)
                    }
                ]
                .to_vec(),
                success: true,
                affectedrows: 1
            }
        );

        let post = SetGlossRequest {
            qtype: String::from("set_gloss"),
            word_id: 33,
            gloss_id,
        };
        let res =
            gkv_update_gloss_id(&db, post.gloss_id, post.word_id, &user_info, course_id).await;
        //println!("arrow: {:?}", res);
        assert_eq!(
            res.unwrap(),
            UpdateGlossIdResponse {
                qtype: String::from("set_gloss"),
                words: [
                    SmallWord {
                        wordid: 17,
                        hqid: gloss_id,
                        lemma: String::from("newword"),
                        pos: String::from("newpos"),
                        def: String::from("newdef"),
                        runningcount: Some(1),
                        arrowed_seq: Some(17),
                        total: Some(4),
                        seq: 17,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: Some(1)
                    },
                    SmallWord {
                        wordid: 20,
                        hqid: gloss_id,
                        lemma: String::from("newword"),
                        pos: String::from("newpos"),
                        def: String::from("newdef"),
                        runningcount: Some(2),
                        arrowed_seq: Some(17),
                        total: Some(4),
                        seq: 20,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: Some(1)
                    },
                    SmallWord {
                        wordid: 31,
                        hqid: gloss_id,
                        lemma: String::from("newword"),
                        pos: String::from("newpos"),
                        def: String::from("newdef"),
                        runningcount: Some(3),
                        arrowed_seq: Some(17),
                        total: Some(4),
                        seq: 1,
                        is_flagged: false,
                        word_text_seq: 2,
                        arrowed_text_seq: Some(1)
                    },
                    SmallWord {
                        wordid: 33,
                        hqid: gloss_id,
                        lemma: String::from("newword"),
                        pos: String::from("newpos"),
                        def: String::from("newdef"),
                        runningcount: Some(4),
                        arrowed_seq: Some(17),
                        total: Some(4),
                        seq: 3,
                        is_flagged: false,
                        word_text_seq: 2,
                        arrowed_text_seq: Some(1)
                    }
                ]
                .to_vec(),
                success: true,
                affectedrows: 1
            }
        );

        //check
        let res = gkv_get_text_words(&db, &info, selected_word_id, course_id).await;
        assert_eq!(
            *res.as_ref().unwrap(),
            MiscErrorResponse {
                this_text: 2,
                text_name: String::from("testingtext2"),
                words: [
                    WordRow {
                        wordid: 31,
                        word: String::from("ὁσίου"),
                        word_type: 0,
                        lemma: Some(String::from("newword")),
                        def: Some(String::from("newdef")),
                        unit: Some(0),
                        pos: Some(String::from("newpos")),
                        arrowed_id: Some(17),
                        hqid: Some(31),
                        seq: 1,
                        arrowed_seq: Some(17),
                        freq: Some(4),
                        runningcount: Some(3),
                        is_flagged: false,
                        word_text_seq: 2,
                        arrowed_text_seq: Some(1),
                        sort_alpha: Some(String::from("newword")),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 32,
                        word: String::from("γὰρ"),
                        word_type: 0,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 2,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 2,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 33,
                        word: String::from("ὅσιος"),
                        word_type: 0,
                        lemma: Some(String::from("newword")),
                        def: Some(String::from("newdef")),
                        unit: Some(0),
                        pos: Some(String::from("newpos")),
                        arrowed_id: Some(17),
                        hqid: Some(31),
                        seq: 3,
                        arrowed_seq: Some(17),
                        freq: Some(4),
                        runningcount: Some(4),
                        is_flagged: false,
                        word_text_seq: 2,
                        arrowed_text_seq: Some(1),
                        sort_alpha: Some(String::from("newword")),
                        last_word_of_page: false,
                        app_crit: None
                    }
                ]
                .to_vec(),
                selected_id: None,
                error: String::from("")
            }
        );

        let timestamp = 1667191605; //get_timestamp().try_into().unwrap(),
        let info = WordtreeQueryRequest {
            n: 101,
            idprefix: String::from("text"),
            x: String::from("0.2813670904164459"),
            request_time: timestamp,
            page: 0, //can be negative for pages before
            mode: String::from("context"),
            query: r#"{"lexicon":"hqvocab","mode":"normal","w":""}"#.to_string(), //WordQuery,
            lex: Some(String::from("hqvocab")),
        };

        let res = gkv_get_texts(&db, &info, course_id).await;
        assert_eq!(
            *res.as_ref().unwrap(),
            WordtreeQueryResponse {
                select_id: Some(0),
                error: String::from(""),
                wtprefix: String::from("text"),
                nocache: 0,
                container: String::from("textContainer"),
                request_time: 1667191605,
                page: 0,
                last_page: 1,
                lastpage_up: 1,
                scroll: String::from(""),
                query: String::from(""),
                arr_options: [
                    AssignmentTree {
                        i: 1,
                        col: [String::from("testingtext"), String::from("1")].to_vec(),
                        c: [].to_vec(),
                        h: false
                    },
                    AssignmentTree {
                        i: 2,
                        col: [String::from("testingtext2"), String::from("2")].to_vec(),
                        c: [].to_vec(),
                        h: false
                    }
                ]
                .to_vec()
            }
        );
        //println!("res: {:?}", res);
        //change order of texts
        let text_id = 2;
        let step = -1;
        let mut tx = db.begin_tx().await.unwrap();
        let res = tx.update_text_order_db(course_id, text_id, step).await;
        tx.commit_tx().await.unwrap();
        // match res {
        //     Ok(r) => (),
        //     Err(ref r) => println!("error: {:?}", r),
        // };
        assert!(res.is_ok());

        let info = WordtreeQueryRequest {
            n: 101,
            idprefix: String::from("text"),
            x: String::from("0.2813670904164459"),
            request_time: timestamp,
            page: 0, //can be negative for pages before
            mode: String::from("context"),
            query: r#"{"lexicon":"hqvocab","mode":"normal","w":""}"#.to_string(), //WordQuery,
            lex: Some(String::from("hqvocab")),
        };

        let res = gkv_get_texts(&db, &info, course_id).await;
        assert_eq!(
            *res.as_ref().unwrap(),
            WordtreeQueryResponse {
                select_id: Some(0),
                error: String::from(""),
                wtprefix: String::from("text"),
                nocache: 0,
                container: String::from("textContainer"),
                request_time: 1667191605,
                page: 0,
                last_page: 1,
                lastpage_up: 1,
                scroll: String::from(""),
                query: String::from(""),
                arr_options: [
                    AssignmentTree {
                        i: 2,
                        col: [String::from("testingtext2"), String::from("2")].to_vec(),
                        c: [].to_vec(),
                        h: false
                    },
                    AssignmentTree {
                        i: 1,
                        col: [String::from("testingtext"), String::from("1")].to_vec(),
                        c: [].to_vec(),
                        h: false
                    }
                ]
                .to_vec()
            }
        );
        //check

        let info = QueryRequest { text: 2, wordid: 0 };
        let selected_word_id = None;
        let res = gkv_get_text_words(&db, &info, selected_word_id, course_id).await;
        assert_eq!(
            *res.as_ref().unwrap(),
            MiscErrorResponse {
                this_text: 2,
                text_name: String::from("testingtext2"),
                words: [
                    WordRow {
                        wordid: 31,
                        word: String::from("ὁσίου"),
                        word_type: 0,
                        lemma: Some(String::from("newword")),
                        def: Some(String::from("newdef")),
                        unit: Some(0),
                        pos: Some(String::from("newpos")),
                        arrowed_id: Some(17),
                        hqid: Some(31),
                        seq: 1,
                        arrowed_seq: Some(17),
                        freq: Some(4),
                        runningcount: Some(1),
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: Some(2),
                        sort_alpha: Some(String::from("newword")),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 32,
                        word: String::from("γὰρ"),
                        word_type: 0,
                        lemma: None,
                        def: None,
                        unit: None,
                        pos: None,
                        arrowed_id: None,
                        hqid: None,
                        seq: 2,
                        arrowed_seq: None,
                        freq: None,
                        runningcount: None,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: None,
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 33,
                        word: String::from("ὅσιος"),
                        word_type: 0,
                        lemma: Some(String::from("newword")),
                        def: Some(String::from("newdef")),
                        unit: Some(0),
                        pos: Some(String::from("newpos")),
                        arrowed_id: Some(17),
                        hqid: Some(31),
                        seq: 3,
                        arrowed_seq: Some(17),
                        freq: Some(4),
                        runningcount: Some(2),
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: Some(2),
                        sort_alpha: Some(String::from("newword")),
                        last_word_of_page: false,
                        app_crit: None
                    }
                ]
                .to_vec(),
                selected_id: None,
                error: String::from("")
            }
        );
    }

    #[tokio::test]
    #[serial]
    async fn set_gloss() {
        let (db, user_info) = set_up().await;
        let course_id = 1;

        let res = setup_text_test(&db, course_id, &user_info).await;
        assert!(res.success);

        // let info = QueryRequest {
        //     text: 1,
        //     wordid: 0,
        // };
        // let selected_word_id = None;

        // let res = gkv_get_text_words(&db, &info, selected_word_id, course_id).await;
        // println!("words: {:?}", res);

        //set an already existing gloss
        let post = SetGlossRequest {
            qtype: String::from("set_gloss"),
            word_id: 17,
            gloss_id: 30,
        };
        let res =
            gkv_update_gloss_id(&db, post.gloss_id, post.word_id, &user_info, course_id).await;

        //println!("arrow: {:?}", res);
        assert_eq!(
            res.unwrap(),
            UpdateGlossIdResponse {
                qtype: String::from("set_gloss"),
                words: [SmallWord {
                    wordid: 17,
                    hqid: 30,
                    lemma: String::from("newword"),
                    pos: String::from("newpos"),
                    def: String::from("newdef"),
                    runningcount: Some(1),
                    arrowed_seq: None,
                    total: Some(1),
                    seq: 17,
                    is_flagged: false,
                    word_text_seq: 1,
                    arrowed_text_seq: None
                }]
                .to_vec(),
                success: true,
                affectedrows: 1
            }
        );

        // println!("arrow: {:?}", res);

        // let res = gkv_get_text_words(&db, &info, selected_word_id, course_id).await;
        // println!("words: {:?}", res);
    }

    #[tokio::test]
    #[serial]
    async fn insert_and_update_gloss() {
        let course_id = 1;
        let (db, user_info) = set_up().await;

        let post = UpdateGlossRequest {
            qtype: String::from("newlemma"),
            hqid: None,
            lemma: String::from("newword"),
            stripped_lemma: String::from("newword"),
            pos: String::from("newpos"),
            def: String::from("newdef"),
            note: String::from("newnote"),
        };

        let res = gkv_update_or_add_gloss(&db, &post, &user_info).await;
        //println!("words: {:?}", res);

        let gloss_id: u32 = res
            .as_ref()
            .unwrap()
            .inserted_id
            .unwrap()
            .try_into()
            .unwrap();

        assert_eq!(
            *res.as_ref().unwrap(),
            UpdateGlossResponse {
                qtype: String::from("newlemma"),
                success: true,
                affectedrows: 1,
                inserted_id: Some(1)
            }
        );

        let post = UpdateGlossRequest {
            qtype: String::from("editlemma"),
            hqid: Some(gloss_id),
            lemma: String::from("newword2"),
            stripped_lemma: String::from("newword2"),
            pos: String::from("newpos2"),
            def: String::from("newdef2"),
            note: String::from("newnote2"),
        };
        let res = gkv_update_or_add_gloss(&db, &post, &user_info).await;

        //println!("words: {:?}", res);
        assert_eq!(
            *res.as_ref().unwrap(),
            UpdateGlossResponse {
                qtype: String::from("editlemma"),
                success: true,
                affectedrows: 1,
                inserted_id: None
            }
        );

        let timestamp = 1667191605; //get_timestamp().try_into().unwrap(),
        let info = WordtreeQueryRequest {
            n: 101,
            idprefix: String::from("updatelog"),
            x: String::from("0.4828853350220542"),
            request_time: timestamp,
            page: 0, //can be negative for pages before
            mode: String::from("context"),
            query: r#"{"lexicon":"hqvocab","mode":"normal","w":""}"#.to_string(), //WordQuery,
            lex: None,
        };

        let res = gkv_update_log(&db, &info, course_id).await;

        //just check number of update records to avoid having to match up timestamps
        assert_eq!(res.unwrap().arr_options.len(), 2);
    }
}
