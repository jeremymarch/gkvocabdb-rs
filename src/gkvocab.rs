use super::*;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateGlossResponse {
    pub qtype: String,
    pub success: bool,
    pub affectedrows: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateGlossIdResponse {
    pub qtype: String,
    pub words: Vec<SmallWord>,
    pub success: bool,
    pub affectedrows: u32,
}

pub async fn gkv_arrow_word(db: &SqlitePool, post: &ArrowWordRequest, info: &ConnectionInfo, course_id:u32) -> Result<ArrowWordResponse, AWError> {
    arrow_word(
        db,
        course_id,
        post.for_lemma_id.unwrap(),
        post.set_arrowed_id_to.unwrap(),
        info,
    ).await
    .map_err(map_sqlx_error)?;
    Ok(ArrowWordResponse {
        success: true,
        affected_rows: 1,
        arrowed_value: 1,
        lemmaid: 1,
    })
}

pub async fn gkv_update_gloss_id(db: &SqlitePool, gloss_id:u32, text_word_id:u32, info: &ConnectionInfo, course_id:u32) -> Result<UpdateGlossIdResponse, AWError> {

    let words = set_gloss_id(
        db,
        course_id,
        gloss_id,
        text_word_id,
        info,
    ).await
    .map_err(map_sqlx_error)?;

    Ok(UpdateGlossIdResponse {
        qtype: "set_gloss".to_string(),
        words,
        success: true,
        affectedrows: 1,
    })
}

pub async fn gkv_update_or_add_gloss(db: &SqlitePool, post: &UpdateGlossRequest, info: &ConnectionInfo) -> Result<UpdateGlossResponse, AWError> {
    match post.qtype.as_str() {
        "newlemma" => {
            let rows_affected = insert_gloss(
                db,
                &post.lemma,
                &post.pos,
                &post.def,
                &post.stripped_lemma,
                &post.note,
                info,
            )
            .await
            .map_err(map_sqlx_error)?;

            return Ok(UpdateGlossResponse {
                qtype: post.qtype.to_string(),
                success: true,
                affectedrows: rows_affected,
            })
        }
        "editlemma" => {
            if post.hqid.is_some() {
                let rows_affected = update_gloss(
                    db,
                    post.hqid.unwrap(),
                    &post.lemma,
                    &post.pos,
                    &post.def,
                    &post.stripped_lemma,
                    &post.note,
                    info,
                )
                .await
                .map_err(map_sqlx_error)?;

                return Ok(UpdateGlossResponse {
                    qtype: post.qtype.to_string(),
                    success: true,
                    affectedrows: rows_affected,
                })
            }
        }
        "deletegloss" => {
            if post.hqid.is_some() {
                let rows_affected = delete_gloss(
                    db,
                    post.hqid.unwrap(),
                    info,
                )
                .await
                .map_err(map_sqlx_error)?;

                return Ok(UpdateGlossResponse {
                    qtype: post.qtype.to_string(),
                    success: true,
                    affectedrows: rows_affected,
                })
            }
        }
        _ => return Ok(UpdateGlossResponse {
            qtype: post.qtype.to_string(),
            success: false,
            affectedrows: 0,
        })
    }
    Ok(UpdateGlossResponse {
        qtype: post.qtype.to_string(),
        success: false,
        affectedrows: 0,
    })
}

pub async fn gkv_tet_gloss(db: &SqlitePool, post: &GetGlossRequest) -> Result<GetGlossResponse, AWError> {
 
    let gloss = get_glossdb(db, post.lemmaid)
        .await
        .map_err(map_sqlx_error)?;

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

pub async fn gkv_get_glosses(db:&SqlitePool, info:&WordtreeQueryRequest) -> Result<WordtreeQueryResponse, AWError> {
    let query_params: WordQuery = serde_json::from_str(&info.query)?;

    //let seq = get_seq_by_prefix(db, table, &query_params.w).await.map_err(map_sqlx_error)?;

    let mut before_rows = vec![];
    let mut after_rows = vec![];
    if info.page <= 0 {
        before_rows = get_before(db, &query_params.w, info.page, info.n)
            .await
            .map_err(map_sqlx_error)?;
        if info.page == 0 {
            //only reverse if page 0. if < 0, each row is inserted under top of container one-by-one in order
            before_rows.reverse();
        }
    }
    if info.page >= 0 {
        after_rows = get_equal_and_after(db, &query_params.w, info.page, info.n)
            .await
            .map_err(map_sqlx_error)?;
    }

    //only check page 0 or page less than 0
    let vlast_page_up = if before_rows.len() < info.n as usize && info.page <= 0 {
        1
    } else {
        0
    };
    //only check page 0 or page greater than 0
    let vlast_page = if after_rows.len() < info.n as usize && info.page >= 0 {
        1
    } else {
        0
    };

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
        nocache: if query_params.wordid.is_none() { 0 } else { 1 }, //prevents caching when queried by wordid in url
        container: format!("{}Container", info.idprefix),
        request_time: info.request_time,
        page: info.page,
        last_page: vlast_page,
        lastpage_up: vlast_page_up,
        scroll: if query_params.w.is_empty() && info.page == 0 && seq == 1 {
            "top".to_string()
        } else {
            "".to_string()
        }, //scroll really only needs to return top
        query: query_params.w.to_owned(),
        arr_options: gloss_rows, //result_rows_stripped//result_rows
    })
}

