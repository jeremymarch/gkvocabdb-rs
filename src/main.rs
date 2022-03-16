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
use actix_web::{middleware, web, App, Error as AWError, HttpResponse, HttpRequest, HttpServer, Result};
use actix_session::{Session, CookieSession};
use actix_multipart::Multipart;
use actix_web::http::header::ContentType;
use actix_web::http::header::LOCATION;

//use mime;

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
struct LoginRequest {
    username:String,
    password:String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct LoginResponse {
    success:bool,
}

//type TreeRow = (String, u32, Option<Vec<TreeRow>>)
#[derive(Debug, Serialize, Deserialize, Clone)]
struct TreeRow {
    v:String,
    i:u32,
    c:Option<Vec<TreeRow>>
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QueryResponse {
    #[serde(rename(serialize = "thisText"), rename(deserialize = "thisText"))]
    pub this_text: u32,
    pub words: Vec<WordRow>,
    #[serde(rename(serialize = "selectedid"), rename(deserialize = "selectedid"))]
    pub selected_id: u32,
    pub error: String,
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
    arr_options: Vec<AssignmentTree> //Vec<(String,u32)>
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct WordtreeQueryResponseTree {
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
    arr_options: Vec<TreeRow>
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
    #[serde(rename(deserialize = "affectedRows"), rename(serialize = "affectedRows"))]
    pub affected_rows: u64,
    pub words: Vec<GlossEntry>,
}

#[allow(dead_code)]
enum WordType {
    Word = 0,
    Punctuation = 1,
    Speaker = 2,
    Section = 4,
    VerseLine = 5, //for verse #
    ParaWithIndent = 6,
    WorkTitle = 7,
    SectionTitle = 8,
    InlineSpeaker = 9,
    ParaNoIndent = 10
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

#[allow(clippy::eval_order_dependence)]
async fn update_or_add_gloss((session, post, req): (Session, web::Form<UpdateLemmaRequest>, HttpRequest)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();

    if let Some(user_id) = get_user_id(session) {

        let timestamp = get_timestamp();
        let updated_ip = get_ip(&req).unwrap_or_else(|| "".to_string());
        let user_agent = get_user_agent(&req).unwrap_or("");

        match post.qtype.as_str() {
            "newlemma" => {           
                
                let rows_affected = insert_gloss(db, post.lemma.as_str(), post.pos.as_str(), post.def.as_str(), post.stripped_lemma.as_str(), post.note.as_str(), user_id, timestamp, &updated_ip, user_agent).await.map_err(map_sqlx_error)?;

                let res = UpdateLemmaResponse {
                    qtype: post.qtype.to_string(),
                    success: true,
                    affectedrows: rows_affected,
                };
                return Ok(HttpResponse::Ok().json(res));          
            },
            "editlemma" => {
                if post.hqid.is_some() {
                    let rows_affected = update_gloss(db, post.hqid.unwrap(), post.lemma.as_str(), post.pos.as_str(), post.def.as_str(), post.stripped_lemma.as_str(), post.note.as_str(), user_id, timestamp, &updated_ip, user_agent).await.map_err(map_sqlx_error)?;
        
                    let res = UpdateLemmaResponse {
                        qtype: post.qtype.to_string(),
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
    else {
        //not logged in
        let res = UpdateLemmaResponse {
            qtype: post.qtype.to_string(),
            success: false,
            affectedrows: 0,
        };

        Ok(HttpResponse::Ok().json(res))
    }
}

#[allow(clippy::eval_order_dependence)]
async fn update_words((session, post, req): (Session, web::Form<UpdateRequest>, HttpRequest)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();

    if let Some(user_id) = get_user_id(session) {
        let course_id = 1;

        let timestamp = get_timestamp();
        let updated_ip = get_ip(&req).unwrap_or_else(|| "".to_string());
        let user_agent = get_user_agent(&req).unwrap_or("");

        match post.qtype.as_str() {
            "arrowWord" => {
                
                let _ = arrow_word(db, course_id, post.for_lemma_id.unwrap(), post.set_arrowed_id_to.unwrap(), user_id, timestamp, &updated_ip, user_agent).await.map_err(map_sqlx_error)?;
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
                    let words = set_gloss_id(db, course_id, post.lemmaid.unwrap(), post.textwordid.unwrap(), user_id, timestamp, &updated_ip, user_agent).await.map_err(map_sqlx_error)?;

                    //println!("TESTING: {}", words.len());

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

        let res = QueryResponse {
            this_text: 1,
            words: [].to_vec(),
            selected_id: 1,
            error: "fall through error (update_words)".to_string(),
        };

        Ok(HttpResponse::Ok().json(res))
    }
    else {
        let res = QueryResponse {
            this_text: 1,
            words: [].to_vec(),
            selected_id: 1,
            error: "Not logged in (update_words)".to_string(),
        };

        Ok(HttpResponse::Ok().json(res))
    }
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
        affected_rows: 0,
        words: vec![gloss],
    };

    Ok(HttpResponse::Ok().json(res))
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssignmentTree {
    pub i:u32,
    pub col:Vec<String>,
    pub c:Vec<AssignmentTree>,
    pub h:bool
}

#[allow(clippy::eval_order_dependence)]
async fn get_glosses((info, req): (web::Query<WordtreeQueryRequest>, HttpRequest)) -> Result<HttpResponse, AWError> {
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

    let mut gloss_rows:Vec<AssignmentTree> = vec![];
    for r in &result_rows_stripped {
        gloss_rows.push(AssignmentTree{i:r.1,col:vec![r.0.clone(), r.1.to_string()],h:false,c:vec![]});
    }

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
        arr_options: gloss_rows//result_rows_stripped//result_rows
    };

    Ok(HttpResponse::Ok().json(res))
}

#[allow(clippy::eval_order_dependence)]
async fn get_texts((info, req): (web::Query<WordtreeQueryRequest>, HttpRequest)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();

    let query_params: WordQuery = serde_json::from_str(&info.query)?;
    let course_id = 1;
    //let seq = get_seq_by_prefix(db, table, &query_params.w).await.map_err(map_sqlx_error)?;

    //only check page 0 or page less than 0
    let vlast_page_up = 1;
    //only check page 0 or page greater than 0
    let vlast_page = 1;

    let seq = 0;

    //let result_rows = [before_rows, after_rows].concat();

    //strip any numbers from end of string
    //let re = Regex::new(r"[0-9]").unwrap();
    //let result_rows_stripped:Vec<TreeRow> = vec![TreeRow{v:"abc".to_string(), i:1, c:None}, TreeRow{v:"def".to_string(), i:2, c:Some(vec![TreeRow{v:"def2".to_string(), i:1, c:None}, TreeRow{v:"def3".to_string(), i:3, c:None}])}];
    
    let w = get_assignment_rows(db, course_id).await.map_err(map_sqlx_error)?;
    let mut assignment_rows:Vec<AssignmentTree> = vec![];
    for r in &w {
        if r.parent_id.is_none() && r.course_id.is_some() && r.course_id.unwrap() == course_id {
            let mut a = AssignmentTree{ i:r.id,col:vec![r.assignment.clone(), r.id.to_string()],h:false,c:vec![] };
            for r2 in &w {
                if r2.parent_id.is_some() && r2.parent_id.unwrap() == a.i {
                    a.h = true;
                    a.c.push(AssignmentTree{ i:r2.id,col:vec![r2.assignment.clone(), r2.id.to_string()],h:false,c:vec![] });
                }
            }
            assignment_rows.push(a);
        }
    }

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
        arr_options: assignment_rows//result_rows_stripped//result_rows
    };

    Ok(HttpResponse::Ok().json(res))
}

/* 
#[allow(clippy::eval_order_dependence)]
async fn fix_assignments_web(req: HttpRequest) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();
    fix_assignments(db).await.map_err(map_sqlx_error)?;

    Ok(HttpResponse::Ok().finish())
}
*/

#[allow(clippy::eval_order_dependence)]
async fn get_text_words((session, info, req): (Session, web::Query<QueryRequest>, HttpRequest)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();

    if get_user_id(session).is_some() {
        let course_id = 1;

        //let query_params: WordQuery = serde_json::from_str(&info.query)?;

        let text_id = match info.wordid {
            0 => info.text,
            _ => {
                get_text_id_for_word_id(db, info.wordid).await.map_err(map_sqlx_error)?
            }
        };

        let w = get_words(db, text_id, course_id).await.map_err(map_sqlx_error)?;

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
    else {
        let res = QueryResponse {
            this_text: 1,
            words: vec![],
            selected_id: 1,
            error: "Not logged in".to_string(),
        };

        Ok(HttpResponse::Ok().json(res))
    }
}
/*
#[allow(clippy::eval_order_dependence)]
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

#[derive(Debug, Serialize)]
struct LoginCheckResponse {
    is_logged_in: bool,
    user_id:u32,
}

#[derive(Debug, Serialize)]
struct ImportResponse {
    success: bool,
    words_inserted:u64,
    error:String,
}

async fn import_text((session, payload, req): (Session, Multipart, HttpRequest)) -> Result<HttpResponse> {
    let db = req.app_data::<SqlitePool>().unwrap();

    let course_id = 1;

    if let Some(user_id) = get_user_id(session) {
        let timestamp = get_timestamp();
        let updated_ip = get_ip(&req).unwrap_or_else(|| "".to_string());
        let user_agent = get_user_agent(&req).unwrap_or("");

        match process_xml::get_xml_string(payload).await {
            Ok((xml_string, title)) => {

                let words = process_xml::process_imported_text(xml_string).await;
        
                if !words.is_empty() && !title.is_empty() {
        
                    let affected_rows = add_text(db, course_id, &title, words, user_id, timestamp, &updated_ip, user_agent).await.map_err(map_sqlx_error)?;
        
                    let res = ImportResponse {
                        success: true,
                        words_inserted: affected_rows,
                        error: "".to_string(),
                    };
                    Ok(HttpResponse::Ok().json(res))
                }
                else { 
                    let res = ImportResponse {
                        success: false,
                        words_inserted: 0,
                        error: "Error importing text.".to_string(),
                    };
                    Ok(HttpResponse::Ok().json(res))
                }
            },
            Err(e) => {
                let res = ImportResponse {
                    success: false,
                    words_inserted: 0,
                    error: format!("Error importing text: invalid utf8. Valid up to position: {}.", e.valid_up_to() ),
                };
                Ok(HttpResponse::Ok().json(res))
            }
        }
    }
    else {
        let res = ImportResponse {
            success: false,
            words_inserted: 0,
            error: "Import failed: not logged in".to_string(),
        };
        Ok(HttpResponse::Ok().json(res))
        /*
        Ok(HttpResponse::BadRequest()
                .content_type("text/plain")
                .body("update_failed: not logged in"))
        */
    }
}


fn get_user_id(session:Session) -> Option<u32> {
    session.get::<u32>("user_id").unwrap_or(None)
}

#[allow(clippy::eval_order_dependence)]
async fn login_get() -> Result<HttpResponse, AWError> {

    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        //.insert_header(("X-Hdr", "sample"))
        .body(r#"<!DOCTYPE html>
<html lang="en">
    <head>
        <meta http-equiv="content-type" content="text/html; charset=utf-8">
        <title>Login</title>
        <script>
            function setTheme() {
                var mode = localStorage.getItem("mode");
                if ((window.matchMedia( "(prefers-color-scheme: dark)" ).matches || mode == "dark") && mode != "light") {
                    document.querySelector("HTML").classList.add("dark");
                }
                else {
                    document.querySelector("HTML").classList.remove("dark");
                }
            }
            setTheme();
        </script>
        <style>
            BODY { font-family:helvetica;arial;display: flex;align-items: center;justify-content: center;height: 87vh; }
            TABLE { border:2px solid black;padding: 24px;border-radius: 10px; }
            BUTTON { padding: 3px 16px; }
            .dark BODY { background-color:black;color:white; }
            .dark INPUT { background-color:black;color:white; }
            .dark TABLE { border:2px solid white; }
            .dark BUTTON { background-color:black;color:white;border:1px solid white; }
        </style>
    </head>
    <body>
        <form action="/login" method="post">
            <table>
                <tbody>
                    <tr>
                        <td>               
                            <label for="username">Username</label>
                        </td>
                        <td>
                            <input type="text" id="username" name="username">
                        </td>
                    </tr>
                    <tr>
                        <td>
                            <label for="password">Password</label>
                        </td>
                        <td>
                            <input type="password" id="password" name="password">
                        </td>
                    </tr>
                    <tr>
                        <td colspan="2" align="center">
                            <button type="submit">Login</button>
                        </td>
                    </tr>
                </tbody>
            </table>
        </form>
        <script>/*document.getElementById("username").focus();*/</script>
    </body>
</html>"#))
}

//use secrecy::Secret;
#[derive(serde::Deserialize)]
pub struct FormData {
    username: String,
    password: String, //Secret<String>,
}

fn validate_login(username:String, password:String) -> Option<u32> {
    if username.to_lowercase() == "jm" && password == "clam1234" {
        Some(3)
    }
    else if username.to_lowercase() == "ykk" && password == "greekdb555" {
        Some(4)
    }
    else if username.to_lowercase() == "hh" && password == "greekdb555" {
        Some(5)
    }
    else if username.to_lowercase() == "cd" && password == "greekdb555" {
        Some(6)
    }
    else if username.to_lowercase() == "rr" && password == "greekdb555" {
        Some(7)
    }
    else {
        None
    }
}

#[allow(clippy::eval_order_dependence)]
async fn login_post((session, form, req): (Session, web::Form<FormData>, HttpRequest)) -> Result<HttpResponse, AWError> {
    let _db = req.app_data::<SqlitePool>().unwrap(); 

    let username = form.0.username;
    let password = form.0.password;
    
    if let Some(user_id) = validate_login(username, password) {
        session.renew(); //https://www.lpalmieri.com/posts/session-based-authentication-in-rust/#4-5-2-session
        if session.insert("user_id", user_id).is_ok() {

            return Ok(HttpResponse::SeeOther()
                .insert_header((LOCATION, "/"))
                .finish());
        }
    }

    session.purge();
    Ok(HttpResponse::SeeOther()
                .insert_header((LOCATION, "/login"))
                .finish())
}

/*
async fn check_login((session, _req): (Session, HttpRequest)) -> Result<HttpResponse, AWError> {
    //session.insert("user_id", 1);
    //session.renew();
    //session.purge();
    if let Some(user_id) = get_user_id(session) {
        return Ok(HttpResponse::Ok().json(LoginCheckResponse { is_logged_in:true,user_id:user_id }));
    }
    Ok(HttpResponse::Ok().json(LoginCheckResponse { is_logged_in:false,user_id:0 }))
}
*/

/* For Basic Authentication 
use actix_web_httpauth::middleware::HttpAuthentication;
use actix_web_httpauth::extractors::basic::BasicAuth;
use actix_web_httpauth::extractors::basic::Config;
use actix_web_httpauth::extractors::AuthenticationError;
use actix_web::dev::ServiceRequest;
use std::pin::Pin;

async fn validator_basic(req: ServiceRequest, credentials: BasicAuth) -> Result<ServiceRequest, Error> {

    let config = req.app_data::<Config>()
    .map(|data| Pin::new(data).get_ref().clone())
    .unwrap_or_else(Default::default);

    match validate_credentials_basic(credentials.user_id(), credentials.password().unwrap().trim()) {
        Ok(res) => {
            if res {
                Ok(req)
            } else {
                Err(AuthenticationError::from(config).into())
            }
        }
        Err(_) => Err(AuthenticationError::from(config).into()),
    }
}

fn validate_credentials_basic(user_id: &str, user_password: &str) -> Result<bool, std::io::Error> {
    if user_id.eq("greekdb") && user_password.eq("pass") {
        return Ok(true);
    }
    Err(std::io::Error::new(std::io::ErrorKind::Other, "Authentication failed!"))
}
*/

#[actix_web::main]
async fn main() -> io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();

    //e.g. export GKVOCABDB_DB_PATH=sqlite://gkvocabnew.sqlite?mode=rwc
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
        
        //let auth_basic = HttpAuthentication::basic(validator_basic);
        App::new()
            //.wrap(json_cfg)
            .app_data(db_pool.clone())
            
            .wrap(middleware::Logger::default())
            //.wrap(auth_basic) //this blocks healthcheck
            .wrap(CookieSession::signed(&[0; 32])
                .secure(false)
                //.expires_in(2147483647) //deprecated
                .max_age(2147483647))
            .wrap(middleware::Compress::default())
            //.wrap(error_handlers)
            .route("/login", web::get().to(login_get))
            .route("/login", web::post().to(login_post))
            /* 
            .service(
                web::resource("/checklogin")
                    .route(web::get().to(check_login)),
            )*/
            /* .service(
                web::resource("/fixassignmentstemp")
                    .route(web::get().to(fix_assignments_web)),
            )*/
            .service(
                web::resource("/query")
                    .route(web::get().to(get_text_words)),
            )
            .service(
                web::resource("/queryglosses")
                    .route(web::get().to(get_glosses)),
            )
            .service(
                web::resource("/querytexts")
                    .route(web::get().to(get_texts)),
            )
            /* .service(
                web::resource("/assignments")
                    .route(web::get().to(get_assignments)),
            )*/
            .service(
                web::resource("/getgloss")
                    .route(web::post().to(get_gloss)),
            )
            .service(
                web::resource("/updatedb") //checks session
                    .route(web::post().to(update_words)),
            )
            .service(
                web::resource("/updategloss") //checks session
                    .route(web::post().to(update_or_add_gloss)),
            )
            .service(
                web::resource("/importtext") //checks session
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

//https://users.rust-lang.org/t/file-upload-in-actix-web/64871/3
pub mod process_xml {

    use quick_xml::Reader;
    use quick_xml::events::Event;

    use actix_multipart::Multipart;
    use futures::{StreamExt, TryStreamExt};

    use super::*;

    //select b.gloss_id,b.lemma,count(b.gloss_id) c from words a inner join glosses b on a.gloss_id=b.gloss_id group by b.gloss_id order by c;
    fn lemmatize_simple(word:&str) -> Option<u32> {
        match word {
            "ἀεί" => Some(260),
            "ἀεὶ" => Some(260),
            "ἀλλ" => Some(54),
            "ἀλλ'" => Some(54),
            "ἀλλ᾽" => Some(54),
            "ἀλλὰ" => Some(54),
            "ἀλλά" => Some(54),
            "ἅμα" => Some(191),
            "ἄν" => Some(78),
            "ἂν" => Some(78),
            "ἆρα" => Some(28),
            "αὖ" => Some(483),
            "γὰρ" => Some(29),
            "γάρ" => Some(29),
            "γε" => Some(135),
            "δ'" => Some(30),
            "δ᾽" => Some(30),
            "δ" => Some(30),
            "δέ" => Some(30),
            "δὲ" => Some(30),
            "δὴ" => Some(59),
            "δή" => Some(59),
            "διά" => Some(61),
            "διὰ" => Some(61),
            "ἐγώ" => Some(392),
            "ἐγὼ" => Some(392),
            "εἰ" => Some(88),
            "εἴ" => Some(88),
            "εἰς" => Some(6),
            "εἴτε" => Some(488),
            "ἐκ" => Some(7),
            "ἐξ" => Some(7),
            "ἐν" => Some(8),
            "ἐπεί" => Some(64),
            "ἐπεὶ" => Some(64),
            "ἐπειδή" => Some(65),
            "ἐπειδὴ" => Some(65),
            "ἔπειτα" => Some(193),
            "ἐπί" => Some(333),
            "ἐπὶ" => Some(333),
            "ἐπ" => Some(333),
            "ἐς" => Some(6),
            "ἔτι" => Some(367),
            "εὖ" => Some(32),
            "ἢ" => Some(34),
            "ἤ" => Some(34),
            "ἤδη" => Some(640),
            "καὶ" => Some(11),
            "καί" => Some(11),
            "καίτοι" => Some(93),
            "κατά" => Some(146),
            "κατὰ" => Some(146),
            "μάλα" => Some(515),
            "μᾶλλον" => Some(310),
            "μέν" => Some(39),
            "μὲν" => Some(39),
            "μέντοι" => Some(598),
            "μετά" => Some(96),
            "μετὰ" => Some(96),
            "μή" => Some(69),
            "μὴ" => Some(69),
            "μηδέ" => Some(312),
            "μηδὲ" => Some(312),
            "μὴν" => Some(555),
            "μήν" => Some(555),
            "μήτε" => Some(196),
            "νῦν" => Some(40),
            "ὅπως" => Some(348),
            "ὅταν" => Some(282),
            "ὅτι" => Some(435),
            "οὐ" => Some(42),
            "οὐκ" => Some(42),
            "οὖν" => Some(182),
            "οὔτε" => Some(200),
            "οὐχ" => Some(42),
            "παρά" => Some(44),
            "παρὰ" => Some(44),
            "περί" => Some(74),
            "περὶ" => Some(74),
            "που" => Some(319),
            "πρίν" => Some(521),
            "πρὶν" => Some(521),
            "πρός" => Some(320),
            "πρὸς" => Some(320),
            "πῶς" => Some(284),
            "σύ" => Some(408),
            "σὺ" => Some(408),
            "τ" => Some(157),
            "τε" => Some(157),
            "τι" => Some(414),
            "τότε" => Some(286),
            "ὑπό" => Some(131),
            "ὑπὸ" => Some(131),
            "χρή" => Some(537),
            "χρὴ" => Some(537),
            "ὦ" => Some(25),
            "ὡς" => Some(76),
            "ὥστε" => Some(259),
            _ => None
        }
    }

    fn split_words(text: &str, in_speaker:bool, in_head:bool) -> Vec<TextWord> {
        let mut words:Vec<TextWord> = vec![];
        let mut last = 0;
        if in_head {
            words.push(TextWord{word: text.to_string(),word_type:WordType::WorkTitle as u32, gloss_id:None});
        }
        else if in_speaker {
            words.push(TextWord{word: text.to_string(),word_type:WordType::Speaker as u32, gloss_id:None});
        }
        else {
            for (index, matched) in text.match_indices(|c: char| !(c.is_alphanumeric() || c == '\'')) {
                //add words
                if last != index && &text[last..index] != " " {
                    let gloss_id = lemmatize_simple(&text[last..index]);
                    words.push(TextWord{word: text[last..index].to_string(), word_type: WordType::Word as u32, gloss_id: gloss_id});
                }
                //add word separators
                if matched != " " {
                    words.push(TextWord{word:matched.to_string(),word_type:WordType::Punctuation as u32, gloss_id:None});
                }
                last = index + matched.len();
            }
            //add last word
            if last < text.len() && &text[last..] != " " {
                let gloss_id = lemmatize_simple(&text[last..]);
                words.push(TextWord{word:text[last..].to_string(),word_type:WordType::Word as u32, gloss_id:gloss_id});
            }
        }
        words
    }

    pub async fn get_xml_string(mut payload: Multipart) -> Result<(String, String), std::str::Utf8Error> {
        let mut xml_string = "".to_string();
        let mut title:String = "".to_string();
        
        // iterate over multipart stream
        while let Ok(Some(mut field)) = payload.try_next().await {
            let content_type = field.content_disposition();//.unwrap();
            //if let Some(filename) = content_type.get_filename() {
            //    println!("file: {}", filename);
            //}
            let name = content_type.get_name().unwrap_or("").to_string();

            //let filepath = format!(".{}", file_path);

            // File::create is blocking operation, use threadpool
            //let mut f = web::block(|| std::fs::File::create(filepath))
            //    .await
            //    .unwrap();

            // Field in turn is stream of *Bytes* object
            while let Some(chunk) = field.next().await {
                let data = chunk.unwrap();
                match std::str::from_utf8(&data) {
                    Ok(xml_data) => {
                        if name == "title" {
                            title = xml_data.to_string();
                        }
                        else if name == "file" {
                            xml_string.push_str(xml_data);
                        }
                    },
                    Err(e) => {
                        return Err(e); //utf8 error
                    }
                // filesystem operations are blocking, we have to use threadpool
                /*f = web::block(move || f.write_all(&data).map(|_| f))
                    .await
                    .unwrap();*/
                }
            }
        }
        Ok((xml_string, title))
    }

    pub async fn process_imported_text(xml_string: String) -> Vec<TextWord> {
        let mut words:Vec<TextWord> = Vec::new();

        let mut reader = Reader::from_str(&xml_string);
        reader.trim_text(true);
        let mut buf = Vec::new();

        let mut in_text = false;
        let mut in_speaker = false;
        let mut in_head = false;
        /*
        TEI: verse lines can either be empty <lb n="5"/>blah OR <l n="5">blah</l> 
        see Perseus's Theocritus for <lb/> and Euripides for <l></l>
        */

        loop {
            match reader.read_event(&mut buf) {
            // for triggering namespaced events, use this instead:
            // match reader.read_namespaced_event(&mut buf) {
                Ok(Event::Start(ref e)) => {
                // for namespaced:
                // Ok((ref namespace_value, Event::Start(ref e)))                
                    if b"text" == e.name() { in_text = true }
                    else if b"speaker" == e.name() { in_speaker = true }
                    else if b"head" == e.name() { in_head = true }
                    else if b"l" == e.name() { 
                        let mut line_num = "".to_string();
                        
                        for a in e.attributes().into_iter() { //.next().unwrap().unwrap();
                            if std::str::from_utf8(a.as_ref().unwrap().key).unwrap() == "n" {         
                                line_num = std::str::from_utf8(&*a.unwrap().value).unwrap().to_string();
                            }
                        }
                        words.push( TextWord{ word: format!("[line]{}", line_num), word_type: WordType::VerseLine as u32,gloss_id:None }); 
                    }
                },
                // unescape and decode the text event using the reader encoding
                Ok(Event::Text(e)) => { 
                    if in_text {
                        if let Ok(s) = e.unescape_and_decode(&reader) {
                            
                            //let seperator = Regex::new(r"([ ,.;]+)").expect("Invalid regex");
                            words.extend_from_slice(&split_words(&s, in_speaker, in_head)[..]);

                            //let mut splits: Vec<String> = s.split_inclusive(&['\t','\n','\r',' ',',', ';','.']).map(|s| s.to_string()).collect();
                            //words2.word.extend_from_slice(&words.word[..]);
                            //words2.word_type.extend_from_slice(&words.word_type[..]);
                        }
                    }
                },
                Ok(Event::Empty(ref e)) => {
                    if b"lb" == e.name() { 
                        let mut line_num = "".to_string();
                        
                        for a in e.attributes().into_iter() { //.next().unwrap().unwrap();
                            if std::str::from_utf8(a.as_ref().unwrap().key).unwrap() == "n" {         
                                line_num = std::str::from_utf8(&*a.unwrap().value).unwrap().to_string();
                            }
                        }
                        words.push( TextWord{ word: format!("[line]{}", line_num), word_type: WordType::VerseLine as u32,gloss_id:None }); 
                    }
                },
                Ok(Event::End(ref e)) => {
                    if b"text" == e.name() { in_text = false }     
                    else if b"speaker" == e.name() { in_speaker = false }
                    else if b"head" == e.name() { in_head = false }
               },
                Ok(Event::Eof) => break, // exits the loop when reaching end of file
                Err(_e) => { words.clear(); return words }, //return empty vec on error //panic!("Error at position {}: {:?}", reader.buffer_position(), e),
                _ => (), // There are several other `Event`s we do not consider here
            }
        
            // if we don't keep a borrow elsewhere, we can clear the buffer to keep memory usage low
            buf.clear();
        }
        /* 
        for a in words {
            println!("{} {}", a.word, a.word_type);
        }*/
        words
    }
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

    async fn create_db(db: &SqlitePool) -> Result<(), sqlx::Error> {
        let query = "CREATE TABLE users (user_id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL);";
        sqlx::query(query)
            .execute(db).await?;

        Ok(())
    }

    #[actix_web::test]
    async fn test_query_paging() {
        let db_path = std::env::var("GKVOCABDB_DB_PATH")
            .unwrap_or_else(|_| panic!("Environment variable for sqlite path not set: GKVOCABDB_DB_PATH."));

        let db_pool = SqlitePool::connect(&db_path).await.expect("Could not connect to db.");
        /*
        let db_pool = SqlitePool::connect("sqlite::memory:").await.expect("Could not connect to db.");
        create_db(&db_pool);
        */

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
            .uri(r#"/healthzzz"#) //400 Bad Request error if all params not present
            .send_request(&mut app).await;
        assert!(&resp.status().is_success()); //health check

        let resp = test::TestRequest::get()
            .uri(r#"/query?text=100&wordid=0"#) //400 Bad Request error if all params not present
            .send_request(&mut app).await;

        assert!(&resp.status().is_success());
        //println!("resp: {:?}", resp);
        let result: QueryResponse = test::read_body_json(resp).await;
        //println!("res: {:?}", result);
        assert_eq!(result.words.len(), 176);
    }
}

