use super::*;

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
    PageBreak = 11,
    Desc = 12,
    InvalidType = 13,
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
    pub select_id: Option<i32>,
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
    pub arr_options: Vec<AssignmentTree>, //Vec<(String,i32)>
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ArrowWordResponse {
    pub success: bool,
    #[serde(
        rename(serialize = "affectedRows"),
        rename(deserialize = "affectedRows")
    )]
    pub affected_rows: i32,
    #[serde(
        rename(serialize = "arrowedValue"),
        rename(deserialize = "arrowedValue")
    )]
    pub arrowed_value: i32,
    pub lemmaid: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct UpdateGlossIdResponse {
    pub qtype: String,
    pub words: Vec<SmallWord>,
    pub success: bool,
    pub affectedrows: i32,
}

pub async fn gkv_arrow_word(
    db: &AnyPool,
    post: &ArrowWordRequest,
    info: &ConnectionInfo,
    course_id: i32,
) -> Result<ArrowWordResponse, AWError> {
    arrow_word(
        db,
        course_id,
        post.for_lemma_id.unwrap(),
        post.set_arrowed_id_to.unwrap(),
        info,
    )
    .await
    .map_err(map_sqlx_error)?;
    Ok(ArrowWordResponse {
        success: true,
        affected_rows: 1,
        arrowed_value: 1,
        lemmaid: 1,
    })
}

pub async fn gkv_update_gloss_id(
    db: &AnyPool,
    gloss_id: i32,
    text_word_id: i32,
    info: &ConnectionInfo,
    course_id: i32,
) -> Result<UpdateGlossIdResponse, AWError> {
    let words = set_gloss_id(db, course_id, gloss_id, text_word_id, info)
        .await
        .map_err(map_sqlx_error)?;

    Ok(UpdateGlossIdResponse {
        qtype: "set_gloss".to_string(),
        words,
        success: true,
        affectedrows: 1,
    })
}

pub async fn gkv_update_or_add_gloss(
    db: &AnyPool,
    post: &UpdateGlossRequest,
    info: &ConnectionInfo,
) -> Result<UpdateGlossResponse, AWError> {
    match post.qtype.as_str() {
        "newlemma" => {
            let (inserted_id, rows_affected) = insert_gloss(
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
                inserted_id: Some(inserted_id),
            });
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
                    inserted_id: None,
                });
            }
        }
        "deletegloss" => {
            if post.hqid.is_some() {
                let rows_affected = delete_gloss(db, post.hqid.unwrap(), info)
                    .await
                    .map_err(map_sqlx_error)?;

                return Ok(UpdateGlossResponse {
                    qtype: post.qtype.to_string(),
                    success: true,
                    affectedrows: rows_affected,
                    inserted_id: None,
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

pub async fn gkv_tet_gloss(
    db: &AnyPool,
    post: &GetGlossRequest,
) -> Result<GetGlossResponse, AWError> {
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

pub async fn gkv_get_glosses(
    db: &AnyPool,
    info: &WordtreeQueryRequest,
) -> Result<WordtreeQueryResponse, AWError> {
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
    let result_rows_stripped: Vec<(String, i32)> = result_rows
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
            "top".to_string()
        } else {
            "".to_string()
        }, //scroll really only needs to return top
        query: query_params.w.to_owned(),
        arr_options: gloss_rows, //result_rows_stripped//result_rows
    })
}

pub async fn gkv_get_occurrences(
    db: &AnyPool,
    info: &WordtreeQueryRequest,
) -> Result<WordtreeQueryResponse, AWError> {
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

    //start numbering at 0 if H&Q, so running_count is correct
    let start_idx =
        usize::from(result_rows.is_empty() || !result_rows[0].name.starts_with("H&Q Unit"));

    let result_rows_formatted: Vec<(String, i32)> = result_rows
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
        scroll: "top".to_string(), //scroll really only needs to return top
        query: query_params.w.to_owned(),
        arr_options: gloss_rows, //result_rows_stripped//result_rows
    })
}

