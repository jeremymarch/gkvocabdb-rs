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
use percent_encoding::percent_decode_str;

use std::io;
use regex::Regex;
use actix_files as fs;
use actix_web::{middleware, web, App, Error as AWError, HttpResponse, HttpRequest, HttpServer, Result};
use actix_session::{Session, CookieSession};

use sqlx::SqlitePool;

use actix_files::NamedFile;
use std::path::PathBuf;

mod db;
use crate::db::*;
use serde::{Deserialize, Serialize};

use std::time::{SystemTime, UNIX_EPOCH};

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
    forLemmaID: u32,
    setArrowedIDTo: u32,
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
    pub tag_id: Option<String>,
    pub root_id: Option<String>,
    pub wordid: Option<String>,
    pub w: String,
}
#[allow(clippy::eval_order_dependence)]
async fn update_words((session, post, req): (Session, web::Form<UpdateRequest>, HttpRequest)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();
    
    match post.qtype.as_str() {
        "arrowWord" => {
            let seq_id = 1;
            let _ = arrow_word(db, seq_id, post.forLemmaID, post.setArrowedIDTo).await.map_err(map_sqlx_error)?;
            let res = UpdateResponse  {
                success: true,
                affected_rows: 1,
                arrowed_value: 1,
                lemmaid:1,
            };
            return Ok(HttpResponse::Ok().json(res));
        }
        "flagUnflagWord" => (),
        "updateLemmaID" => (),
        "newlemma" => (),
        "editlemma" => (),
        "getWordAnnotation" => (),
        "getLemma" => (),
        "removeDuplicate" => (),
        "updateCounts" => (),
        "getWordsByLemmaId" => (),
        _ => (),
    }
    

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
async fn get_wordtree((info, req): (web::Query<WordtreeQueryRequest>, HttpRequest)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();

    let query_params: WordQuery = serde_json::from_str(&info.query)?;
    
    let table = "hqvocab";
    
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

    let seq = if after_rows.len() > 0 { after_rows[0].1 } else { 0 };

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

async fn health_check(_req: HttpRequest) -> Result<HttpResponse, AWError> {
    Ok(HttpResponse::Ok().finish()) //send 200 with empty body
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();

    //e.g. export GKVOCABDB_DB_PATH=sqlite://gkvocabdb.sqlite?mode=rwc
    let db_path = std::env::var("GKVOCABDB_DB_PATH")
                   .unwrap_or_else(|_| panic!("Environment variable for sqlite path not set: GKVOCABDB_DB_PATH."));
    let db_pool = SqlitePool::connect(&db_path).await.expect("Could not connect to db.");

    /*
    https://github.com/SergioBenitez/Rocket/discussions/1989
    .journal_mode(SqliteJournalMode::Off)
    .read_only(true)
    */
/*
    let error_handlers = ErrorHandlers::new()
            .handler(
                http::StatusCode::INTERNAL_SERVER_ERROR,
                api::internal_server_error,
            )
            .handler(http::StatusCode::BAD_REQUEST, api::bad_request)
            .handler(http::StatusCode::NOT_FOUND, api::not_found);
*/
    HttpServer::new(move || {
        App::new()
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
                web::resource("/queryWordTree")
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

fn map_utf8_error(_e: std::str::Utf8Error) -> PhilologusError {   
    PhilologusError { code: StatusCode::INTERNAL_SERVER_ERROR, name: "percent_decode_str utf-8 error".to_string(), error: "percent_decode_str utf-8 error".to_string() }
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
