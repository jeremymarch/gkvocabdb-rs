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
use thiserror::Error;
use actix_web::{ ResponseError, http::StatusCode};
//use percent_encoding::percent_decode_str;

use std::io;
use actix_files as fs;
use actix_web::{middleware, web, App, error, Error as AWError, HttpResponse, HttpRequest, HttpServer, Result};
use actix_session::{Session, CookieSession};
use actix_multipart::Multipart;

use quick_xml::Reader;
use quick_xml::events::Event;

use mime;

use sqlx::SqlitePool;
use sqlx::sqlite::SqliteConnectOptions;
use std::str::FromStr;

mod db;
use crate::db::*;
use serde::{Deserialize, Serialize};
extern crate chrono;
use chrono::prelude::*;
//use std::time::{SystemTime, UNIX_EPOCH};

//https://stackoverflow.com/questions/64348528/how-can-i-pass-multi-variable-by-actix-web-appdata
//https://doc.rust-lang.org/rust-by-example/generics/new_types.html
#[derive(Clone)]
struct SqliteUpdatePool (SqlitePool);


#[derive(Debug, Serialize, Deserialize, Clone)]
struct QueryResponse {
    #[serde(rename(serialize = "thisText"), rename(deserialize = "thisText"))]
    this_text: u32,
    words: Vec<WordRow>,
    #[serde(rename(serialize = "selectedid"), rename(deserialize = "selectedid"))]
    selected_id: u32,
    error: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct UpdateResponse {
    success: bool,
    #[serde(rename(serialize = "affectedRows"), rename(deserialize = "affectedRows"))]
    affected_rows: u32,
    #[serde(rename(serialize = "arrowedValue"), rename(deserialize = "arrowedValue"))]
    arrowed_value: u32,
    lemmaid: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct UpdateGlossIdResponse {
    qtype: String,
    words: Vec<SmallWord>,
    success: bool,
    affectedrows: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct UpdateLemmaResponse {
    qtype: String,
    success: bool,
    affectedrows: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct WordtreeQueryResponse {
    #[serde(rename(serialize = "selectId"), rename(deserialize = "selectId"))]
    select_id: u32,
    error: String,
    wtprefix: String,
    nocache: u8,
    container: String,
    #[serde(rename(serialize = "requestTime"), rename(deserialize = "requestTime"))]
    request_time: u64,
    page: i32, //can be negative for pages before
    #[serde(rename(serialize = "lastPage"), rename(deserialize = "lastPage"))]
    last_page: u8,
    #[serde(rename(serialize = "lastPageUp"), rename(deserialize = "lastPageUp"))]
    lastpage_up: u8,
    scroll: String,
    query: String,
    #[serde(rename(serialize = "arrOptions"), rename(deserialize = "arrOptions"))]
    arr_options: Vec<(String,u32)>
}

#[derive(Deserialize)]
pub struct QueryRequest {
    pub text: i32,
    pub wordid: i32,
}

#[derive(Deserialize)]
pub struct UpdateRequest {
    pub qtype: String,
    #[serde(rename(serialize = "forLemmaID"), rename(deserialize = "forLemmaID"))]
    pub for_lemma_id: Option<u32>,
    #[serde(rename(serialize = "setArrowedIDTo"), rename(deserialize = "setArrowedIDTo"))]
    pub set_arrowed_id_to: Option<u32>,

    pub textwordid: Option<u32>,
    pub lemmaid: Option<u32>,
    pub lemmastr: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateLemmaRequest {
    pub qtype: String,
    pub hqid:Option<u32>, 
    pub lemma:String, 
    #[serde(rename(serialize = "strippedLemma"), rename(deserialize = "strippedLemma"))]
    pub stripped_lemma:String, 
    pub pos:String, 
    pub def:String, 
    pub note:String,
}

#[derive(Deserialize)]
pub struct GetGlossRequest {
    pub qtype: String,
    pub lemmaid: u32
}

/*
#[derive(Deserialize)]
pub struct WordQuery {
    pub regex: String,
    pub lexicon: String,
    pub tag_id: String,
    pub root_id: String,
    pub wordid: Option<String>,
    pub w: String,
}
*/

#[derive(Deserialize)]
pub struct WordtreeQueryRequest {
    pub n: u32,
    pub idprefix: String,
    pub x: String,
    #[serde(rename(deserialize = "requestTime"))]
    pub request_time: u64,
    pub page: i32, //can be negative for pages before
    pub mode: String,
    pub query: String,//WordQuery,
    pub lex: Option<String>,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GetGlossResponse {
    pub success: bool,
    pub affectedRows: u64,
    pub words: Vec<GlossEntry>,
}

#[allow(clippy::eval_order_dependence)]
async fn update_gloss((session, post, req): (Session, web::Form<UpdateLemmaRequest>, HttpRequest)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();
    //let course_id = 1;
    let user_id = 2;

    let now = Utc::now();
    let timestamp: i64 = now.timestamp();
    //let naive_datetime = NaiveDateTime::from_timestamp(timestamp, 0);
    //let timestamp_string: DateTime<Utc> = DateTime::from_utc(naive_datetime, Utc);
    //println!("Current timestamp is {}", timestamp);
    //println!("{}", datetime_again);

    let user_agent = get_user_agent(&req).unwrap_or("");

    match post.qtype.as_str() {
        "newlemma" => {           
            let updated_ip = "0.0.0.1";
            let rows_affected = new_lemma(db, post.lemma.as_str(), post.pos.as_str(), post.def.as_str(), post.stripped_lemma.as_str(), post.note.as_str(), user_id, timestamp, updated_ip, user_agent).await.map_err(map_sqlx_error)?;

            let res = UpdateLemmaResponse {
                qtype: "newLemma1".to_string(),
                success: true,
                affectedrows: rows_affected,
            };
            return Ok(HttpResponse::Ok().json(res));          
        },
        "editlemma" => {
            if post.hqid.is_some() {
                let updated_ip = "0.0.0.1";
                let rows_affected = update_lemma(db, post.hqid.unwrap(), post.lemma.as_str(), post.pos.as_str(), post.def.as_str(), post.stripped_lemma.as_str(), post.note.as_str(), user_id, timestamp, updated_ip, user_agent).await.map_err(map_sqlx_error)?;
    
                let res = UpdateLemmaResponse {
                    qtype: "newLemma1".to_string(),
                    success: true,
                    affectedrows: rows_affected,
                };
                return Ok(HttpResponse::Ok().json(res));               
            }
        },
        _ => (),
    }
    let res = UpdateLemmaResponse {
        qtype: post.qtype.to_string(),
        success: false,
        affectedrows: 0,
    };

    Ok(HttpResponse::Ok().json(res))
}

#[allow(clippy::eval_order_dependence)]
async fn update_words((session, post, req): (Session, web::Form<UpdateRequest>, HttpRequest)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();
    let course_id = 1;
    let user_id = 2;

    let now = Utc::now();
    let timestamp: i64 = now.timestamp();
    //let naive_datetime = NaiveDateTime::from_timestamp(timestamp, 0);
    //let timestamp_string: DateTime<Utc> = DateTime::from_utc(naive_datetime, Utc);
    //println!("Current timestamp is {}", timestamp);
    //println!("{}", datetime_again);

    let user_agent = get_user_agent(&req).unwrap_or("");

    match post.qtype.as_str() {
        "arrowWord" => {
            
            let _ = arrow_word(db, course_id, post.for_lemma_id.unwrap(), post.set_arrowed_id_to.unwrap(), user_id, timestamp).await.map_err(map_sqlx_error)?;
            let res = UpdateResponse  {
                success: true,
                affected_rows: 1,
                arrowed_value: 1,
                lemmaid:1,
            };
            return Ok(HttpResponse::Ok().json(res));
        }
        "flagUnflagWord" => (),
        "updateLemmaID" => {
            //qtype:"updateLemmaID",textwordid:vTextWordID, lemmaid:vlemmaid, lemmastr:vlemmastr
            
            if post.textwordid.is_some() && post.lemmaid.is_some() {
                let words = set_gloss_id(db, course_id, post.lemmaid.unwrap(), post.textwordid.unwrap(), user_id, timestamp).await.map_err(map_sqlx_error)?;

                println!("TESTING: {}", words.len());

                let res = UpdateGlossIdResponse {
                    qtype: "updateLemmaID".to_string(),
                    words,
                    success: true,
                    affectedrows: 1,
                };
                return Ok(HttpResponse::Ok().json(res));
            }
        },

        "getWordAnnotation" => (),
        "removeDuplicate" => (),
        "updateCounts" => (),
        "getWordsByLemmaId" => (),
        _ => (),
    }

    /*
    history topics:

    arrow word
    change word's gloss
    new gloss
    edit gloss
    flagged word/unflagged

    delete gloss

    changed text seq

    inserted word
    deleted word
    changed word seq
    */
    

    if let Some(count) = session.get::<i32>("counter")? {
        session.insert("counter", count + 1)?;
    } else {
        session.insert("counter", 1)?;
    }


    let res = QueryResponse {
        this_text: 1,
        words: [].to_vec(),
        selected_id: 1,
        error: "".to_string(),
    };

    Ok(HttpResponse::Ok().json(res))
}

#[allow(clippy::eval_order_dependence)]
async fn get_gloss((info, req): (web::Form<GetGlossRequest>, HttpRequest)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();
    let gloss = get_glossdb(db, info.lemmaid).await.map_err(map_sqlx_error)?;

/*
    $a = new \stdClass();
    $a->hqid = $row[0];
    $a->l = $row[1];
    $a->pos = $row[2];
    $a->g = $row[3];
    $a->n = $row[4];
    array_push($words, $a);
*/
    let res = GetGlossResponse {
        success: true,
        affectedRows: 0,
        words: vec![gloss],
    };

    Ok(HttpResponse::Ok().json(res))
}

#[allow(clippy::eval_order_dependence)]
async fn get_wordtree((info, req): (web::Query<WordtreeQueryRequest>, HttpRequest)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();

    let query_params: WordQuery = serde_json::from_str(&info.query)?;
    
    //let seq = get_seq_by_prefix(db, table, &query_params.w).await.map_err(map_sqlx_error)?;

    let mut before_rows = vec![];
    let mut after_rows = vec![];
    if info.page <= 0 {
        
        before_rows = get_before(db, &query_params.w, info.page, info.n).await.map_err(map_sqlx_error)?;
        if info.page == 0 { //only reverse if page 0. if < 0, each row is inserted under top of container one-by-one in order
            before_rows.reverse();
        }
    }
    if info.page >= 0 {
        after_rows = get_equal_and_after(db, &query_params.w, info.page, info.n).await.map_err(map_sqlx_error)?;
    }

    //only check page 0 or page less than 0
    let vlast_page_up = if before_rows.len() < info.n as usize && info.page <= 0 { 1 } else { 0 };
    //only check page 0 or page greater than 0
    let vlast_page = if after_rows.len() < info.n as usize && info.page >= 0 { 1 } else { 0 };

    let seq = if !after_rows.is_empty() { after_rows[0].1 } else { 0 };

    let result_rows = [before_rows, after_rows].concat();

    //strip any numbers from end of string
    //let re = Regex::new(r"[0-9]").unwrap();
    let result_rows_stripped:Vec<(String,u32)> = result_rows.into_iter().map( |mut row| { row.0 = format!("<b>{}</b> {} [count {}] <a href='javascript:editLemmaFormToggle2({})'>edit</a>", row.0,row.2,row.3,row.1); (row.0,row.1) }).collect();

    let res = WordtreeQueryResponse {
        select_id: seq,
        error: "".to_owned(),
        wtprefix: info.idprefix.clone(),
        nocache: if query_params.wordid.is_none() { 0 } else { 1 }, //prevents caching when queried by wordid in url
        container: format!("{}Container", info.idprefix),
        request_time: info.request_time,
        page: info.page,
        last_page: vlast_page,
        lastpage_up: vlast_page_up,
        scroll: if query_params.w.is_empty() && info.page == 0 && seq == 1 { "top".to_string() } else { "".to_string() }, //scroll really only needs to return top
        query: query_params.w.to_owned(),
        arr_options: result_rows_stripped//result_rows
    };

    Ok(HttpResponse::Ok().json(res))
}

#[allow(clippy::eval_order_dependence)]
async fn get_text_words((info, req): (web::Query<QueryRequest>, HttpRequest)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();

    //let query_params: WordQuery = serde_json::from_str(&info.query)?;

    let text_id = match info.wordid {
        0 => info.text,
        _ => {
            get_text_id_for_word_id(db, info.wordid).await.map_err(map_sqlx_error)?
        }
    };

    let w = get_words(db, text_id).await.map_err(map_sqlx_error)?;

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
 
    let res = QueryResponse {
        this_text: 1,
        words: w,
        selected_id: 1,
        error: "".to_string(),
    };

    Ok(HttpResponse::Ok().json(res))
}

#[allow(clippy::eval_order_dependence)]
async fn get_assignments(req: HttpRequest) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();

    let w = get_assignment_rows(db).await.map_err(map_sqlx_error)?;

    Ok(HttpResponse::Ok().json(w))
}

fn get_user_agent(req: &HttpRequest) -> Option<&str> {
    req.headers().get("user-agent")?.to_str().ok()
}

fn get_ip(req: &HttpRequest) -> Option<String> {
    if req.peer_addr().is_some() { 
        Some(req.peer_addr().unwrap().ip().to_string())
    } 
    else 
    { 
        None
    }
}

async fn health_check(_req: HttpRequest) -> Result<HttpResponse, AWError> {
    Ok(HttpResponse::Ok().finish()) //send 200 with empty body
}

async fn import_text(payload: Multipart) -> Result<HttpResponse> {
    let upload_status = files::save_file(payload, "/path/filename.jpg".to_string()).await;

    match upload_status {
        Some(true) => {

            Ok(HttpResponse::Ok()
                .content_type("text/plain")
                .body("update_succeeded"))
        }
        _ => Ok(HttpResponse::BadRequest()
            .content_type("text/plain")
            .body("update_failed")),
    }
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();

    //e.g. export GKVOCABDB_DB_PATH=sqlite://gkvocabdb.sqlite?mode=rwc
    let db_path = std::env::var("GKVOCABDB_DB_PATH")
                   .unwrap_or_else(|_| panic!("Environment variable for sqlite path not set: GKVOCABDB_DB_PATH."));
    
    let options = SqliteConnectOptions::from_str(&db_path)
        .expect("Could not connect to db.")
        .foreign_keys(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .read_only(false)
        .collation("PolytonicGreek", |l, r| l.to_lowercase().cmp( &r.to_lowercase() ) );
    
    let db_pool = SqlitePool::connect_with(options).await.expect("Could not connect to db.");

    /*
    https://github.com/SergioBenitez/Rocket/discussions/1989
    .journal_mode(SqliteJournalMode::Off)
    .read_only(true)
    */
    /*
    use actix_web::error::JsonPayloadError;
    use actix_web::middleware::ErrorHandlers;
    use actix_web::error::InternalError;

    fn post_error(err: JsonPayloadError, req: &HttpRequest) -> Error {
        InternalError::from_response(err, HttpResponse::BadRequest().json(post_error)).into()
      }
      */

    HttpServer::new(move || {
        /*
        // custom `Json` extractor configuration: https://docs.rs/actix-web/4.0.0-beta.20/actix_web/web/struct.JsonConfig.html
        let json_cfg = web::JsonConfig::default()
        // limit request payload size
        .limit(4096)
        // only accept text/plain content type
        .content_type(|mime| mime == mime::TEXT_PLAIN)
        // use custom error handler
        .error_handler(|err, _req| {
            error::InternalError::from_response(err, HttpResponse::Conflict().into()).into()
        });

        let error_handlers = ErrorHandlers::new()
        .handler(
            http::StatusCode::INTERNAL_SERVER_ERROR,
            api::internal_server_error,
        )
        .handler(http::StatusCode::BAD_REQUEST, api::bad_request)
        .handler(http::StatusCode::NOT_FOUND, api::not_found);
        */
        App::new()
            //.wrap(json_cfg)
            .app_data(db_pool.clone())
            
            .wrap(middleware::Logger::default())
            .wrap(CookieSession::signed(&[0; 32]).secure(false))
            .wrap(middleware::Compress::default())
            
            //.wrap(error_handlers)
            .service(
                web::resource("/query")
                    .route(web::get().to(get_text_words)),
            )
            .service(
                web::resource("/queryglosses")
                    .route(web::get().to(get_wordtree)),
            )
            .service(
                web::resource("/assignments")
                    .route(web::get().to(get_assignments)),
            )
            .service(
                web::resource("/updatedb")
                    .route(web::post().to(update_words)),
            )
            .service(
                web::resource("/getgloss")
                    .route(web::post().to(get_gloss)),
            )
            .service(
                web::resource("/updategloss")
                    .route(web::post().to(update_gloss)),
            )
            .service(
                web::resource("/importtext")
                    .route(web::post().to(import_text)),
            )
            .service(
                web::resource("/healthzzz")
                    .route(web::get().to(health_check)),
            )
            .service(fs::Files::new("/", "./static").prefer_utf8(true).index_file("index.html"))
    })
    .bind("0.0.0.0:8088")?
    .run()
    .await
}

#[derive(Error, Debug)]
pub struct PhilologusError {
       code: StatusCode,
       name: String,
       error: String,
}

impl std::fmt::Display for PhilologusError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "PhilologusError: {} {}: {}.", self.code.as_u16(), self.name, self.error)
    }
}

impl ResponseError for PhilologusError {
    fn error_response(&self) -> HttpResponse {
        let error_response = ErrorResponse {
            code: self.code.as_u16(),
            message: self.error.to_string(),
            error: self.name.to_string(),
        };
        HttpResponse::build(self.code).json(error_response)
    }
}

fn map_sqlx_error(e: sqlx::Error) -> PhilologusError {   
    match e {
        sqlx::Error::Configuration(e) => PhilologusError { code: StatusCode::INTERNAL_SERVER_ERROR, name: "sqlx error".to_string(), error: format!("sqlx Configuration: {}", e) },
        sqlx::Error::Database(e) => PhilologusError { code: StatusCode::INTERNAL_SERVER_ERROR, name: "sqlx error".to_string(), error: format!("sqlx Database: {}", e) },
        sqlx::Error::Io(e) => PhilologusError { code: StatusCode::INTERNAL_SERVER_ERROR, name: "sqlx error".to_string(), error: format!("sqlx Io: {}", e) },
        sqlx::Error::Tls(e) => PhilologusError { code: StatusCode::INTERNAL_SERVER_ERROR, name: "sqlx error".to_string(), error: format!("sqlx Tls: {}", e) },
        sqlx::Error::Protocol(e) => PhilologusError { code: StatusCode::INTERNAL_SERVER_ERROR, name: "sqlx error".to_string(), error: format!("sqlx Protocol: {}", e) },
        sqlx::Error::RowNotFound => PhilologusError { code: StatusCode::INTERNAL_SERVER_ERROR, name: "sqlx error".to_string(), error: "sqlx RowNotFound".to_string() },
        sqlx::Error::TypeNotFound { .. } => PhilologusError { code: StatusCode::INTERNAL_SERVER_ERROR, name: "sqlx error".to_string(), error: "sqlx TypeNotFound".to_string() },
        sqlx::Error::ColumnIndexOutOfBounds { .. } => PhilologusError { code: StatusCode::INTERNAL_SERVER_ERROR, name: "sqlx error".to_string(), error: "sqlx ColumnIndexOutOfBounds".to_string() },
        sqlx::Error::ColumnNotFound(e) => PhilologusError { code: StatusCode::INTERNAL_SERVER_ERROR, name: "sqlx error".to_string(), error: format!("sqlx ColumnNotFound: {}", e) },
        sqlx::Error::ColumnDecode { .. } => PhilologusError { code: StatusCode::INTERNAL_SERVER_ERROR, name: "sqlx error".to_string(), error: "sqlx ColumnDecode".to_string() },
        sqlx::Error::Decode(e) => PhilologusError { code: StatusCode::INTERNAL_SERVER_ERROR, name: "sqlx error".to_string(), error: format!("sqlx Decode: {}", e) },
        sqlx::Error::PoolTimedOut => PhilologusError { code: StatusCode::INTERNAL_SERVER_ERROR, name: "sqlx error".to_string(), error: "sqlx PoolTimeOut".to_string() },
        sqlx::Error::PoolClosed => PhilologusError { code: StatusCode::INTERNAL_SERVER_ERROR, name: "sqlx error".to_string(), error: "sqlx PoolClosed".to_string() },
        sqlx::Error::WorkerCrashed => PhilologusError { code: StatusCode::INTERNAL_SERVER_ERROR, name: "sqlx error".to_string(), error: "sqlx WorkerCrashed".to_string() },
        sqlx::Error::Migrate(e) => PhilologusError { code: StatusCode::INTERNAL_SERVER_ERROR, name: "sqlx error".to_string(), error: format!("sqlx Migrate: {}", e) },
        _ => PhilologusError { code: StatusCode::INTERNAL_SERVER_ERROR, name: "sqlx error".to_string(), error: "sqlx Unknown error".to_string() },
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    code: u16,
    error: String,
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App};

    //use serde::{Serialize, Deserialize};
    //use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
    //use actix_web::http::header::ContentType;
/*
    #[actix_rt::test]
    async fn test_index_get() {
        let mut app = test::init_service(App::new().route("/", web::get().to(index))).await;
        let req = test::TestRequest::with_header("content-type", "text/plain").to_request();
        let resp = test::call_service(&mut app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_rt::test]
    async fn test_index_post() {
        let mut app = test::init_service(App::new().route("/", web::get().to(index))).await;
        let req = test::TestRequest::post().uri("/").to_request();
        let resp = test::call_service(&mut app, req).await;
        assert!(resp.status().is_client_error());
    }
*/

    //cargo test -- --nocapture

    #[actix_web::test]
    async fn test_query_paging() {
        let db_path = std::env::var("GKVOCABDB_DB_PATH")
            .unwrap_or_else(|_| panic!("Environment variable for sqlite path not set: GKVOCABDB_DB_PATH."));

        let db_pool = SqlitePool::connect(&db_path).await.expect("Could not connect to db.");

        let mut app = test::init_service(
            App::new()
            .app_data(db_pool.clone())
            .service(
                web::resource("/healthzzz")
                    .route(web::get().to(health_check)),
            )
            .service(
                web::resource("/query")
                    .route(web::get().to(get_text_words)),
        )).await;

        let resp = test::TestRequest::get()
        .uri(r#"/query?text=1&wordid=0"#) //400 Bad Request error if all params not present
        .send_request(&mut app).await;

        assert!(&resp.status().is_success());
        //println!("resp: {:?}", resp);
        let result: QueryResponse = test::read_body_json(resp).await;
        //println!("res: {:?}", result);
        assert_eq!(result.words.len(), 1000);
    }
}

//https://users.rust-lang.org/t/file-upload-in-actix-web/64871/3
pub mod files {
    use std::io::Write;

    use actix_multipart::Multipart;
    use actix_web::{middleware, web, App, Error, HttpResponse, HttpServer};
    use futures::{StreamExt, TryStreamExt};

    use quick_xml::Reader;
    use quick_xml::events::Event;

    use regex::Regex;

    struct Words {
        word: Vec<String>,
        word_type: Vec<u32>,
    }

    fn split_words(text: &str) -> Words {
        //let mut result = Vec::new();
        //let mut result_type = Vec::new();
        let mut words = Words { word:vec![], word_type:vec![]};
        let mut last = 0;
        for (index, matched) in text.match_indices(|c: char| !(c.is_alphanumeric() || c == '\'')) {
            //add words
            if last != index && &text[last..index] != " " {
                words.word.push(text[last..index].to_string());
                words.word_type.push(0);
            }
            //add word separators
            if matched != " " {
                words.word.push(matched.to_string());
                words.word_type.push(1);
            }
            last = index + matched.len();
        }
        //add last word
        if last < text.len() && &text[last..] != " " {
            words.word.push(text[last..].to_string());
            words.word_type.push(0);
        }
        words
    }

    pub async fn save_file(mut payload: Multipart, file_path: String) -> Option<bool> {
        let mut a = "".to_string();
        // iterate over multipart stream
        while let Ok(Some(mut field)) = payload.try_next().await {
            let content_type = field.content_disposition();//.unwrap();
            //let filename = content_type.get_filename().unwrap();
            let filepath = format!(".{}", file_path);

            // File::create is blocking operation, use threadpool
            //let mut f = web::block(|| std::fs::File::create(filepath))
            //    .await
            //    .unwrap();

            // Field in turn is stream of *Bytes* object
            while let Some(chunk) = field.next().await {
                let data = chunk.unwrap();
                a.push_str(std::str::from_utf8(&data).unwrap());
                // filesystem operations are blocking, we have to use threadpool
                /*f = web::block(move || f.write_all(&data).map(|_| f))
                    .await
                    .unwrap();*/
            }
        }
        //println!("string: {}", a);
        let mut reader = Reader::from_str(&a);
        reader.trim_text(true);
        let mut buf = Vec::new();
        //let mut res:Vec<_> = Vec::new();
        //let mut type_a:Vec<_> = Vec::new();
        let mut words2 = Words { word:vec![], word_type:vec![]};
        let mut in_text = false;
        loop {
            match reader.read_event(&mut buf) {
            // for triggering namespaced events, use this instead:
            // match reader.read_namespaced_event(&mut buf) {
                Ok(Event::Start(ref e)) => {
                // for namespaced:
                // Ok((ref namespace_value, Event::Start(ref e)))
                
                    match e.name() {
                        b"text" => in_text = true,
                        _ => (),
                    }
                },
                // unescape and decode the text event using the reader encoding
                Ok(Event::Text(e)) => { 
                    if in_text == true {
                        if let Ok(s) = e.unescape_and_decode(&reader) {
                            
                            //let seperator = Regex::new(r"([ ,.;]+)").expect("Invalid regex");
                            let words = split_words(&s);

                            //let mut splits: Vec<String> = s.split_inclusive(&['\t','\n','\r',' ',',', ';','.']).map(|s| s.to_string()).collect();
                            words2.word.extend_from_slice(&words.word[..]);
                            words2.word_type.extend_from_slice(&words.word_type[..]);
                        }
                    }
                },
                Ok(Event::End(ref e)) => {
                    match e.name() {
                        b"text" => in_text = false,
                        _ => (),
                    }
                },
                Ok(Event::Eof) => break, // exits the loop when reaching end of file
                Err(e) => panic!("Error at position {}: {:?}", reader.buffer_position(), e),
                _ => (), // There are several other `Event`s we do not consider here
            }
        
            // if we don't keep a borrow elsewhere, we can clear the buffer to keep memory usage low
            buf.clear();
        }
        for a in words2.word {
            println!("{}", a);
        }
        
        Some(true)
    }
}
