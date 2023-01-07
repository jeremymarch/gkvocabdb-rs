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

use super::*;
use std::collections::HashMap;

use actix_web::http::header::{
    ContentDisposition, DispositionParam, DispositionType,
};

#[derive(Debug, Serialize, Deserialize, Clone,Eq,PartialEq)]
struct Gloss {
    id:u32,
    lemma:String,
    def:String,
    sort_alpha: String,
    arrow: bool
}

pub async fn export_text(
    (info, session, req): (web::Query<ExportRequest>, Session, HttpRequest),
) -> Result<HttpResponse> {
    let db = req.app_data::<SqlitePool>().unwrap();
    let bold_glosses = false;
    let course_id = 1;

    if let Some(_user_id) = login::get_user_id(session) {
        let mut latex:String = include_str!("latex/doc_template.tex")
            .replace("%BOLDLEMMATA%", if bold_glosses { "\\bf" } else { "" });

        //info.text_ids is now a comma separated string of text_ids "133,134,135"
        //let texts:Vec<u32> = vec![133,134,135,136,137];//vec![info.textid];
        let texts: Vec<u32> = info.text_ids
            .split(',')
            .map(|s| s.parse().expect("parse error"))
            .collect();

        for text_id in texts {

            let words:Vec<WordRow> = db::get_words_for_export(db, text_id, course_id).await.map_err(map_sqlx_error)?;

            //divide words into seperate vectors of words per page
            let mut words_divided_by_page:Vec<Vec<WordRow>> = vec![];

            let mut start = 0;
            for (idx, ww,) in words.iter().enumerate() {
                if ww.last_word_of_page {
                    words_divided_by_page.push(words[start..=idx].to_vec());
                    start = idx + 1;
                }
            }
            if start < words.len() - 1 {
                words_divided_by_page.push(words[start..words.len()].to_vec());
            }

            for words_in_page in words_divided_by_page {
                let mut res = String::from(""); //start fresh

                let mut title = String::from("");
                let mut header = String::from("");
                let mut prev_non_space = true;
                let mut last_type = WordType::InvalidType;
                let mut glosses: HashMap<u32, Gloss> = HashMap::new();

                let mut app_crits:Vec<String> = vec![]; //placeholder for now

                for w in words_in_page {
                    
                    let word = w.word.trim().to_string();

                    if let Some(app_crit) = w.app_crit {
                        app_crits.push(app_crit);
                    }

                    match WordType::from_i32(w.word_type.into()) {
                        WordType::WorkTitle => { //7
                            title = word;
                            header = title.clone();
                        },
                        WordType::Speaker => { //2
                            res.push_str(format!("%StartSubTitle%{}%EndSubTitle%", word).as_str()); 
                        },
                        WordType::InlineSpeaker => { //9
                            res.push_str(format!("%StartInnerSubTitle%{}%EndInnerSubTitle%", word).as_str());
                        },
                        WordType::Section => { //4

                            // function fixSubsection($a) {

                            //     $r = str_replace("[section]", "", $a);
                            //     if (preg_match('/(\d+)[.](\d+)/', $r, $matches, PREG_OFFSET_CAPTURE) === 1) {
                            //         if ($matches[2][0] == "1") {
                            //             $r = "%StartSubSection%" . $matches[1][0] . "%EndSubSection%";
                            //         }
                            //         else {
                            //             $r = "%StartSubSubSection%" . $matches[2][0] . "%EndSubSubSection%";
                            //         }		
                            //         return $r;
                            //     }
                            //     else { 
                            //         return "%StartSubSection%" . $r . "%EndSubSection%";
                            //     }
                            // }

                            //fixSubsection(word);
                            res.push_str(format!("%StartSubSection%{}%EndSubSection%", word).as_str());
                            if last_type == WordType::InvalidType || last_type == WordType::ParaWithIndent { //-1 || 6
                                prev_non_space = true;
                            }
                            else {
                                prev_non_space = false;
                            }
                        },
                        WordType::ParaWithIndent => { //6
                            res.push_str("%para%");
                            prev_non_space = true;
                        },
                        WordType::ParaNoIndent => { //10
                            res.push_str("%parnoindent%");
                            prev_non_space = true;
                        },
                        WordType::Word | WordType::Punctuation => { //0 | 1
                            let punc = vec![".", ",", "·", "·", ";", ";", ">", "]" ,")", ",\"", "·\"", "·\"", ".’"];

                            res.push_str(format!("{}{}", if punc.contains(&word.as_str()) || prev_non_space { "" } else { " " }, word).as_str()); // (( punc.contains(word) || prev_non_space ) ? "" : " ") . $word;
                            
                            if word == "<" || word == "[" || word == "(" {
                                prev_non_space = true;
                            }
                            else {
                                prev_non_space = false;
                            }
                        },
                        WordType::VerseLine => { //5
                            //need to skip "[line]" prefix
                            res.push_str(format!("%VERSELINESTART%{}%VERSELINEEND%", word).as_str());
                            prev_non_space = true;
                        }
                        _ => (),
                    }
                    //last_seq = w.seq as i64;
                    //last_word_id = w.wordid as i64;

                    if !w.lemma.is_empty() && !w.def.is_empty() && !glosses.contains_key(&w.hqid) {
        
                        // if (!is_null($row["arrowedSeq"]) && (int)$row["seq"] > (int)$row["arrowedSeq"]) {
                        //     //echo $row["seq"] . ", " . $row["arrowedSeq"] . "\n";
                        //     continue; //skip
                        // }
                        // else if ((int)$row["seq"] == (int)$row["arrowedSeq"]) {
                        //     $g->arrow = TRUE;
                        //     $arrowedIndex[] = array($row["lemma"], $row["sortalpha"], $currentPageNum);
                        // }
                        // else {
                        //     $g->arrow = FALSE;
                        // }

                        let is_arrowed;
                        if w.arrowed_text_seq == Some(w.word_text_seq) && w.arrowed_seq == Some(w.seq) {
                            
                            is_arrowed = true; //this instance is arrowed
                        }
                        else if (w.arrowed_text_seq.is_some() && w.arrowed_text_seq < Some(w.word_text_seq)) || 
                            (w.arrowed_text_seq.is_some() && w.arrowed_text_seq == Some(w.word_text_seq) && w.arrowed_seq.is_some() && 
                            w.arrowed_seq < Some(w.seq)) {
                                //word was already arrowed, so hide it here
                                continue;
                        }
                        else {
                            //show word, not yet arrowed
                            is_arrowed = false;
                        }

                        let gloss = Gloss {
                            id:w.hqid,
                            lemma:w.lemma,
                            def:w.def,
                            sort_alpha:w.sort_alpha,
                            arrow:is_arrowed,
                        };
                        glosses.insert(w.hqid, gloss);
                    }
                    
                    last_type = WordType::from_i32(w.word_type.into());
                }
                let mut sorted_glosses:Vec<Gloss> = glosses.values().cloned().collect();
                sorted_glosses.sort_by(|a, b| a.sort_alpha.to_lowercase().cmp(&b.sort_alpha.to_lowercase()));

                latex = apply_latex_templates(&mut latex, &title, &mut res, &sorted_glosses, &header, &app_crits);
            }
        }
        latex.push_str("\\end{document}\n");

        let filename = "glosser_export.tex";
        let cd_header = ContentDisposition {
            disposition: DispositionType::Attachment,
            parameters: vec![
                DispositionParam::Filename(String::from(filename)),
            ],
        };
        Ok(HttpResponse::Ok()
            .content_type("application/x-latex")
            .insert_header(cd_header)
            .body(latex))
    } else {
        let res = ImportResponse {
            success: false,
            words_inserted: 0,
            error: "Export failed: not logged in".to_string(),
        };
        Ok(HttpResponse::Ok().json(res))
    }
}

