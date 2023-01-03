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
        let template = include_str!("latex/doc_template.tex");
        let mut res = template.replace("%BOLDLEMMATA%", if bold_glosses { "\\bf" } else { "" });

        let text_id = info.textid;

        let words = get_words_for_export(db, text_id, course_id).await.map_err(map_sqlx_error)?;

        let mut title = String::from("");
        let mut header = String::from("");
        let mut prev_non_space = true;
        let mut last_type = WordType::InvalidType;
        //let verse_line_start = false;
        //let mut last_word_id:i64 = -1;
        //let mut last_seq:i64 = -1;
        let mut glosses: HashMap<u32, Gloss> = HashMap::new();

        for w in words {
            
            let word = w.word.trim().to_string();

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
                    res.push_str(&word); //fixSubsection(word);
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

                let is_arrowed;
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
        sorted_glosses.sort_by(|a, b| a.sort_alpha.cmp(&b.sort_alpha));

        let res = mapply_templates(&title, &mut res, &sorted_glosses, &header);

        Ok(HttpResponse::Ok()
            .content_type("application/x-latex")
            .body(res))
    } else {
        let res = ImportResponse {
            success: false,
            words_inserted: 0,
            error: "Export failed: not logged in".to_string(),
        };
        Ok(HttpResponse::Ok().json(res))
    }
}

fn mapply_templates(title:&str, text:&mut String, glosses:&Vec<Gloss>, header:&str) -> String {
    let mut latex = String::from("");
    
    latex.push_str(r"\newpage\n");

    if !header.is_empty() {
        if !title.is_empty() {
            latex.push_str(r"\fancyhead[ER]{ }\n");
        }
        else {
            latex.push_str(format!(r"\fancyhead[ER]{{{}}}\n", header).as_str());
        }
    }
    if !title.is_empty() {
        latex.push_str(format!(r"\begin{{center}}\noindent\textbf{{{}}}\par\end{{center}}\n", title).as_str());
    }

    if text.contains(r"%VERSELINESTART%") {
        *text = text.replace(r"%StartSubTitle%", "");
        *text = text.replace(r"%EndSubTitle%", " & "); //add this even if no %StartSubTitle%
        *text = text.replace(r"%LINEEND%", " \\\\ \n");
        *text = text.replace(r"%VERSELINESTART%", " & ");
        *text = text.replace(r"%VERSELINEEND%", " \\\\ \n");
        latex.push_str(format!(r"{}{}\end{{tabular}}\n", include_str!("latex/verse_table_start.tex"), text).as_str());
    }
    else {

        latex.push_str(r"\begin{spacing}{\GlossLineSpacing}\n");
        latex.push_str(r"\noindent\n");

        *text = text.replace(r"%StartSubTitle%", r"\begin{center}"); //\hspace{0pt} solves problem when \par\marginsec{} come together
        *text = text.replace(r"%EndSubTitle%", r"\end{center}");

        *text = text.replace(r"%StartSubSection%", r"\hspace{0pt}\marginsec{"); //\hspace{0pt} solves problem when \par\marginsec{} come together
        *text = text.replace(r"%EndSubSection%", "}");
        *text = text.replace(r"%StartSubSubSection%", r"\hspace{0pt}\marginseclight{"); //\hspace{0pt} solves problem when \par\marginsec{} come together
        *text = text.replace(r"%EndSubSubSection%", "}");
        *text = text.replace(r"%para%", "\n\\par\n");
        //\hspace*{\fill}: https://tex.stackexchange.com/questions/54040/underful-hbox-badness-10000
        latex.push_str(format!(r"{}\hspace*{{\fill}}\n\end{{spacing}}\n", text).as_str());
    }

    
    //appCrits here
    // if ( count($appCrits) > 0) {
    //     $latex .=  '~\\\\' . "\n";
    // }
    // for ($i = 0; $i < count($appCrits); $i++) {
    //     $latex .= $appCrits[$i] . '\\\\' . "\n";
    // }
    

    latex.push_str(r"\begin{table}[b!]\leftskip -0.84cm\n");
    latex.push_str(r"\begin{tabular}{ m{0.2cm} L{3.25in} D{3.1in} }\n");
    for g in glosses {
        let arrow = if g.arrow { "\textbf{→}" } else { "" };
        let mut lemma = g.lemma.replace('\\', "\\textbackslash");
        lemma = lemma.replace('&', r"\&");
        
        lemma = lemma.replace("<i>", "\textit{");
        lemma = lemma.replace("</i>", "}");
        let mut def = g.def.replace('\\', "\\textbackslash");
        def = def.replace('&', r"\&");
        
        def = def.replace("<i>", "\textit{");
        def = def.replace("</i>", "}");

        latex.push_str(format!("{} & {} & {} \\\\\n", arrow, lemma, def).as_str());
        //$arrow . ' & ' . $lemma . ' & ' . $def . ' \\\\' . "\n";
    }
    latex.push_str(r"\end{tabular}\n");
    latex.push_str(r"\end{table}\n");

    latex = latex.replace(r"& Α´", "Α´");//hack to fix two type 2s together, lines 866
    latex = latex.replace(r"& Β´", "Β´");//hack to fix two type 2s together, lines 872

    latex
}
