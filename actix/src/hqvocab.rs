use super::*;

#[derive(Deserialize)]
pub struct HQVocabRequest {
    pub sort: Option<String>,
    pub lower: Option<String>,
    pub upper: Option<String>,
    pub abbrev: Option<String>,
}

pub async fn hqvocab(
    (info, req): (web::Query<HQVocabRequest>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<GlosserDbSqlite>().unwrap();
    let mut template = include_str!("hqvocab.html").to_string();

    // let mut rows = String::from("");
    // let mut count = 0;
    // let rowlabels = vec!["Present Indicative", "Future Indicative", "Imperfect Indicative", "Aorist Indicative", "Perfect Indicative", "Pluperfect Indicative", "Present Subjunctive", "Aorist Subjunctive", "Present Optative", "Future Optative", "Aorist Optative","Present Imperative", "Aorist Imperative", "Present Infinitive", "Future Infinitive", "Aorist Infinitive", "Perfect Infinitive", "Present Participle", "Future Participle", "Aorist Participle", "Perfect Participle"];
    // let voices = vec!["Active", "Middle", "Passive"];
    // for l in rowlabels {
    //     rows.push_str(format!(r#"<tr class="{}"><td>{}</td>"#, l.to_lowercase(), l).as_str());
    //     for v in &voices {
    //         rows.push_str(format!(
    //         r#"<td class="formcell {}">
    //             <div class="formcellInner">
    //             <input type="text" id="gkform{}" class="gkinput formcellinput" spellcheck="false" autocapitalize="off" autocomplete="off"/>
    //             </div>
    //         </td>"#,
    //         v.to_lowercase(), count).as_str());
    //         count += 1;
    //     }
    //     rows.push_str("</tr>");
    // }

    let lower_str = match &info.lower {
        Some(s) => s.clone(),
        None => "1".to_string(),
    };

    let upper_str = match &info.upper {
        Some(s) => s.clone(),
        None => "20".to_string(),
    };

    let mut lower: u32 = lower_str.trim().parse::<u32>().unwrap_or(1);
    let mut upper: u32 = upper_str.trim().parse::<u32>().unwrap_or(20);

    if lower < 1 {
        lower = 1;
    }
    if lower > 20 {
        lower = 20;
    }
    if upper < 1 {
        upper = 1;
    }
    if upper > 20 {
        upper = 20;
    }
    if lower > upper {
        upper = lower;
    }

    let sort = info.sort.clone().unwrap_or_else(|| "unit".to_string());
    let mut tx = db.begin_tx().await.map_err(map_glosser_error)?;
    for p in ["noun", "verb", "adjective", "other"] {
        let mut res = String::from("");
        let mut last_unit = 0;

        let hqv = tx
            .get_hqvocab_column(p, lower, upper, &sort)
            .await
            .map_err(map_glosser_error)?;
        for w in hqv {
            if sort != "alpha" && last_unit != w.1 {
                res.push_str(
                    format!(
                        "<div class='rowdiv'><p class='rowp'>Unit: {}</p></div>",
                        w.1
                    )
                    .as_str(),
                );
                last_unit = w.1;
            }
            let u = format!(" <span class='unitNum'>({})</span>", w.1);
            res.push_str(
                format!(
                    "<p class='row tooltip'>{}{}<span class='tooltiptext'>{}</span></p>",
                    if info.abbrev.is_some() {
                        if w.0.starts_with("ἐκ, ") {
                            w.0.to_string()
                        } else if w.0.starts_with("—, ἐ") {
                            "—, ἐρήσομαι".to_string()
                        } else if w.0.starts_with("—, ἀν") {
                            "—, ἀνερήσομαι".to_string()
                        } else if w.0.starts_with("—, ἀλ") {
                            "—, ἀλλήλων".to_string()
                        } else {
                            w.0.chars().take_while(|&ch| ch != ',').collect::<String>()
                            //w.0.split(',').next().unwrap().to_string()
                        }
                        // }
                        // else {
                        //     w.0.chars()
                        //     .take_while(|&ch| ch != ',')
                        //     .collect::<String>()
                        // }
                    } else {
                        w.0
                    },
                    if sort == "alpha" { u } else { "".to_string() },
                    w.2
                )
                .as_str(),
            );
        }

        template = template.replacen(format!("%{}%", p).as_str(), &res, 1);
    }
    tx.commit_tx().await.map_err(map_glosser_error)?;

    template = template.replacen("%%upper%%", &upper.to_string(), 1);
    template = template.replacen("%%lower%%", &lower.to_string(), 1);

    if info.abbrev.is_some() {
        template = template.replacen("%abbreviated%", "checked", 1);
    } else {
        template = template.replacen("%abbreviated%", "", 1);
    }

    if sort != "alpha" {
        template = template.replacen("%sortalpha%", "", 1);
        template = template.replacen("%sortunit%", "checked", 1);
    } else {
        template = template.replacen("%sortalpha%", "checked", 1);
        template = template.replacen("%sortunit%", "", 1);
    }

    Ok(HttpResponse::Ok().content_type("text/html").body(template))
}