pub async fn gkv_update_log(
    db: &AnyPool,
    info: &WordtreeQueryRequest,
) -> Result<WordtreeQueryResponse, AWError> {
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

pub async fn gkv_get_texts(
    db: &AnyPool,
    info: &WordtreeQueryRequest,
) -> Result<WordtreeQueryResponse, AWError> {
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

    let w = get_texts_db(db, course_id).await.map_err(map_sqlx_error)?;
    let mut assignment_rows: Vec<AssignmentTree> = vec![];
    let mut last_container_id: i64 = -1;

    let use_containers = false;

    for r in &w {
        if use_containers {
            if r.container_id.is_some()
                && r.container.is_some()
                && r.container_id.unwrap() != last_container_id as i32
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
            "top".to_string()
        } else {
            "".to_string()
        }, //scroll really only needs to return top
        query: query_params.w.to_owned(),
        arr_options: assignment_rows, //result_rows_stripped//result_rows
    })
}

pub async fn gkv_move_text(
    db: &AnyPool,
    text_id: i32,
    step: i32,
    _info: &ConnectionInfo,
    course_id: i32,
) -> Result<(), AWError> {
    Ok(db::update_text_order_db(db, course_id, text_id, step)
        .await
        .map_err(map_sqlx_error)?)
}

pub async fn gkv_get_text_words(
    db: &AnyPool,
    info: &QueryRequest,
    selected_word_id: Option<i32>,
) -> Result<MiscErrorResponse, AWError> {
    let course_id = 1;

    //let query_params: WordQuery = serde_json::from_str(&info.query)?;

    let text_id = match info.wordid {
        0 => info.text,
        _ => get_text_id_for_word_id(db, info.wordid)
            .await
            .map_err(map_sqlx_error)?,
    };

    let w = db::get_words(db, text_id, course_id)
        .await
        .map_err(map_sqlx_error)?;

    let text_name = db::get_text_name(db, text_id)
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

pub async fn gkv_create_db(db: &AnyPool) -> Result<(), AWError> {
    Ok(db::create_db(db).await.map_err(map_sqlx_error)?)
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

    async fn set_up() -> (AnyPool, ConnectionInfo) {
        // let options = SqliteConnectOptions::from_str("sqlite::memory:")
        //     .expect("Could not connect to db.")
        //     .foreign_keys(true)
        //     .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        //     .read_only(false)
        //     .collation("PolytonicGreek", |l, r| {
        //         l.to_lowercase().cmp(&r.to_lowercase())
        //     });
        //let db_string = "sqlite::memory:".to_string();
        //let db_string = "sqlite://aaa.db".to_string();
        //let db_string = "sqlite://gkvocabnew.sqlite?mode=rwc".to_string();
        let db_string = "postgres://jwm:1234@localhost/gk1".to_string();
        let db = get_db(&db_string).await.expect("Could not connect to db."); //AnyPool::connect_with(options)
                                                                              // .await
                                                                              // .expect("Could not connect to db.");

        gkv_create_db(&db).await.expect("Could not create db.");
        println!("aaaa");
        let user_id = db::insert_user(&db, "testuser", "tu", 0, "12341234", "tu@blah.com")
            .await
            .unwrap();
        println!("bbb");
        let info = ConnectionInfo {
            user_id: user_id.try_into().unwrap(),
            timestamp: get_timestamp(),
            ip_address: "0.0.0.0".to_string(),
            user_agent: "test_agent".to_string(),
        };
        (db, info)
    }

    async fn setup_text_test(
        db: &AnyPool,
        course_id: i32,
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
                qtype: "newlemma".to_string(),
                hqid: None,
                lemma: "newword".to_string(),
                stripped_lemma: "newword".to_string(),
                pos: "newpos".to_string(),
                def: "newdef".to_string(),
                note: "newnote".to_string(),
            };
            let _ = gkv_update_or_add_gloss(db, &post, user_info).await;
        }

        import_text_xml::import(db, course_id, user_info, title, xml_string).await
    }

    async fn setup_small_text_test(
        db: &AnyPool,
        course_id: i32,
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
                qtype: "newlemma".to_string(),
                hqid: None,
                lemma: "newword".to_string(),
                stripped_lemma: "newword".to_string(),
                pos: "newpos".to_string(),
                def: "newdef".to_string(),
                note: "newnote".to_string(),
            };
            let _ = gkv_update_or_add_gloss(db, &post, user_info).await;
        }

        import_text_xml::import(db, course_id, user_info, title, xml_string).await
    }

    #[actix_rt::test]
    async fn import_basic_text() {
        let (db, user_info) = set_up().await;
        let course_id = 1;

        //empty title fails
        let title = "";
        let xml_string = "<TEI.2><text>blahblah</text></TEI.2>";
        let res = import_text_xml::import(&db, course_id, &user_info, title, xml_string).await;
        assert!(!res.success);

        //empty title xml fails
        let xml_string = "";
        let res = import_text_xml::import(&db, course_id, &user_info, title, xml_string).await;
        assert!(!res.success);

        let title = "testtext";

        //no TEI or TEI.2 tags
        let xml_string = "<TE><text>blahblah</text></TE>";
        let res = import_text_xml::import(&db, course_id, &user_info, title, xml_string).await;
        assert!(!res.success);

        //xml has tags, but no text fails
        let xml_string = "<TEI.2><text></text></TEI.2>";
        let res = import_text_xml::import(&db, course_id, &user_info, title, xml_string).await;
        assert!(!res.success);

        //pass with TEI.2
        let xml_string = "<TEI.2><text>blahblah</text></TEI.2>";
        let res = import_text_xml::import(&db, course_id, &user_info, title, xml_string).await;
        assert!(res.success);

        //pass with TEI
        let xml_string = "<TEI><text>blahblah</text></TEI>";
        let res = import_text_xml::import(&db, course_id, &user_info, title, xml_string).await;
        assert!(res.success);

        let res = setup_text_test(&db, course_id, &user_info).await;
        assert!(res.success);
    }

    #[actix_rt::test]
    async fn lemmatizer_test() {
        let (db, user_info) = set_up().await;
        let course_id = 1;

        //insert gloss before adding it to the lemmatizer because of foreign key
        let post = UpdateGlossRequest {
            qtype: "newlemma".to_string(),
            hqid: None,
            lemma: "newword".to_string(),
            stripped_lemma: "newword".to_string(),
            pos: "newpos".to_string(),
            def: "newdef".to_string(),
            note: "newnote".to_string(),
        };
        let _ = gkv_update_or_add_gloss(&db, &post, &user_info).await;

        //add to lemmatizer
        db::insert_lemmatizer_form(&db, "ὥστε", 1).await;

        let title = "title";
        let xml_string = "<TEI.2><text>blah ὥστε δὲ</text></TEI.2>";
        let res = import_text_xml::import(&db, course_id, &user_info, title, xml_string).await;
        assert!(res.success);

        //check gkv_get_text_words
        let info = QueryRequest { text: 1, wordid: 0 };
        let selected_word_id = None;
        let res = gkv_get_text_words(&db, &info, selected_word_id).await;

        assert_eq!(
            res.unwrap(),
            MiscErrorResponse {
                this_text: 1,
                text_name: "title".to_string(),
                words: [
                    WordRow {
                        wordid: 1,
                        word: "blah".to_string(),
                        word_type: 0,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 1,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 2,
                        word: "ὥστε".to_string(),
                        word_type: 0,
                        lemma: "newword".to_string(),
                        def: "newdef".to_string(),
                        unit: 0,
                        pos: "newpos".to_string(),
                        arrowed_id: None,
                        hqid: 1,
                        seq: 2,
                        arrowed_seq: None,
                        freq: 1,
                        runningcount: 1,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "newword".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 3,
                        word: "δὲ".to_string(),
                        word_type: 0,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 3,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    }
                ]
                .to_vec(),
                selected_id: None,
                error: "".to_string()
            }
        );
    }

    #[actix_rt::test]
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
        //let res = gkv_get_text_words(&db, &info, selected_word_id).await;
        //println!("words: {:?}", res);

        let post = ArrowWordRequest {
            qtype: "arrowWord".to_string(),
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

        // let res = gkv_get_text_words(&db, &info, selected_word_id).await;
        // println!("words: {:?}", res);
    }

    #[actix_rt::test]
    async fn arrow_word2() {
        let (db, user_info) = set_up().await;
        let course_id = 1;

        let res = setup_text_test(&db, course_id, &user_info).await;
        assert!(res.success);

        //insert gloss
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
        //println!("words: {:?}", res);

        let gloss_id: i32 = res
            .as_ref()
            .unwrap()
            .inserted_id
            .unwrap()
            .try_into()
            .unwrap();

        //set_gloss on word
        let post = SetGlossRequest {
            qtype: "set_gloss".to_string(),
            word_id: 17,
            gloss_id,
        };
        let res =
            gkv_update_gloss_id(&db, post.gloss_id, post.word_id, &user_info, course_id).await;
        //println!("arrow: {:?}", res);
        assert_eq!(
            res.unwrap(),
            UpdateGlossIdResponse {
                qtype: "set_gloss".to_string(),
                words: [SmallWord {
                    wordid: 17,
                    hqid: gloss_id,
                    lemma: "newword".to_string(),
                    pos: "newpos".to_string(),
                    def: "newdef".to_string(),
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
            qtype: "set_gloss".to_string(),
            word_id: 20,
            gloss_id,
        };
        let res =
            gkv_update_gloss_id(&db, post.gloss_id, post.word_id, &user_info, course_id).await;
        //println!("arrow: {:?}", res);
        assert_eq!(
            res.unwrap(),
            UpdateGlossIdResponse {
                qtype: "set_gloss".to_string(),
                words: [
                    SmallWord {
                        wordid: 17,
                        hqid: gloss_id,
                        lemma: "newword".to_string(),
                        pos: "newpos".to_string(),
                        def: "newdef".to_string(),
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
                        lemma: "newword".to_string(),
                        pos: "newpos".to_string(),
                        def: "newdef".to_string(),
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
            qtype: "arrowWord".to_string(),
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
        let res = gkv_get_text_words(&db, &info, selected_word_id).await;

        assert_eq!(
            res.unwrap(),
            MiscErrorResponse {
                this_text: 1,
                text_name: "testingtext".to_string(),
                words: [
                    WordRow {
                        wordid: 1,
                        word: "Θύρσις ἢ ᾠδή".to_string(),
                        word_type: 7,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 1,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 2,
                        word: "Θύρσις".to_string(),
                        word_type: 2,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 2,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 3,
                        word: "[line]5".to_string(),
                        word_type: 5,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 3,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 4,
                        word: "αἴκα".to_string(),
                        word_type: 0,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 4,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 5,
                        word: "δ".to_string(),
                        word_type: 0,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 5,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 6,
                        word: "᾽".to_string(),
                        word_type: 1,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 6,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 7,
                        word: "αἶγα".to_string(),
                        word_type: 0,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 7,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 8,
                        word: "λάβῃ".to_string(),
                        word_type: 0,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 8,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 9,
                        word: "τῆνος".to_string(),
                        word_type: 0,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 9,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 10,
                        word: "γέρας".to_string(),
                        word_type: 0,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 10,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 11,
                        word: ",".to_string(),
                        word_type: 1,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 11,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 12,
                        word: "ἐς".to_string(),
                        word_type: 0,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 12,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 13,
                        word: "τὲ".to_string(),
                        word_type: 0,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 13,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 14,
                        word: "καταρρεῖ".to_string(),
                        word_type: 0,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 14,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 15,
                        word: "".to_string(),
                        word_type: 11,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 15,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 16,
                        word: "[line]10".to_string(),
                        word_type: 5,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 16,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 17,
                        word: "ὁσίου".to_string(),
                        word_type: 0,
                        lemma: "newword".to_string(),
                        def: "newdef".to_string(),
                        unit: 0,
                        pos: "newpos".to_string(),
                        arrowed_id: Some(17),
                        hqid: 31,
                        seq: 17,
                        arrowed_seq: Some(17),
                        freq: 2,
                        runningcount: 1,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: Some(1),
                        sort_alpha: "newword".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 18,
                        word: "γὰρ".to_string(),
                        word_type: 0,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 18,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 19,
                        word: "ἀνδρὸς".to_string(),
                        word_type: 0,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 19,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 20,
                        word: "ὅσιος".to_string(),
                        word_type: 0,
                        lemma: "newword".to_string(),
                        def: "newdef".to_string(),
                        unit: 0,
                        pos: "newpos".to_string(),
                        arrowed_id: Some(17),
                        hqid: 31,
                        seq: 20,
                        arrowed_seq: Some(17),
                        freq: 2,
                        runningcount: 2,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: Some(1),
                        sort_alpha: "newword".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 21,
                        word: "ὢν".to_string(),
                        word_type: 0,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 21,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 22,
                        word: "ἐτύγχανον".to_string(),
                        word_type: 0,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 22,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 23,
                        word: "".to_string(),
                        word_type: 10,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 23,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 24,
                        word: "This".to_string(),
                        word_type: 12,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 24,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 25,
                        word: "is".to_string(),
                        word_type: 12,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 25,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 26,
                        word: "a".to_string(),
                        word_type: 12,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 26,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 27,
                        word: "test".to_string(),
                        word_type: 12,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 27,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 28,
                        word: ".".to_string(),
                        word_type: 1,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 28,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 29,
                        word: "".to_string(),
                        word_type: 10,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 29,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 30,
                        word: "γὰρ".to_string(),
                        word_type: 0,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 30,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                ]
                .to_vec(),
                selected_id: None,
                error: "".to_string()
            }
        );

        //add second text
        let res = setup_small_text_test(&db, course_id, &user_info).await;
        assert!(res.success);

        let info = QueryRequest { text: 2, wordid: 0 };
        let selected_word_id = None;
        let res = gkv_get_text_words(&db, &info, selected_word_id).await;
        assert_eq!(
            *res.as_ref().unwrap(),
            MiscErrorResponse {
                this_text: 2,
                text_name: "testingtext2".to_string(),
                words: [
                    WordRow {
                        wordid: 31,
                        word: "ὁσίου".to_string(),
                        word_type: 0,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 1,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 2,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 32,
                        word: "γὰρ".to_string(),
                        word_type: 0,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 2,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 2,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 33,
                        word: "ὅσιος".to_string(),
                        word_type: 0,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 3,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 2,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    }
                ]
                .to_vec(),
                selected_id: None,
                error: "".to_string()
            }
        );
        //println!("res {:?}", res);

        //set_gloss
        let post = SetGlossRequest {
            qtype: "set_gloss".to_string(),
            word_id: 31,
            gloss_id,
        };
        let res =
            gkv_update_gloss_id(&db, post.gloss_id, post.word_id, &user_info, course_id).await;
        //println!("arrow: {:?}", res);
        assert_eq!(
            res.unwrap(),
            UpdateGlossIdResponse {
                qtype: "set_gloss".to_string(),
                words: [
                    SmallWord {
                        wordid: 17,
                        hqid: gloss_id,
                        lemma: "newword".to_string(),
                        pos: "newpos".to_string(),
                        def: "newdef".to_string(),
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
                        lemma: "newword".to_string(),
                        pos: "newpos".to_string(),
                        def: "newdef".to_string(),
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
                        lemma: "newword".to_string(),
                        pos: "newpos".to_string(),
                        def: "newdef".to_string(),
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
            qtype: "set_gloss".to_string(),
            word_id: 33,
            gloss_id,
        };
        let res =
            gkv_update_gloss_id(&db, post.gloss_id, post.word_id, &user_info, course_id).await;
        //println!("arrow: {:?}", res);
        assert_eq!(
            res.unwrap(),
            UpdateGlossIdResponse {
                qtype: "set_gloss".to_string(),
                words: [
                    SmallWord {
                        wordid: 17,
                        hqid: gloss_id,
                        lemma: "newword".to_string(),
                        pos: "newpos".to_string(),
                        def: "newdef".to_string(),
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
                        lemma: "newword".to_string(),
                        pos: "newpos".to_string(),
                        def: "newdef".to_string(),
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
                        lemma: "newword".to_string(),
                        pos: "newpos".to_string(),
                        def: "newdef".to_string(),
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
                        lemma: "newword".to_string(),
                        pos: "newpos".to_string(),
                        def: "newdef".to_string(),
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
        let res = gkv_get_text_words(&db, &info, selected_word_id).await;
        assert_eq!(
            *res.as_ref().unwrap(),
            MiscErrorResponse {
                this_text: 2,
                text_name: "testingtext2".to_string(),
                words: [
                    WordRow {
                        wordid: 31,
                        word: "ὁσίου".to_string(),
                        word_type: 0,
                        lemma: "newword".to_string(),
                        def: "newdef".to_string(),
                        unit: 0,
                        pos: "newpos".to_string(),
                        arrowed_id: Some(17),
                        hqid: 31,
                        seq: 1,
                        arrowed_seq: Some(17),
                        freq: 4,
                        runningcount: 3,
                        is_flagged: false,
                        word_text_seq: 2,
                        arrowed_text_seq: Some(1),
                        sort_alpha: "newword".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 32,
                        word: "γὰρ".to_string(),
                        word_type: 0,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 2,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 2,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 33,
                        word: "ὅσιος".to_string(),
                        word_type: 0,
                        lemma: "newword".to_string(),
                        def: "newdef".to_string(),
                        unit: 0,
                        pos: "newpos".to_string(),
                        arrowed_id: Some(17),
                        hqid: 31,
                        seq: 3,
                        arrowed_seq: Some(17),
                        freq: 4,
                        runningcount: 4,
                        is_flagged: false,
                        word_text_seq: 2,
                        arrowed_text_seq: Some(1),
                        sort_alpha: "newword".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    }
                ]
                .to_vec(),
                selected_id: None,
                error: "".to_string()
            }
        );

        let timestamp = 1667191605; //get_timestamp().try_into().unwrap(),
        let info = WordtreeQueryRequest {
            n: 101,
            idprefix: "text".to_string(),
            x: "0.2813670904164459".to_string(),
            request_time: timestamp,
            page: 0, //can be negative for pages before
            mode: "context".to_string(),
            query: r#"{"lexicon":"hqvocab","mode":"normal","w":""}"#.to_string(), //WordQuery,
            lex: Some("hqvocab".to_string()),
        };

        let res = gkv_get_texts(&db, &info).await;
        assert_eq!(
            *res.as_ref().unwrap(),
            WordtreeQueryResponse {
                select_id: Some(0),
                error: "".to_string(),
                wtprefix: "text".to_string(),
                nocache: 0,
                container: "textContainer".to_string(),
                request_time: 1667191605,
                page: 0,
                last_page: 1,
                lastpage_up: 1,
                scroll: "".to_string(),
                query: "".to_string(),
                arr_options: [
                    AssignmentTree {
                        i: 1,
                        col: ["testingtext".to_string(), "1".to_string()].to_vec(),
                        c: [].to_vec(),
                        h: false
                    },
                    AssignmentTree {
                        i: 2,
                        col: ["testingtext2".to_string(), "2".to_string()].to_vec(),
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
        let res = db::update_text_order_db(&db, course_id, text_id, step).await;
        // match res {
        //     Ok(r) => (),
        //     Err(ref r) => println!("error: {:?}", r),
        // };
        assert!(res.is_ok());

        let info = WordtreeQueryRequest {
            n: 101,
            idprefix: "text".to_string(),
            x: "0.2813670904164459".to_string(),
            request_time: timestamp,
            page: 0, //can be negative for pages before
            mode: "context".to_string(),
            query: r#"{"lexicon":"hqvocab","mode":"normal","w":""}"#.to_string(), //WordQuery,
            lex: Some("hqvocab".to_string()),
        };

        let res = gkv_get_texts(&db, &info).await;
        assert_eq!(
            *res.as_ref().unwrap(),
            WordtreeQueryResponse {
                select_id: Some(0),
                error: "".to_string(),
                wtprefix: "text".to_string(),
                nocache: 0,
                container: "textContainer".to_string(),
                request_time: 1667191605,
                page: 0,
                last_page: 1,
                lastpage_up: 1,
                scroll: "".to_string(),
                query: "".to_string(),
                arr_options: [
                    AssignmentTree {
                        i: 2,
                        col: ["testingtext2".to_string(), "2".to_string()].to_vec(),
                        c: [].to_vec(),
                        h: false
                    },
                    AssignmentTree {
                        i: 1,
                        col: ["testingtext".to_string(), "1".to_string()].to_vec(),
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
        let res = gkv_get_text_words(&db, &info, selected_word_id).await;
        assert_eq!(
            *res.as_ref().unwrap(),
            MiscErrorResponse {
                this_text: 2,
                text_name: "testingtext2".to_string(),
                words: [
                    WordRow {
                        wordid: 31,
                        word: "ὁσίου".to_string(),
                        word_type: 0,
                        lemma: "newword".to_string(),
                        def: "newdef".to_string(),
                        unit: 0,
                        pos: "newpos".to_string(),
                        arrowed_id: Some(17),
                        hqid: 31,
                        seq: 1,
                        arrowed_seq: Some(17),
                        freq: 4,
                        runningcount: 1,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: Some(2),
                        sort_alpha: "newword".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 32,
                        word: "γὰρ".to_string(),
                        word_type: 0,
                        lemma: "".to_string(),
                        def: "".to_string(),
                        unit: 0,
                        pos: "".to_string(),
                        arrowed_id: None,
                        hqid: 0,
                        seq: 2,
                        arrowed_seq: None,
                        freq: 0,
                        runningcount: 0,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: None,
                        sort_alpha: "".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    },
                    WordRow {
                        wordid: 33,
                        word: "ὅσιος".to_string(),
                        word_type: 0,
                        lemma: "newword".to_string(),
                        def: "newdef".to_string(),
                        unit: 0,
                        pos: "newpos".to_string(),
                        arrowed_id: Some(17),
                        hqid: 31,
                        seq: 3,
                        arrowed_seq: Some(17),
                        freq: 4,
                        runningcount: 2,
                        is_flagged: false,
                        word_text_seq: 1,
                        arrowed_text_seq: Some(2),
                        sort_alpha: "newword".to_string(),
                        last_word_of_page: false,
                        app_crit: None
                    }
                ]
                .to_vec(),
                selected_id: None,
                error: "".to_string()
            }
        );
    }

    #[actix_rt::test]
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

        // let res = gkv_get_text_words(&db, &info, selected_word_id).await;
        // println!("words: {:?}", res);

        //set an already existing gloss
        let post = SetGlossRequest {
            qtype: "set_gloss".to_string(),
            word_id: 17,
            gloss_id: 30,
        };
        let res =
            gkv_update_gloss_id(&db, post.gloss_id, post.word_id, &user_info, course_id).await;
        //println!("arrow: {:?}", res);
        assert_eq!(
            res.unwrap(),
            UpdateGlossIdResponse {
                qtype: "set_gloss".to_string(),
                words: [SmallWord {
                    wordid: 17,
                    hqid: 30,
                    lemma: "newword".to_string(),
                    pos: "newpos".to_string(),
                    def: "newdef".to_string(),
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

        // let res = gkv_get_text_words(&db, &info, selected_word_id).await;
        // println!("words: {:?}", res);
    }

    #[actix_rt::test]
    async fn insert_and_update_gloss() {
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
        //println!("words: {:?}", res);

        let gloss_id: i32 = res
            .as_ref()
            .unwrap()
            .inserted_id
            .unwrap()
            .try_into()
            .unwrap();

        assert_eq!(
            *res.as_ref().unwrap(),
            UpdateGlossResponse {
                qtype: "newlemma".to_string(),
                success: true,
                affectedrows: 1,
                inserted_id: Some(1)
            }
        );

        let post = UpdateGlossRequest {
            qtype: "editlemma".to_string(),
            hqid: Some(gloss_id),
            lemma: "newword2".to_string(),
            stripped_lemma: "newword2".to_string(),
            pos: "newpos2".to_string(),
            def: "newdef2".to_string(),
            note: "newnote2".to_string(),
        };
        let res = gkv_update_or_add_gloss(&db, &post, &user_info).await;
        //println!("words: {:?}", res);
        assert_eq!(
            *res.as_ref().unwrap(),
            UpdateGlossResponse {
                qtype: "editlemma".to_string(),
                success: true,
                affectedrows: 1,
                inserted_id: None
            }
        );

        let timestamp = 1667191605; //get_timestamp().try_into().unwrap(),
        let info = WordtreeQueryRequest {
            n: 101,
            idprefix: "updatelog".to_string(),
            x: "0.4828853350220542".to_string(),
            request_time: timestamp,
            page: 0, //can be negative for pages before
            mode: "context".to_string(),
            query: r#"{"lexicon":"hqvocab","mode":"normal","w":""}"#.to_string(), //WordQuery,
            lex: None,
        };

        let res = gkv_update_log(&db, &info).await;
        //just check number of update records to avoid having to match up timestamps
        assert_eq!(res.unwrap().arr_options.len(), 2);
    }
}
