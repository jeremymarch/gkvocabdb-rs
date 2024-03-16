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
use actix_files as fs;
use actix_multipart::Multipart;
use actix_session::config::PersistentSession;
use actix_session::storage::CookieSessionStore;
use actix_session::Session;
use actix_session::SessionMiddleware;
use actix_web::cookie::time::Duration;
use actix_web::cookie::Key;
use actix_web::http::header::{ContentDisposition, DispositionParam, DispositionType};
use actix_web::http::StatusCode;
use actix_web::ResponseError;
use actix_web::{
    middleware, web, App, Error as AWError, HttpRequest, HttpResponse, HttpServer, Result,
};
use futures::{StreamExt, TryStreamExt};
use gkvocabdb::dbsqlite::GlosserDbSqlite;
use thiserror::Error;

use serde::{Deserialize, Serialize};
use std::io;
//use std::time::{SystemTime, UNIX_EPOCH};
//use mime;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::SqlitePool;
use std::str::FromStr;

use gkvocabdb::ExportRequest;
use gkvocabdb::*;
mod hqvocab;
mod login;

const SECS_IN_YEAR: i64 = 60 * 60 * 24 * 7 * 4 * 12;

//https://stackoverflow.com/questions/64348528/how-can-i-pass-multi-variable-by-actix-web-appdata
//https://doc.rust-lang.org/rust-by-example/generics/new_types.html