pub async fn gkv_get_occurrences(db:&SqlitePool, info:&WordtreeQueryRequest) -> Result<WordtreeQueryResponse, AWError> {
    let query_params: WordQuery = serde_json::from_str(&info.query)?;

    //only check page 0 or page less than 0
    let vlast_page_up = 1;
    //only check page 0 or page greater than 0
    let vlast_page = 1;

    let course_id = 1;
    let gloss_id = query_params.tag_id.unwrap_or(0);

    let result_rows = get_gloss_occurrences(db, course_id, gloss_id)
        .await
        .map_err(map_sqlx_error)?;

    let result_rows_formatted: Vec<(String, u32)> = result_rows
        .into_iter()
        .enumerate()
        .map(|(i, mut row)| {
            row.name = format!(
                "{}. <b class='occurrencesarrow'>{}</b> {} - {}",
                i + 1,
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
        scroll: "top".to_string(), //scroll really only needs to return top
        query: query_params.w.to_owned(),
        arr_options: gloss_rows, //result_rows_stripped//result_rows
    })
}

pub async fn gkv_update_log(db:&SqlitePool, info:&WordtreeQueryRequest) -> Result<WordtreeQueryResponse, AWError> {
    let query_params: WordQuery = serde_json::from_str(&info.query)?;
    let course_id = 1;

    let log = get_update_log(db, course_id)
        .await
        .map_err(map_sqlx_error)?;

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
        scroll: "top".to_string(),
        query: query_params.w.to_owned(),
        arr_options: log,
    })
}

pub async fn gkv_get_texts(db:&SqlitePool, info:&WordtreeQueryRequest) -> Result<WordtreeQueryResponse, AWError> {
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

    let w = get_texts_db(db, course_id)
        .await
        .map_err(map_sqlx_error)?;
    let mut assignment_rows: Vec<AssignmentTree> = vec![];
    for r in &w {
        if r.parent_id.is_none() && r.course_id.is_some() && r.course_id.unwrap() == course_id {
            let mut a = AssignmentTree {
                i: r.id,
                col: vec![r.assignment.clone(), r.id.to_string()],
                h: false,
                c: vec![],
            };
            for r2 in &w {
                if r2.parent_id.is_some() && r2.parent_id.unwrap() == a.i {
                    a.h = true;
                    a.c.push(AssignmentTree {
                        i: r2.id,
                        col: vec![r2.assignment.clone(), r2.id.to_string()],
                        h: false,
                        c: vec![],
                    });
                }
            }
            assignment_rows.push(a);
        }
    }

    Ok(WordtreeQueryResponse {
        select_id: Some(seq),
        error: "".to_owned(),
        wtprefix: info.idprefix.clone(),
        nocache: if query_params.wordid.is_none() { 0 } else { 1 }, //prevents caching when queried by wordid in url
        container: format!("{}Container", info.idprefix),
        request_time: info.request_time,
        page: info.page,
        last_page: vlast_page,
        lastpage_up: vlast_page_up,
        scroll: if query_params.w.is_empty() && info.page == 0 && seq == 1 {
            "top".to_string()
        } else {
            "".to_string()
        }, //scroll really only needs to return top
        query: query_params.w.to_owned(),
        arr_options: assignment_rows, //result_rows_stripped//result_rows
    })
}

pub async fn gkv_get_text_words(db:&SqlitePool, info:&QueryRequest, selected_word_id:Option<u32>) -> Result<MiscErrorResponse, AWError> {
    let course_id = 1;

    //let query_params: WordQuery = serde_json::from_str(&info.query)?;

    let text_id = match info.wordid {
        0 => info.text,
        _ => get_text_id_for_word_id(db, info.wordid)
            .await
            .map_err(map_sqlx_error)?,
    };

    let w = get_words(db, text_id, course_id)
        .await
        .map_err(map_sqlx_error)?;

    let text_name = get_text_name(db, text_id)
        .await
        .map_err(map_sqlx_error)?;

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
        error: "".to_string(),
    })
}

