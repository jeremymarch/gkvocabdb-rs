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
use crate::map_glosser_error;
use actix_web::web;
use actix_web::Error as AWError;
use actix_web::HttpRequest;
use actix_web::HttpResponse;
use gkvocabdb::dbsqlite::GlosserDbSqlite;
use gkvocabdb::GlosserDb;
use regex::Regex;
use serde::Deserialize;

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

    let mut lower_str = match &info.lower {
        Some(s) => s.trim().to_lowercase(),
        None => String::from("1"),
    };

    let mut upper_str = match &info.upper {
        Some(s) => s.trim().to_lowercase(),
        None => String::from("20"),
    };

    let unit_min = 1;
    let mut unit_max = 20;

    let lower_re = Regex::new("^([im])([1-9])$").unwrap();
    let lower_matches = lower_re.captures(&lower_str);
    if let Some(lower_matches) = lower_matches {
        let text = lower_matches.get(1).unwrap().as_str();
        let num = lower_matches.get(2).unwrap().as_str();

        if text == "i" {
            lower_str = format!("3{}", num);
        } else {
            lower_str = format!("4{}", num);
        }
    }

    let upper_re = Regex::new("^([im])([1-9])$").unwrap();
    let upper_matches = upper_re.captures(&upper_str);
    if let Some(upper_matches) = upper_matches {
        let text = upper_matches.get(1).unwrap().as_str();
        let num = upper_matches.get(2).unwrap().as_str();

        if text == "i" {
            upper_str = format!("3{}", num);
        } else {
            upper_str = format!("4{}", num);
        }
        unit_max = 49;
    }

    let mut lower: u32 = lower_str.trim().parse::<u32>().unwrap_or(unit_min);
    let mut upper: u32 = upper_str.trim().parse::<u32>().unwrap_or(unit_max);

    lower = lower.clamp(unit_min, unit_max);
    upper = upper.clamp(unit_min, unit_max);

    if lower > upper {
        upper = lower;
    }

    // let lower_display = if lower_matches.is_some() {
    //     lower_str
    // } else {
    //     lower.to_string()
    // };
    // let upper_display = if upper_matches.is_some() {
    //     upper_str
    // } else {
    //     upper.to_string()
    // };

    let sort = info.sort.clone().unwrap_or_else(|| String::from("unit"));
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
                let unit_title = match w.1 {
                    1..=20 => format!("Unit: {}", w.1),
                    31..=39 => format!("Ion {}", w.1 - 30),
                    41..=49 => format!("Medea {}", w.1 - 40),
                    _ => String::from("?"),
                };
                res.push_str(
                    format!(
                        "<div class='rowdiv'><p class='rowp'>{}</p></div>",
                        unit_title
                    )
                    .as_str(),
                );
                last_unit = w.1;
            }
            let alpha_unit_title = match w.1 {
                1..=20 => format!(" <span class='unitNum'>({})</span>", w.1),
                31..=39 => format!(" <span class='unitNum'>(i{})</span>", w.1 - 30),
                41..=49 => format!(" <span class='unitNum'>(m{})</span>", w.1 - 40),
                _ => String::from("?"),
            };
            res.push_str(
                format!(
                    "<p class='row tooltip'>{}{}<span class='tooltiptext'>{}</span></p>",
                    if info.abbrev.is_some() {
                        if w.0.starts_with("ἐκ, ") {
                            w.0.to_string()
                        } else if w.0.starts_with("—, ἐ") {
                            String::from("—, ἐρήσομαι")
                        } else if w.0.starts_with("—, ἀν") {
                            String::from("—, ἀνερήσομαι")
                        } else if w.0.starts_with("—, ἀλ") {
                            String::from("—, ἀλλήλων")
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
                    if sort == "alpha" {
                        alpha_unit_title
                    } else {
                        String::from("")
                    },
                    w.2
                )
                .as_str(),
            );
        }

        template = template.replacen(format!("%{}%", p).as_str(), &res, 1);
    }
    tx.commit_tx().await.map_err(map_glosser_error)?;
    let upper_display = if upper > 20 {
        info.upper.as_ref().unwrap()
    } else {
        &upper.to_string()
    };
    template = template.replacen("%%upper%%", upper_display.trim(), 1);
    let lower_display = if lower > 20 {
        info.lower.as_ref().unwrap()
    } else {
        &lower.to_string()
    };
    template = template.replacen("%%lower%%", lower_display.trim(), 1);

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