#[derive(Debug, Serialize, Deserialize, Clone)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct MoveTextRequest {
    qtype: String,
    text_id: u32,
    step: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct PagebreakRequest {
    word_id: u32,
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

#[derive(Debug, Serialize)]
struct LoginCheckResponse {
    is_logged_in: bool,
    user_id: u32,
}

fn not_logged_in_response() -> Result<HttpResponse, AWError> {
    Ok(HttpResponse::Unauthorized().finish())
}

async fn import_text(
    (session, payload, req): (Session, Multipart, HttpRequest),
) -> Result<HttpResponse> {
    let db = req.app_data::<GlosserDbSqlite>().unwrap();

    let course_id = 1;

    // let user_id = db::insert_user(&db, "testuser", "tu", 0, "12341234", "tu@blah.com").await.unwrap();
    // let user_info = ConnectionInfo {
    //     user_id: user_id.try_into().unwrap(),
    //     timestamp: get_timestamp(),
    //     ip_address: String::from("0.0.0.0"),
    //     user_agent: String::from("test_agent"),
    // };
    // for _n in 1..100 {
    //     let post = UpdateGlossRequest {
    //         qtype: String::from("newlemma"),
    //         hqid: None,
    //         lemma: String::from("newword"),
    //         stripped_lemma: String::from("newword"),
    //         pos: String::from("newpos"),
    //         def: String::from("newdef"),
    //         note: String::from("newnote"),
    //     };
    //     let _ = gkv_update_or_add_gloss(&db, &post, &user_info).await;
    // }

    if let Some(user_id) = login::get_user_id(session) {
        let info = ConnectionInfo {
            user_id,
            timestamp: get_timestamp(),
            ip_address: get_ip(&req).unwrap_or_default(),
            user_agent: get_user_agent(&req).unwrap_or("").to_string(),
        };

        match get_xml_string(payload).await {
            Ok((xml_string, title)) => {
                let res = import_text::gkv_import_text(db, course_id, &info, &title, &xml_string)
                    .await
                    .map_err(map_glosser_error)?;
                Ok(HttpResponse::Ok().json(res))
            }
            Err(e) => {
                let res = ImportResponse {
                    success: false,
                    words_inserted: 0,
                    error: format!(
                        "Error importing text: invalid utf8. Valid up to position: {}.",
                        e.valid_up_to()
                    ),
                };
                Ok(HttpResponse::Ok().json(res))
            }
        }
    } else {
        not_logged_in_response()
    }
}

async fn get_xml_string(mut payload: Multipart) -> Result<(String, String), std::str::Utf8Error> {
    let mut ttbytes = web::BytesMut::new();
    let mut ddbytes = web::BytesMut::new();

    //cf. https://stackoverflow.com/questions/65989077/how-do-i-pass-multipart-form-data-stream-from-client-to-third-party-server-usin

    // iterate over multipart stream
    while let Ok(Some(mut field)) = payload.try_next().await {
        let content_type = field.content_disposition();
        let name = content_type.get_name().unwrap_or("").to_string();

        // Field in turn is stream of *Bytes* object
        while let Some(chunk) = field.next().await {
            let data = chunk.unwrap();

            if name == "title" {
                ttbytes.extend_from_slice(&data);
            } else if name == "file" {
                ddbytes.extend_from_slice(&data);
            }
        }
    }

    let title: String = match std::str::from_utf8(&ttbytes) {
        Ok(xml_data) => xml_data.to_string(),
        Err(e) => {
            return Err(e); //utf8 error
        }
    };

    let xml_string: String = match std::str::from_utf8(&ddbytes) {
        Ok(xml_data) => xml_data.to_string(),
        Err(e) => {
            return Err(e); //utf8 error
        }
    };

    Ok((xml_string, title))
}

async fn insert_pagebreak(
    (info, session, req): (web::Form<PagebreakRequest>, Session, HttpRequest),
) -> Result<HttpResponse> {
    let db = req.app_data::<GlosserDbSqlite>().unwrap();

    if let Some(_user_id) = login::get_user_id(session) {
        let mut tx = db.begin_tx().await.unwrap();
        tx.insert_pagebreak(info.word_id)
            .await
            .map_err(map_glosser_error)?;
        tx.commit_tx().await.unwrap();

        Ok(HttpResponse::Ok().json(1))
    } else {
        not_logged_in_response()
    }
}

async fn delete_pagebreak(
    (info, session, req): (web::Form<PagebreakRequest>, Session, HttpRequest),
) -> Result<HttpResponse> {
    let db = req.app_data::<GlosserDbSqlite>().unwrap();

    if let Some(_user_id) = login::get_user_id(session) {
        let mut tx = db.begin_tx().await.unwrap();
        tx.delete_pagebreak(info.word_id)
            .await
            .map_err(map_glosser_error)?;
        tx.commit_tx().await.unwrap();

        Ok(HttpResponse::Ok().json(1))
    } else {
        not_logged_in_response()
    }
}

async fn export_text(
    (info, session, req): (web::Query<ExportRequest>, Session, HttpRequest),
) -> Result<HttpResponse> {
    let db = req.app_data::<GlosserDbSqlite>().unwrap();
    let bold_glosses = false;
    let course_id = 1;

    //println!("host: {:?}", req.connection_info().host());

    if let Some(_user_id) = login::get_user_id(session) {
        let text_ids_to_export = if !info.text_ids.contains(',') {
            //if single text_id passed in we want to get all of its sibling texts and return them as a comma separated string
            if let Ok(id) = info.text_ids.parse::<u32>() {
                let mut tx = db.begin_tx().await.unwrap();
                let text_ids = tx.get_sibling_texts(id).await.map_err(map_glosser_error)?;
                tx.commit_tx().await.unwrap();
                text_ids
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            } else {
                String::from("") //error parsing single text_id string input
            }
        } else {
            //if multiple texts passed in just use the as is
            info.text_ids.clone()
        };

        if let Ok(latex) = export_text::gkv_export_texts_as_latex(
            db,
            /*lysias*/ //"133,135,136,137",
            /*xenophon*/ //"129,130,131,132"
            /*phaedrus*/ //"228,229,230,231,232,233,234,235,236,237,238,239,240,241,242,243,244,245,246,247,248,249,250,251,252,253,254,255,256,257,258,259,260,261,262,263,264,265,266,267,268,269", //"129,130,131,132", //info.text_ids.as_str(),
            /*thuc*/ //"270,271,272,273,274,275,276,277,278,280,281,282,283,284,285,286,287,288,289,290,291,292,293,294,295",
            /*ajax*/
            //"296,297,298,299,300,301,302,303,304,305,306,307,308,309,310,311,312,313,314",
            &text_ids_to_export,
            course_id,
            bold_glosses,
        )
        .await
        .map_err(map_glosser_error)
        {
            let filename = "glosser_export.tex";
            let cd_header = ContentDisposition {
                disposition: DispositionType::Attachment,
                parameters: vec![DispositionParam::Filename(String::from(filename))],
            };

            Ok(HttpResponse::Ok()
                .content_type("application/x-latex")
                .insert_header(cd_header)
                .body(latex))
        } else {
            let res = ImportResponse {
                success: false,
                words_inserted: 0,
                error: String::from("Export failed"),
            };
            Ok(HttpResponse::Ok().json(res))
        }
    } else {
        not_logged_in_response()
    }
}

async fn update_or_add_gloss(
    (session, post, req): (Session, web::Form<UpdateGlossRequest>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<GlosserDbSqlite>().unwrap();

    if let Some(user_id) = login::get_user_id(session) {
        let info = ConnectionInfo {
            user_id,
            timestamp: get_timestamp(),
            ip_address: get_ip(&req).unwrap_or_default(),
            user_agent: get_user_agent(&req).unwrap_or("").to_string(),
        };

        let res = gkv_update_or_add_gloss(db, &post, &info)
            .await
            .map_err(map_glosser_error)?;

        Ok(HttpResponse::Ok().json(res))
    } else {
        not_logged_in_response()
    }
}

async fn arrow_word_req(
    (session, post, req): (Session, web::Form<ArrowWordRequest>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<GlosserDbSqlite>().unwrap();

    if let Some(user_id) = login::get_user_id(session) {
        let course_id = 1;

        let info = ConnectionInfo {
            user_id,
            timestamp: get_timestamp(),
            ip_address: get_ip(&req).unwrap_or_default(),
            user_agent: get_user_agent(&req).unwrap_or("").to_string(),
        };

        let res = gkv_arrow_word(db, &post, &info, course_id)
            .await
            .map_err(map_glosser_error)?;
        Ok(HttpResponse::Ok().json(res))
    } else {
        not_logged_in_response()
    }
}

async fn set_gloss(
    (session, post, req): (Session, web::Form<SetGlossRequest>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<GlosserDbSqlite>().unwrap();

    if let Some(user_id) = login::get_user_id(session) {
        let course_id = 1;

        let info = ConnectionInfo {
            user_id,
            timestamp: get_timestamp(),
            ip_address: get_ip(&req).unwrap_or_default(),
            user_agent: get_user_agent(&req).unwrap_or("").to_string(),
        };

        let res = gkv_update_gloss_id(db, post.gloss_id, post.word_id, &info, course_id)
            .await
            .map_err(map_glosser_error)?;
        Ok(HttpResponse::Ok().json(res))
    } else {
        not_logged_in_response()
    }
}

async fn move_text(
    (session, post, req): (Session, web::Form<MoveTextRequest>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<GlosserDbSqlite>().unwrap();

    if let Some(user_id) = login::get_user_id(session) {
        let course_id = 1;

        let info = ConnectionInfo {
            user_id,
            timestamp: get_timestamp(),
            ip_address: get_ip(&req).unwrap_or_default(),
            user_agent: get_user_agent(&req).unwrap_or("").to_string(),
        };

        gkv_move_text(db, post.text_id, post.step, &info, course_id)
            .await
            .map_err(map_glosser_error)?;
        let res = MiscErrorResponse {
            this_text: 1,
            text_name: String::from(""),
            words: [].to_vec(),
            selected_id: None,
            error: String::from("Success"),
        };
        Ok(HttpResponse::Ok().json(res))
    } else {
        not_logged_in_response()
    }
}

async fn get_gloss(
    (post, req): (web::Form<GetGlossRequest>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<GlosserDbSqlite>().unwrap();

    let res = gkv_get_gloss(db, &post).await.map_err(map_glosser_error)?;

    Ok(HttpResponse::Ok().json(res))
}

async fn get_glosses(
    (info, req): (web::Query<WordtreeQueryRequest>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<GlosserDbSqlite>().unwrap();

    let res = gkv_get_glosses(db, &info)
        .await
        .map_err(map_glosser_error)?;

    Ok(HttpResponse::Ok().json(res))
}

async fn gloss_occurrences(
    (info, req): (web::Query<WordtreeQueryRequest>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<GlosserDbSqlite>().unwrap();

    let res = gkv_get_occurrences(db, &info)
        .await
        .map_err(map_glosser_error)?;

    Ok(HttpResponse::Ok().json(res))
}

async fn update_log(
    (info, req): (web::Query<WordtreeQueryRequest>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<GlosserDbSqlite>().unwrap();

    let res = gkv_update_log(db, &info).await.map_err(map_glosser_error)?;

    Ok(HttpResponse::Ok().json(res))
}

async fn get_texts(
    (info, req): (web::Query<WordtreeQueryRequest>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<GlosserDbSqlite>().unwrap();

    let res = gkv_get_texts(db, &info).await.map_err(map_glosser_error)?;

    Ok(HttpResponse::Ok().json(res))
}

/*
async fn fix_assignments_web(req: HttpRequest) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();
    fix_assignments(db).await.map_err(map_glosser_error)?;

    Ok(HttpResponse::Ok().finish())
}
*/

async fn get_text_words(
    (session, info, req): (Session, web::Query<QueryRequest>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<GlosserDbSqlite>().unwrap();

    let selected_word_id: Option<u32> = Some(info.wordid);

    if login::get_user_id(session).is_some() {
        let res = gkv_get_text_words(db, &info, selected_word_id)
            .await
            .map_err(map_glosser_error)?;

        Ok(HttpResponse::Ok().json(res))
    } else {
        let res = MiscErrorResponse {
            this_text: 1,
            text_name: String::from(""),
            words: vec![],
            selected_id: selected_word_id,
            error: String::from("Not logged in"),
        };

        Ok(HttpResponse::Ok().json(res))
    }
}

/*
async fn get_assignments(req: HttpRequest) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();
    let course_id = 1;
    let w = get_assignment_rows(db, course_id).await.map_err(map_glosser_error)?;

    Ok(HttpResponse::Ok().json(w))
}
*/

#[derive(Error, Debug)]
pub struct PhilologusError {
    code: StatusCode,
    name: String,
    error: String,
}

impl std::fmt::Display for PhilologusError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            fmt,
            "PhilologusError: {} {}: {}.",
            self.code.as_u16(),
            self.name,
            self.error
        )
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

pub fn map_glosser_error(e: GlosserError) -> PhilologusError {
    match e {
        GlosserError::Database(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: String::from("sqlx error"),
            error: format!("sqlx Configuration: {}", e),
        },
        GlosserError::XmlError(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: String::from("xml error"),
            error: format!("xml error: {}", e),
        },
        GlosserError::JsonError(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: String::from("json error"),
            error: format!("json error: {}", e),
        },
        GlosserError::ImportError(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: String::from("import error"),
            error: format!("import error: {}", e),
        },
        GlosserError::AuthenticationError => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: String::from("authentication error"),
            error: String::from("authentication error"),
        },
        GlosserError::UnknownError => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: String::from("unknown error"),
            error: String::from("unknown error"),
        },
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    code: u16,
    error: String,
    message: String,
}

fn get_user_agent(req: &HttpRequest) -> Option<&str> {
    req.headers().get("user-agent")?.to_str().ok()
}

fn get_ip(req: &HttpRequest) -> Option<String> {
    req.peer_addr().map(|addr| addr.ip().to_string())
}

async fn health_check(_req: HttpRequest) -> Result<HttpResponse, AWError> {
    //remember that basic authentication blocks this
    Ok(HttpResponse::Ok().finish()) //send 200 with empty body
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();

    //e.g. export GKVOCABDB_DB_PATH=sqlite://gkvocabnew.sqlite?mode=rwc
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

    let db_pool = GlosserDbSqlite {
        db: SqlitePool::connect_with(options)
            .await
            .expect("Could not connect to db."),
    };

    gkv_create_db(&db_pool).await.expect("Could not create db.");
    let mut tx = db_pool.begin_tx().await.unwrap();
    tx.load_lemmatizer().await.unwrap();
    tx.commit_tx().await.unwrap();

    //insert_thuc_paras(&db_pool).await;

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
        let string_key_64_bytes = std::env::var("GKVOCABDB_KEY")
            .unwrap_or_else(|_| panic!("GKVOCABDB_KEY env variable not set."));
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
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), secret_key)
                    .cookie_secure(cookie_secure) //cookie_secure must be false if testing without https
                    .cookie_same_site(actix_web::cookie::SameSite::Strict)
                    .cookie_content_security(actix_session::config::CookieContentSecurity::Private)
                    .session_lifecycle(
                        PersistentSession::default().session_ttl(Duration::seconds(SECS_IN_YEAR)),
                    )
                    .cookie_name(String::from("gkvocabdbid"))
                    .build(),
            )
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
        .route("/logout", web::get().to(login::logout))
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
        .service(web::resource("/hqvocab").route(web::get().to(hqvocab::hqvocab)))
        .service(web::resource("/arrowword").route(web::post().to(arrow_word_req)))
        .service(web::resource("/setgloss").route(web::post().to(set_gloss)))
        .service(web::resource("/updategloss").route(web::post().to(update_or_add_gloss)))
        .service(web::resource("/importtext").route(web::post().to(import_text)))
        .service(web::resource("/exporttext").route(web::get().to(export_text)))
        .service(web::resource("/movetext").route(web::post().to(move_text)))
        .service(web::resource("/insertpagebreak").route(web::post().to(insert_pagebreak)))
        .service(web::resource("/deletepagebreak").route(web::post().to(delete_pagebreak)))
        .service(web::resource("/healthzzz").route(web::get().to(health_check)))
        .service(
            fs::Files::new("/", "./static")
                .prefer_utf8(true)
                .index_file("index.html"),
        );
}

#[cfg(test)]
mod tests {
    //use super::*;
    //use actix_web::{test, web, App};

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
        assert_eq!(result.error, String::from("Not logged in"));
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