pub async fn gkv_create_db(db:&SqlitePool) -> Result<(), AWError> {
    db::create_db(db).await.map_err(map_sqlx_error)?;
    Ok(())
}

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

pub fn map_sqlx_error(e: sqlx::Error) -> PhilologusError {
    match e {
        sqlx::Error::Configuration(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: format!("sqlx Configuration: {}", e),
        },
        sqlx::Error::Database(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: format!("sqlx Database: {}", e),
        },
        sqlx::Error::Io(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: format!("sqlx Io: {}", e),
        },
        sqlx::Error::Tls(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: format!("sqlx Tls: {}", e),
        },
        sqlx::Error::Protocol(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: format!("sqlx Protocol: {}", e),
        },
        sqlx::Error::RowNotFound => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: "sqlx RowNotFound".to_string(),
        },
        sqlx::Error::TypeNotFound { .. } => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: "sqlx TypeNotFound".to_string(),
        },
        sqlx::Error::ColumnIndexOutOfBounds { .. } => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: "sqlx ColumnIndexOutOfBounds".to_string(),
        },
        sqlx::Error::ColumnNotFound(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: format!("sqlx ColumnNotFound: {}", e),
        },
        sqlx::Error::ColumnDecode { .. } => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: "sqlx ColumnDecode".to_string(),
        },
        sqlx::Error::Decode(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: format!("sqlx Decode: {}", e),
        },
        sqlx::Error::PoolTimedOut => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: "sqlx PoolTimeOut".to_string(),
        },
        sqlx::Error::PoolClosed => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: "sqlx PoolClosed".to_string(),
        },
        sqlx::Error::WorkerCrashed => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: "sqlx WorkerCrashed".to_string(),
        },
        sqlx::Error::Migrate(e) => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: format!("sqlx Migrate: {}", e),
        },
        _ => PhilologusError {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            name: "sqlx error".to_string(),
            error: "sqlx Unknown error".to_string(),
        },
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

    async fn set_up() -> (SqlitePool, ConnectionInfo) {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("Could not connect to db.")
        .foreign_keys(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .read_only(false)
        .collation("PolytonicGreek", |l, r| {
            l.to_lowercase().cmp(&r.to_lowercase())
        });
        let db = SqlitePool::connect_with(options).await.expect("Could not connect to db.");

        let _res = gkv_create_db(&db).await.expect("Could not create db.");
        let user_id = db::insert_user(&db, "testuser", "tu", 0, "12341234", "tu@blah.com").await.unwrap();
        let info = ConnectionInfo {
            user_id: user_id.try_into().unwrap(),
            timestamp: get_timestamp(),
            ip_address: "0.0.0.0".to_string(),
            user_agent: "test_agent".to_string(),
        };
        (db, info)
    }

    #[actix_rt::test]
    async fn import_basic_text() {
        let (db, user_info) = set_up().await;
        let course_id = 1;

        //empty title fails
        let title = "";
        let xml_string = "<TEI.2><text>blahblah</text></TEI.2>";
        let res = import_text_xml::import(&db, course_id, &user_info, &title, &xml_string).await;
        assert!(!res.success);

        //empty title xml fails
        let xml_string = "";
        let res = import_text_xml::import(&db, course_id, &user_info, &title, &xml_string).await;
        assert!(!res.success);

        let title = "testtext";

        //no TEI or TEI.2 tags
        let xml_string = "<TE><text>blahblah</text></TE>";
        let res = import_text_xml::import(&db, course_id, &user_info, &title, &xml_string).await;
        assert!(!res.success);

        //xml has tags, but no text fails
        let xml_string = "<TEI.2><text></text></TEI.2>";
        let res = import_text_xml::import(&db, course_id, &user_info, &title, &xml_string).await;
        assert!(!res.success);

        //pass with TEI.2
        let xml_string = "<TEI.2><text>blahblah</text></TEI.2>";
        let res = import_text_xml::import(&db, course_id, &user_info, &title, &xml_string).await;
        assert!(res.success);

        //pass with TEI
        let xml_string = "<TEI><text>blahblah</text></TEI>";
        let res = import_text_xml::import(&db, course_id, &user_info, &title, &xml_string).await;
        assert!(res.success);
    }

    #[actix_rt::test]
    async fn insert_gloss() {
        let (db, user_info) = set_up().await;

        let post = UpdateGlossRequest {
            qtype: "newlemma".to_string(),
            hqid: None,
            lemma: "newword".to_string(),
            stripped_lemma: "newword".to_string(),
            pos: "newpos".to_string(),
            def: "newdef".to_string(),
            note: "newnote".to_string(),
        };
        let res = gkv_update_or_add_gloss(&db, &post, &user_info).await;

        assert!(res.is_ok());
    }
}
