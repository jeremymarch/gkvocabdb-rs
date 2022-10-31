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
use actix_web::{http::StatusCode, ResponseError};
use thiserror::Error;

use actix_files as fs;
use actix_session::Session;
use actix_session::SessionMiddleware;
use actix_session::storage::CookieSessionStore;
use actix_web::http::header::ContentType;
use actix_web::http::header::LOCATION;
use actix_web::cookie::Key;
use actix_web::{
    middleware, web, App, Error as AWError, HttpRequest, HttpResponse, HttpServer, Result,
};
use actix_web::cookie::time::Duration;
use actix_session::config::PersistentSession;
const SECS_IN_YEAR: i64 = 60 * 60 * 24 * 7 * 4 * 12;

use std::io;

//use mime;

use sqlx::sqlite::SqliteConnectOptions;
use sqlx::SqlitePool;
use std::str::FromStr;

use crate::gkvocab::*;

mod hqvocab;
mod gkvocab;
mod db;
mod export_text;
mod import_text_xml;
mod login;
use crate::db::*;
use serde::{Deserialize, Serialize};
extern crate chrono;
use chrono::prelude::*;
//use std::time::{SystemTime, UNIX_EPOCH};

//https://stackoverflow.com/questions/64348528/how-can-i-pass-multi-variable-by-actix-web-appdata
//https://doc.rust-lang.org/rust-by-example/generics/new_types.html

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssignmentTree {
    pub i: u32,
    pub col: Vec<String>,
    pub c: Vec<AssignmentTree>,
    pub h: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConnectionInfo {
    pub user_id: u32,
    pub timestamp: i64,
    pub ip_address: String,
    pub user_agent: String,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
struct LoginResponse {
    success: bool,
}

//type TreeRow = (String, u32, Option<Vec<TreeRow>>)
#[derive(Debug, Serialize, Deserialize, Clone)]
struct TreeRow {
    v: String,
    i: u32,
    c: Option<Vec<TreeRow>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MiscErrorResponse {
    #[serde(rename(serialize = "thisText"), rename(deserialize = "thisText"))]
    pub this_text: u32,
    pub text_name: String,
    pub words: Vec<WordRow>,
    #[serde(rename(serialize = "selectedid"), rename(deserialize = "selectedid"))]
    pub selected_id: Option<u32>,
    pub error: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct WordtreeQueryResponseTree {
    #[serde(rename(serialize = "selectId"), rename(deserialize = "selectId"))]
    select_id: Option<u32>,
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
    arr_options: Vec<TreeRow>,
}

#[derive(Deserialize, Serialize)]
pub struct ImportRequest {
    pub title: String,
}

#[derive(Deserialize, Serialize)]
pub struct QueryRequest {
    pub text: u32,
    pub wordid: u32,
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

#[derive(Deserialize)]
pub struct SetGlossRequest {
    pub qtype: String,
    pub word_id: u32,
    pub gloss_id: u32,
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

#[derive(Deserialize)]
pub struct GetGlossRequest {
    pub qtype: String,
    pub lemmaid: u32,
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
pub struct ExportRequest {
    pub textid: u32,
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
    #[serde(
        rename(deserialize = "affectedRows"),
        rename(serialize = "affectedRows")
    )]
    pub affected_rows: u64,
    pub words: Vec<GlossEntry>,
}

#[derive(Debug, Serialize)]
struct LoginCheckResponse {
    is_logged_in: bool,
    user_id: u32,
}

#[derive(Debug, Serialize)]
pub struct ImportResponse {
    pub success: bool,
    pub words_inserted: u64,
    pub error: String,
}

async fn update_or_add_gloss(
    (session, post, req): (Session, web::Form<UpdateGlossRequest>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();

    if let Some(user_id) = login::get_user_id(session) {
        let info = ConnectionInfo {
            user_id,
            timestamp: get_timestamp(),
            ip_address: get_ip(&req).unwrap_or_else(|| "".to_string()),
            user_agent: get_user_agent(&req).unwrap_or("").to_string(),
        };

        let res = gkv_update_or_add_gloss(db, &post, &info).await?;

        Ok(HttpResponse::Ok().json(res))
    }
    else {
        //not logged in
        let res = UpdateGlossResponse {
            qtype: post.qtype.to_string(),
            success: false,
            affectedrows: 0,
            inserted_id: None,
        };
        Ok(HttpResponse::Ok().json(res))
    }
}

async fn arrow_word_req(
    (session, post, req): (Session, web::Form<ArrowWordRequest>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();

    if let Some(user_id) = login::get_user_id(session) {
        let course_id = 1;

        let info = ConnectionInfo {
            user_id,
            timestamp: get_timestamp(),
            ip_address: get_ip(&req).unwrap_or_else(|| "".to_string()),
            user_agent: get_user_agent(&req).unwrap_or("").to_string(),
        };

        let res = gkv_arrow_word(db, &post, &info, course_id).await?;
        return Ok(HttpResponse::Ok().json(res));
    }

    let res = MiscErrorResponse {
        this_text: 1,
        text_name:"".to_string(),
        words: [].to_vec(),
        selected_id: None,
        error: "Not logged in (update_words)".to_string(),
    };
    Ok(HttpResponse::Ok().json(res))
}

async fn set_gloss(
    (session, post, req): (Session, web::Form<SetGlossRequest>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();

    if let Some(user_id) = login::get_user_id(session) {
        let course_id = 1;

        let info = ConnectionInfo {
            user_id,
            timestamp: get_timestamp(),
            ip_address: get_ip(&req).unwrap_or_else(|| "".to_string()),
            user_agent: get_user_agent(&req).unwrap_or("").to_string(),
        };
       
        let res = gkv_update_gloss_id(db, post.gloss_id, post.word_id, &info, course_id).await?;
        return Ok(HttpResponse::Ok().json(res));
    } 
    let res = MiscErrorResponse {
        this_text: 1,
        text_name:"".to_string(),
        words: [].to_vec(),
        selected_id: None,
        error: "Not logged in (update_words)".to_string(),
    };

    Ok(HttpResponse::Ok().json(res))
}

async fn get_gloss(
    (post, req): (web::Form<GetGlossRequest>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();

    let res = gkv_tet_gloss(db, &post).await?;

    Ok(HttpResponse::Ok().json(res))
}

async fn get_glosses(
    (info, req): (web::Query<WordtreeQueryRequest>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();

    let res = gkv_get_glosses(db, &info).await?;

    Ok(HttpResponse::Ok().json(res))
}

async fn gloss_occurrences(
    (info, req): (web::Query<WordtreeQueryRequest>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();

    let res = gkv_get_occurrences(db, &info).await?;

    Ok(HttpResponse::Ok().json(res))
}

async fn update_log(
    (info, req): (web::Query<WordtreeQueryRequest>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();

    let res = gkv_update_log(db, &info).await?;

    Ok(HttpResponse::Ok().json(res))
}

async fn get_texts(
    (info, req): (web::Query<WordtreeQueryRequest>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();

    let res = gkv_get_texts(db, &info).await?;

    Ok(HttpResponse::Ok().json(res))
}

/*
async fn fix_assignments_web(req: HttpRequest) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();
    fix_assignments(db).await.map_err(map_sqlx_error)?;

    Ok(HttpResponse::Ok().finish())
}
*/

async fn get_text_words(
    (session, info, req): (Session, web::Query<QueryRequest>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();

    let selected_word_id: Option<u32> = Some(info.wordid);

    if login::get_user_id(session).is_some() {

        let res = gkv_get_text_words(db, &info, selected_word_id).await?;

        Ok(HttpResponse::Ok().json(res))
    } else {
        let res = MiscErrorResponse {
            this_text: 1,
            text_name: "".to_string(),
            words: vec![],
            selected_id: selected_word_id,
            error: "Not logged in".to_string(),
        };

        Ok(HttpResponse::Ok().json(res))
    }
}

/*
async fn get_assignments(req: HttpRequest) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();
    let course_id = 1;
    let w = get_assignment_rows(db, course_id).await.map_err(map_sqlx_error)?;

    Ok(HttpResponse::Ok().json(w))
}
*/
fn get_user_agent(req: &HttpRequest) -> Option<&str> {
    req.headers().get("user-agent")?.to_str().ok()
}

fn get_ip(req: &HttpRequest) -> Option<String> {
    req.peer_addr().map(|addr| addr.ip().to_string())
}

fn get_timestamp() -> i64 {
    let now = Utc::now();
    now.timestamp()
}

async fn health_check(_req: HttpRequest) -> Result<HttpResponse, AWError> {
    //remember that basic authentication blocks this
    Ok(HttpResponse::Ok().finish()) //send 200 with empty body
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();

    //e.g. export GKVOCABDB_DB_PATH=sqlite://db.sqlite?mode=rwc
    let db_path = std::env::var("GKVOCABDB_DB_PATH").unwrap_or_else(|_| {
        panic!("Environment variable for sqlite path not set: GKVOCABDB_DB_PATH.")
    });

    let options = SqliteConnectOptions::from_str(&db_path)
        .expect("Could not connect to db.")
        .foreign_keys(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .read_only(false)
        .collation("PolytonicGreek", |l, r| {
            l.to_lowercase().cmp(&r.to_lowercase())
        });

    let db_pool = SqlitePool::connect_with(options)
        .await
        .expect("Could not connect to db.");

    gkv_create_db(&db_pool).await.expect("Could not create db.");

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

        //let auth_basic = HttpAuthentication::basic(validator_basic);
        
    //1. to make a new key:
    // let secret_key = Key::generate(); // only for testing: should use same key from .env file/variable, else have to login again on each restart
    // println!("key: {}{}", hex::encode( secret_key.signing() ), hex::encode( secret_key.encryption() ));

    //2. a simple example testing key
    //https://docs.rs/cookie/0.16.0/src/cookie/secure/key.rs.html#35
    // let key: &Vec<u8> = &(0..64).collect();
    // let secret_key = Key::from(key);

    //3. to load from string
    // let string_key_64_bytes = "c67ba35ad969a3f4255085c359f120bae733c5a5756187aaffab31c7c84628b6a9a02ce6a1e923a945609a884f913f83ea50675b184514b5d15c3e1a606a3fd2";
    // let key = hex::decode(string_key_64_bytes).expect("Decoding key failed");
    // let secret_key = Key::from(&key);

    //4. or load from env
    //e.g. export GKVOCABDB_KEY=56d520157194bdab7aec18755508bf6d063be7a203ddb61ebaa203eb1335c2ab3c13ecba7fc548f4563ac1d6af0b94e6720377228230f210ac51707389bf3285
    let string_key_64_bytes = std::env::var("GKVOCABDB_KEY").unwrap_or_else(|_| { panic!("Key env not set.") });
    let key = hex::decode(string_key_64_bytes).expect("Decoding key failed");
    let secret_key = Key::from(&key);

    let cookie_secure = !cfg!(debug_assertions); //cookie is secure for release, not secure for debug builds

    App::new()
        //.app_data(web::JsonConfig::default().error_handler(|err, _req| actix_web::error::InternalError::from_response(
        //    err, HttpResponse::Conflict().finish()).into()))
        //.wrap(json_cfg)
        .app_data(db_pool.clone())
        .wrap(middleware::Compress::default())
        
        //.wrap(auth_basic) //this blocks healthcheck
        .wrap(SessionMiddleware::builder(
            CookieSessionStore::default(), secret_key)
                .cookie_secure(cookie_secure) //cookie_secure must be false if testing without https
                .cookie_same_site(actix_web::cookie::SameSite::Strict)
                .cookie_content_security(actix_session::config::CookieContentSecurity::Private)
                .session_lifecycle(
                    PersistentSession::default().session_ttl(Duration::seconds(SECS_IN_YEAR))
                )
                .cookie_name(String::from("gkvocabdbid"))
                .build())
        .wrap(middleware::Logger::default())
        //.wrap(error_handlers)
        .configure(config)
    })
    .bind("0.0.0.0:8088")?
    .run()
    .await
}

fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("/login", web::get().to(login::login_get))
        .route("/login", web::post().to(login::login_post))
        .service(web::resource("/query").route(web::get().to(get_text_words)))
        .service(web::resource("/queryglosses").route(web::get().to(get_glosses)))
        .service(web::resource("/querytexts").route(web::get().to(get_texts)))
        .service(web::resource("/glossuses").route(web::get().to(gloss_occurrences)))
        .service(web::resource("/updatelog").route(web::get().to(update_log)))
        /* .service(
            web::resource("/assignments")
                .route(web::get().to(get_assignments)),
        )*/
        .service(web::resource("/getgloss").route(web::post().to(get_gloss)))
        .service(
            web::resource("/hqvocab")
                .route(web::get().to(hqvocab::hqvocab)),
        )
        .service(
            web::resource("/arrowword") //checks session
                .route(web::post().to(arrow_word_req)),
        )
        .service(
            web::resource("/setgloss") //checks session
                .route(web::post().to(set_gloss)),
        )
        .service(
            web::resource("/updategloss") //checks session
                .route(web::post().to(update_or_add_gloss)),
        )
        .service(
            web::resource("/importtext") //checks session
                .route(web::post().to(import_text_xml::import_text)),
        )
        .service(
            web::resource("/exporttext") //checks session
                .route(web::get().to(export_text::export_text)),
        )
        .service(web::resource("/healthzzz").route(web::get().to(health_check)))
        .service(
            fs::Files::new("/", "./static")
                .prefer_utf8(true)
                .index_file("index.html"),
        );
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
    */
    /*
    #[actix_rt::test]
    async fn test_index_post() {
        let db_path = std::env::var("GKVOCABDB_DB_PATH").unwrap_or_else(|_| {
            panic!("Environment variable for sqlite path not set: GKVOCABDB_DB_PATH.")
        });

        let options = SqliteConnectOptions::from_str(&db_path)
            .expect("Could not connect to db.")
            .foreign_keys(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .read_only(false)
            .collation("PolytonicGreek", |l, r| {
                l.to_lowercase().cmp(&r.to_lowercase())
            });

        let db_pool = SqlitePool::connect_with(options)
            .await
            .expect("Could not connect to db.");

        let string_key_64_bytes = "c67ba35ad969a3f4255085c359f120bae733c5a5756187aaffab31c7c84628b6a9a02ce6a1e923a945609a884f913f83ea50675b184514b5d15c3e1a606a3fd2";
        let key = hex::decode(string_key_64_bytes).expect("Decoding key failed");
        let secret_key = Key::from(&key);

        let cookie_secure = !cfg!(debug_assertions); //cookie is secure for release, not secure for debug builds

        let mut app = test::init_service(
            App::new()
                //.app_data(web::JsonConfig::default().error_handler(|err, _req| actix_web::error::InternalError::from_response(
                //    err, HttpResponse::Conflict().finish()).into()))
                //.wrap(json_cfg)
                .app_data(db_pool.clone())
                .wrap(middleware::Compress::default())
                
                //.wrap(auth_basic) //this blocks healthcheck
                .wrap(SessionMiddleware::builder(
                    CookieSessionStore::default(), secret_key.clone())
                        .cookie_secure(cookie_secure) //cookie_secure must be false if testing without https
                        .cookie_same_site(actix_web::cookie::SameSite::Strict)
                        .cookie_content_security(actix_session::config::CookieContentSecurity::Private)
                        .session_lifecycle(
                            PersistentSession::default().session_ttl(Duration::seconds(SECS_IN_YEAR))
                        )
                        .cookie_name(String::from("gkvocabdbid"))
                        .build())
                .wrap(middleware::Logger::default())
                        //.wrap(error_handlers)
                        .configure(config),
        )
        .await;

        let req = test::TestRequest::get().uri("/index.html").to_request();
        let resp = test::call_service(&mut app, req).await;
        assert!(resp.status().is_success());

        let resp = test::TestRequest::get()
            .uri(r#"/query?text=100&wordid=0"#)
            .send_request(&mut app)
            .await;

        assert!(&resp.status().is_success());
        //println!("resp: {:?}", resp);
        let result: MiscErrorResponse = test::read_body_json(resp).await;
        //println!("res: {:?}", result);
        assert_eq!(result.error, "Not logged in".to_string());
    }

    //cargo test -- --nocapture

    // async fn create_db(db: &SqlitePool) -> Result<(), sqlx::Error> {
    //     let query =
    //         "CREATE TABLE users (user_id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL);";
    //     sqlx::query(query).execute(db).await?;

    //     Ok(())
    // }

    #[actix_web::test]
    async fn test_query_paging() {
        let db_path = std::env::var("GKVOCABDB_DB_PATH").unwrap_or_else(|_| {
            panic!("Environment variable for sqlite path not set: GKVOCABDB_DB_PATH.")
        });

        let db_pool = SqlitePool::connect(&db_path)
            .await
            .expect("Could not connect to db.");
        
        // let db_pool = SqlitePool::connect("sqlite::memory:").await.expect("Could not connect to db.");
        // create_db(&db_pool);
        

        let mut app = test::init_service(
            App::new()
                .app_data(db_pool.clone())
                .service(web::resource("/healthzzz").route(web::get().to(health_check)))
                .service(web::resource("/query").route(web::get().to(get_text_words))),
        )
        .await;

        let resp = test::TestRequest::get()
            .uri(r#"/healthzzz"#) //400 Bad Request error if all params not present
            .send_request(&mut app)
            .await;
        assert!(&resp.status().is_success()); //health check

        let resp = test::TestRequest::get()
            .uri(r#"/query?text=100&wordid=0"#) //400 Bad Request error if all params not present
            .send_request(&mut app)
            .await;

        assert!(&resp.status().is_success());
        //println!("resp: {:?}", resp);
        let _result: MiscErrorResponse = test::read_body_json(resp).await;
        //println!("res: {:?}", result);
        //assert_eq!(result.words.len(), 176);
        
    }
    */
}
