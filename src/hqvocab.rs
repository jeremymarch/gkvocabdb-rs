use super::*;

#[derive(Deserialize)]
pub struct HQVocabRequest {
    pub sort: Option<String>,
    pub unit: Option<u32>,
}

pub async fn hqvocab((info, req): (web::Query<HQVocabRequest>, HttpRequest)) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<SqlitePool>().unwrap();
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

    let unit = info.unit.unwrap_or(1);
    let sort = info.sort.clone().unwrap_or_else(|| "unit".to_string());
    let u = if unit > 20 { 20 } else { unit };
    
    for p in ["noun", "verb", "adjective", "other"] {
        let mut res = String::from("");
        let mut last_unit = 0;
        
        let hqv = get_hqvocab_column(db, p, u, &sort)
            .await
            .map_err(map_sqlx_error)?;
        for w in hqv {
            if sort != "alpha" && last_unit != w.1 {
                res.push_str(format!("<div class='rowdiv'><p class='rowp'>Unit: {}</p></div>", w.1).as_str());
                last_unit = w.1;
            }
            let u = format!(" <span class='unitNum'>({})</span>", w.1);
            res.push_str(format!("<p class='row tooltip'>{}{}<span class='tooltiptext'>{}</span></p>", w.0, if sort == "alpha" { u } else {"".to_string()}, w.2).as_str());
        }

        template = template.replacen(format!("%{}%",p).as_str(), &res, 1);
    }

    template = template.replacen("%%unit%%", &u.to_string(), 1);
    if sort != "alpha" {
        template = template.replacen("%sortalpha%", "", 1);
        template = template.replacen("%sortunit%", "checked", 1);
    }
    else {
        template = template.replacen("%sortalpha%", "checked", 1);
        template = template.replacen("%sortunit%", "", 1);
    }

    Ok(HttpResponse::Ok()
            .content_type("text/html")
            .body(template))
}