fn apply_latex_templates(latex:&mut String, title:&str, text:&mut String, glosses:&Vec<Gloss>, header:&str, app_crits:&Vec<String>) -> String {
    
    latex.push_str("\\newpage\n");

    if !header.is_empty() {
        if !title.is_empty() {
            latex.push_str("\\fancyhead[ER]{ }\n");
        }
        else {
            latex.push_str(format!("\\fancyhead[ER]{{{}}}\n", header).as_str());
        }
    }
    if !title.is_empty() {
        latex.push_str(format!("\\begin{{center}}\\noindent\\textbf{{{}}}\\par\\end{{center}}\n", title).as_str());
    }

    if text.contains("%VERSELINESTART%") {
        *text = text.replace("%StartSubTitle%", "");
        *text = text.replace("%EndSubTitle%", " & "); //add this even if no %StartSubTitle%
        *text = text.replace("%LINEEND%", " \\\\ \n");
        *text = text.replace("%VERSELINESTART%", " & ");
        *text = text.replace("%VERSELINEEND%", " \\\\ \n");
        latex.push_str(format!("{}{}\\end{{tabular}}\n", include_str!("latex/verse_table_start.tex"), text).as_str());
    }
    else {

        latex.push_str("\\begin{spacing}{\\GlossLineSpacing}\n");
        latex.push_str("\\noindent\n");

        *text = text.replace("%StartSubTitle%", "\\begin{center}"); //\hspace{0pt} solves problem when \par\marginsec{} come together
        *text = text.replace("%EndSubTitle%", "\\end{center}");

        *text = text.replace("%StartSubSection%", "\\hspace{0pt}\\marginsec{"); //\hspace{0pt} solves problem when \par\marginsec{} come together
        *text = text.replace("%EndSubSection%", "}");
        *text = text.replace("%StartSubSubSection%", "\\hspace{0pt}\\marginseclight{"); //\hspace{0pt} solves problem when \par\marginsec{} come together
        *text = text.replace("%EndSubSubSection%", "}");
        *text = text.replace("%para%", "\n\\par\n");
        //\hspace*{\fill}: https://tex.stackexchange.com/questions/54040/underful-hbox-badness-10000
        latex.push_str(format!("{}\\hspace*{{\\fill}}\n\\end{{spacing}}\n", text).as_str());
    }

    
    //insert apparatus criticus here appcrit
    if !app_crits.is_empty() {
        latex.push_str("~\\\\\n");
    }
    for a in app_crits {
        latex.push_str(format!("{}\\\\\n", a).as_str());
    }

    latex.push_str("\\begin{table}[b!]\\leftskip -0.84cm\n");
    latex.push_str("\\begin{tabular}{ m{0.2cm} L{3.25in} D{3.1in} }\n");
    for g in glosses {
        let arrow = if g.arrow { "\\textbf{→}" } else { "" };
        let mut lemma = g.lemma.replace('\\', "\\textbackslash");
        lemma = lemma.replace('&', "\\&");
        
        lemma = lemma.replace("<i>", "\\textit{");
        lemma = lemma.replace("</i>", "}");
        let mut def = g.def.replace('\\', "\\textbackslash");
        def = def.replace('&', "\\&");
        
        def = def.replace("<i>", "\\textit{");
        def = def.replace("</i>", "}");

        latex.push_str(format!("{} & {} & {} \\\\\n", arrow, lemma, def).as_str());
        //$arrow . ' & ' . $lemma . ' & ' . $def . ' \\\\' . "\n";
    }
    latex.push_str("\\end{tabular}\n");
    latex.push_str("\\end{table}\n");

    *latex = latex.replace(r"& Α´", "Α´");//hack to fix two type 2s together, lines 866
    *latex = latex.replace(r"& Β´", "Β´");//hack to fix two type 2s together, lines 872

    latex.to_string()
}
