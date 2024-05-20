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

use crate::GlosserDb;
use crate::GlosserError;
use crate::WordRow;
use crate::WordType;
use regex::Regex;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
struct Gloss {
    id: u32,
    lemma: String,
    def: String,
    sort_alpha: String,
    arrow: bool,
}

struct ArrowedWordsIndex {
    gloss_lemma: String,
    gloss_sort: String,
    page_number: usize,
}

pub async fn gkv_export_texts_as_latex(
    db: &dyn GlosserDb,
    text_ids: &str,
    course_id: u32,
    bold_glosses: bool,
) -> Result<String, GlosserError> {
    let first_page_number = 58; //should be an even number, else headers will be reversed and we want page 1 to be a right hand page
    let even_page_header = "LGI - UPPER LEVEL GREEK";
    let build_index = true; // whether to build the index
    let index_page_offset = first_page_number; // which page the text starts on

    let mut latex: String = include_str!("latex/doc_template.tex")
        .replace("%BOLDLEMMATA%", if bold_glosses { "\\bf" } else { "" })
        .replace("%EVEN_PAGE_HEADER%", even_page_header)
        .replace("%FIRST_PAGE_NUMBER%", &first_page_number.to_string());

    let texts: Vec<u32> = text_ids
        .split(',')
        .map(|s| s.parse().expect("parse error"))
        .collect();

    let mut tx = db.begin_tx().await?;
    let mut header = tx.get_text_title(*texts.first().unwrap()).await?; //this is overwritten for now by the title
    let mut words: Vec<WordRow> = vec![];
    for text_id in texts {
        words.append(&mut tx.get_words_for_export(text_id, course_id).await?);
    }
    tx.commit_tx().await?;

    //divide words into seperate vectors of words per page
    let mut words_divided_by_page: Vec<Vec<WordRow>> = vec![];

    //to build index
    let mut arrowed_words_index: Vec<ArrowedWordsIndex> = vec![];

    let mut start = 0;
    let mut page_num = 1;
    let mut at_least_one_word = false;
    for (idx, ww) in words.iter().enumerate() {
        if WordType::from_i32(ww.word_type.into()) == WordType::WorkTitle {
            words_divided_by_page.push(vec![]);
            page_num += 1;
            at_least_one_word = false;
            //println!("title1 {}", page_num);
            if page_num % 2 != 0 {
                words_divided_by_page.push(vec![]); //add blank page before work title page, if page is odd (note idx is 0 indexed; page numbers are 1 indexed)
                page_num += 1;
                at_least_one_word = false;
                //println!("title2 {}", page_num);
            }
        }
        if WordType::from_i32(ww.word_type.into()) == WordType::Word {
            at_least_one_word = true;
        }
        if ww.last_word_of_page && at_least_one_word {
            words_divided_by_page.push(words[start..=idx].to_vec());
            start = idx + 1;
            page_num += 1;
            at_least_one_word = false;
        }
    }
    //println!("final {}", page_num);

    if start < words.len() - 1 {
        words_divided_by_page.push(words[start..words.len()].to_vec());
    }
    words_divided_by_page.push(vec![]); //blank page before index
    page_num += 1;
    if page_num % 2 != 0 {
        words_divided_by_page.push(vec![]); //extra blank page if on left page
    }

    for (page_idx, words_in_page) in words_divided_by_page.into_iter().enumerate() {
        let mut res = String::from(""); //start fresh

        let mut title = String::from("");
        //let mut header = String::from("ΥΠΕΡ ΤΟΥ ΕΡΑΤΟΣΘΕΝΟΥΣ ΦΟΝΟΥ ΑΠΟΛΟΓΙΑ");
        let mut prev_non_space = true;
        let mut last_type = WordType::InvalidType;
        let mut glosses: HashMap<u32, Gloss> = HashMap::new();

        let mut app_crits: Vec<String> = vec![]; //placeholder for now

        let mut verse_text = String::from("");
        let mut verse_line: String = String::from("reset"); //set to 0 when not in a verse section
        let mut verse_inline_speaker: String = String::from("");

        for w in words_in_page {
            // if w.wordid > 69704 && w.wordid < 69728 {
            //     println!("word2 {} {}", w.wordid, w.word);
            // }
            let word = w
                .word
                .trim()
                .replace('\u{1F71}', "\u{03AC}") //acute -> tonos, etc...
                .replace('\u{1FBB}', "\u{0386}")
                .replace('\u{1F73}', "\u{03AD}")
                .replace('\u{1FC9}', "\u{0388}")
                .replace('\u{1F75}', "\u{03AE}")
                .replace('\u{1FCB}', "\u{0389}")
                .replace('\u{1F77}', "\u{03AF}")
                .replace('\u{1FDB}', "\u{038A}")
                .replace('\u{1F79}', "\u{03CC}")
                .replace('\u{1FF9}', "\u{038C}")
                .replace('\u{1F7B}', "\u{03CD}")
                .replace('\u{1FEB}', "\u{038E}")
                .replace('\u{1F7D}', "\u{03CE}")
                .replace('\u{1FFB}', "\u{038F}")
                .replace('\u{1FD3}', "\u{0390}") //iota + diaeresis + acute
                .replace('\u{1FE3}', "\u{03B0}") //upsilon + diaeresis + acute
                .replace('\u{037E}', "\u{003B}") //semicolon
                .replace('\u{0387}', "\u{00B7}") //middle dot
                .replace('\u{0344}', "\u{0308}\u{0301}") //combining diaeresis with acute
                .replace('\u{02B9}', r#"^{\prime}"#); //MODIFIER LETTER PRIME

            //assert!(!word.contains('\u{1F79}'));

            if let Some(app_crit) = w.app_crit {
                app_crits.push(app_crit);
            }

            match WordType::from_i32(w.word_type.into()) {
                WordType::WorkTitle => {
                    //7
                    title.clone_from(&word);
                    header.clone_from(&title);
                    verse_line = String::from("reset");
                }
                WordType::Speaker => {
                    //2
                    //including lines 91, 134, 201, 719, 864, 872, , 974, 1047, 1226, 1318

                    match verse_line.as_str() {
                        "reset" => (),
                        "" => (),
                        _ => res.push_str(
                            format!(
                                "{}%VERSEREALLINESTART%{}%VERSELINESTART%{}%VERSELINEEND%",
                                verse_inline_speaker,
                                verse_text,
                                format_verse_line(&verse_line)
                            )
                            .as_str(),
                        ),
                    }
                    verse_line = String::from("reset");

                    verse_text = String::from("");
                    verse_inline_speaker = String::from("");

                    // res.push_str(
                    //     format!(
                    //         "%VERSELINESTART%{}%VERSELINEEND%",
                    //         verse_line
                    //     )
                    //     .as_str(),
                    // );
                    prev_non_space = true;
                    res.push_str(format!("%StartSubTitle%{}%EndSubTitle%", word).as_str());
                }
                WordType::InlineSpeaker => {
                    //9
                    res.push_str(
                        format!("%StartInnerSubTitle%{}%EndInnerSubTitle%", word).as_str(),
                    );
                    verse_line = String::from("reset");
                }
                WordType::InlineVerseSpeaker => {
                    //all but lines 91, 134, 201, 719, 864, 872, , 974, 1047, 1226, 1318
                    //14
                    //res.push_str(format!("%StartInlineVerseSpeaker%{}%EndInlineVerseSpeaker%", word).as_str());
                    verse_inline_speaker.clone_from(&word);
                    //verse_line = String::from("reset");
                }
                WordType::Section => {
                    //4
                    let w = fix_subsection(&word);
                    res.push_str(&w);
                    if last_type == WordType::InvalidType || last_type == WordType::ParaWithIndent {
                        //-1 || 6
                        prev_non_space = true;
                    } else {
                        prev_non_space = false;
                    }
                    verse_line = String::from("reset");
                }
                WordType::ParaWithIndent => {
                    //6
                    res.push_str("%para%");
                    prev_non_space = true;
                    verse_line = String::from("reset");
                }
                WordType::ParaNoIndent => {
                    //10
                    res.push_str("%parnoindent%");
                    prev_non_space = true;
                    verse_line = String::from("reset");
                }
                WordType::Word | WordType::Punctuation => {
                    //0 | 1
                    let punc = vec![
                        ".", ",", "·", "·", ";", ";", ">", "]", ")", ",\"", "·\"", "·\"", ".’",
                    ];

                    let ww = format!(
                        "{}{}",
                        if punc.contains(&word.as_str()) || prev_non_space {
                            ""
                        } else {
                            " "
                        },
                        word
                    );
                    if verse_line == "reset" {
                        res.push_str(ww.as_str()); // (( punc.contains(word) || prev_non_space ) ? "" : " ") . $word;
                    } else {
                        verse_text.push_str(ww.as_str());
                    }
                    prev_non_space = word == "<" || word == "[" || word == "(";
                }
                WordType::VerseLine => {
                    //5
                    //need to skip "[line]" prefix
                    match verse_line.as_str() {
                        "reset" => (),
                        "" => (),
                        _ => res.push_str(
                            format!(
                                "{}%VERSEREALLINESTART%{}%VERSELINESTART%{}%VERSELINEEND%",
                                verse_inline_speaker,
                                verse_text,
                                format_verse_line(&verse_line)
                            )
                            .as_str(),
                        ),
                    }
                    verse_line.clone_from(&word);

                    verse_text = String::from("");
                    verse_inline_speaker = String::from("");

                    // res.push_str(
                    //     format!(
                    //         "%VERSELINESTART%{}%VERSELINEEND%",
                    //         verse_line
                    //     )
                    //     .as_str(),
                    // );
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
                                       //$arrowedIndex[] = array($row["lemma"], $row["sortalpha"], $currentPageNum);
                    if build_index {
                        arrowed_words_index.push(ArrowedWordsIndex {
                            gloss_lemma: w.lemma.to_owned(),
                            gloss_sort: w.sort_alpha.to_owned(),
                            page_number: page_idx + index_page_offset,
                        });
                    }
                } else if (w.arrowed_text_seq.is_some()
                    && w.arrowed_text_seq < Some(w.word_text_seq))
                    || (w.arrowed_text_seq.is_some()
                        && w.arrowed_text_seq == Some(w.word_text_seq)
                        && w.arrowed_seq.is_some()
                        && w.arrowed_seq < Some(w.seq))
                {
                    //word was already arrowed, so hide it here
                    continue;
                } else {
                    //show word, not yet arrowed
                    is_arrowed = false;
                }

                let gloss = Gloss {
                    id: w.hqid,
                    lemma: w
                        .lemma
                        .replace('\u{1F71}', "\u{03AC}") //acute -> tonos, etc...
                        .replace('\u{1FBB}', "\u{0386}")
                        .replace('\u{1F73}', "\u{03AD}")
                        .replace('\u{1FC9}', "\u{0388}")
                        .replace('\u{1F75}', "\u{03AE}")
                        .replace('\u{1FCB}', "\u{0389}")
                        .replace('\u{1F77}', "\u{03AF}")
                        .replace('\u{1FDB}', "\u{038A}")
                        .replace('\u{1F79}', "\u{03CC}")
                        .replace('\u{1FF9}', "\u{038C}")
                        .replace('\u{1F7B}', "\u{03CD}")
                        .replace('\u{1FEB}', "\u{038E}")
                        .replace('\u{1F7D}', "\u{03CE}")
                        .replace('\u{1FFB}', "\u{038F}")
                        .replace('\u{1FD3}', "\u{0390}") //iota + diaeresis + acute
                        .replace('\u{1FE3}', "\u{03B0}") //upsilon + diaeresis + acute
                        .replace('\u{037E}', "\u{003B}") //semicolon
                        .replace('\u{0387}', "\u{00B7}") //middle dot
                        .replace('\u{0344}', "\u{0308}\u{0301}"), //combining diaeresis with acute,
                    def: w.def,
                    sort_alpha: w.sort_alpha,
                    arrow: is_arrowed,
                };
                glosses.insert(w.hqid, gloss);
            }

            last_type = WordType::from_i32(w.word_type.into());
        }

        //finish last verse line if needed
        if verse_line.as_str() != "reset" && verse_line.as_str() != "" {
            res.push_str(
                format!(
                    "{}%VERSEREALLINESTART%{}%VERSELINESTART%{}%VERSELINEEND%",
                    verse_inline_speaker,
                    verse_text,
                    format_verse_line(&verse_line)
                )
                .as_str(),
            );
            // verse_line = String::from("");
            // verse_text = String::from("");
            // verse_inline_speaker = String::from("");
        }

        let mut sorted_glosses: Vec<Gloss> = glosses.values().cloned().collect();
        sorted_glosses.sort_by(|a, b| {
            a.sort_alpha
                .to_lowercase()
                .cmp(&b.sort_alpha.to_lowercase())
        });

        latex = apply_latex_templates(
            &mut latex,
            &title,
            &mut res,
            &sorted_glosses,
            &header,
            &app_crits,
        );
    }

    if build_index && !arrowed_words_index.is_empty() {
        // global $INDEXtemplate;
        // global $startIndexTable;
        // $latex = $INDEXtemplate . "\n";
        // //$latex .= $startIndexTable;
        // $latex .= '\fancyhead[ER]{' . "INDEX OF ARROWED WORDS" . "}\n";
        // $latex .= "\\noindent \n";

        // $boldLemmata = FALSE;
        // $latex = str_replace("%BOLDLEMMATA%", ($boldLemmata) ? '\bf' : "", $latex);
        // usort($arrowedIndex, "indexsort");
        // $i = 0;
        // foreach ($arrowedIndex as $a) {
        //     //$latex .= explode(",", $a[0], 2)[0] . " & " . $a[2] . " \\\\ \n";
        //     $latex .= explode(",", $a[0], 2)[0] . " \dotfill " . $a[2] . " \\\\ \n";
        //     $i++;
        //     if ($i > 43) {
        //         $i = 0;
        //         //$latex .= "\\end{tabular}\n";
        //         $latex .= "\\newpage \n";
        //         $latex .= "\\noindent \n";
        //         //$latex .= $startIndexTable . "\n";
        //     }

        // }
        // //$latex .= '\end{tabular}' . "\n";
        // $latex .= '\end{document}' . "\n";
        latex.push_str(ARROWED_INDEX_TEMPLATE);

        arrowed_words_index.sort_by(|a, b| {
            a.gloss_sort
                .to_lowercase()
                .cmp(&b.gloss_sort.to_lowercase())
        });
        let mut gloss_per_page = 0;
        for gloss in arrowed_words_index {
            //$latex .= explode(",", $a[0], 2)[0] . " \dotfill " . $a[2] . " \\\\ \n";
            latex.push_str(
                &gloss
                    .gloss_lemma
                    .chars()
                    .take_while(|&ch| ch != ',')
                    .collect::<String>(),
            );
            latex.push_str(r" \dotfill ");
            latex.push_str(&gloss.page_number.to_string());
            latex.push_str(" \\\\ \n");

            gloss_per_page += 1;
            if gloss_per_page > 43 {
                gloss_per_page = 0;
                latex.push_str("\\newpage \n");
                latex.push_str("\\noindent \n");
            }
        }
    }

    latex.push_str("\\end{document}\n");
    Ok(latex)
}

fn format_verse_line(word: &str) -> String {
    let word_input = word.replace("[line]", "");
    let mut output = String::from("");

    if word_input.contains('-') {
        // 105-106, etc.
        output = word_input // print everything for line ranges
    } else {
        let re = Regex::new("([0-9]+)(.*)").unwrap();
        let matches = re.captures(&word_input);

        if let Some(matches) = matches {
            let line_num = matches.get(1).unwrap().as_str();
            let line_num2 = line_num.parse::<u32>().unwrap();
            if line_num2 % 5 == 0 {
                output = word_input // 175 [str
            } else if let Some(rest) = matches.get(2) {
                output = rest.as_str().to_string() // just [str etc.
            }
        }
    }
    output.to_lowercase()
}

//for thuc
fn fix_subsection(word: &str) -> String {
    let section_input = word.replace("[section]", "");

    let re = Regex::new("([0-9]+)[.]([0-9]+)").unwrap();
    let matches = re.captures(&section_input);

    if let Some(matches) = matches {
        let section = matches.get(1).unwrap().as_str();
        let subsection = matches.get(2).unwrap().as_str();

        if subsection == "1" {
            format!("%StartSubSection%{}%EndSubSection%", section)
        } else {
            format!("%StartSubSubSection%{}%EndSubSubSection%", subsection)
        }
    } else {
        format!("%StartSubSection%{}%EndSubSection%", section_input)
    }
}

fn apply_latex_templates(
    latex: &mut String,
    title: &str,
    text: &mut String,
    glosses: &Vec<Gloss>,
    header: &str,
    app_crits: &Vec<String>,
) -> String {
    latex.push_str("\\newpage\n");

    if !header.is_empty() {
        if !title.is_empty() {
            latex.push_str("\\fancyhead[OR]{ }\n");
        } else {
            latex.push_str(format!("\\fancyhead[OR]{{{}}}\n", header).as_str());
        }
    }
    if !title.is_empty() {
        latex.push_str(
            format!(
                "\\begin{{center}}\\noindent\\textbf{{{}}}\\par\\end{{center}}\n",
                title
            )
            .as_str(),
        );
    }

    if text.contains("%VERSELINESTART%") {
        *text = text.replace("%StartSubTitle%", "");
        *text = text.replace("%EndSubTitle%", " \\\\ "); //add this even if no %StartSubTitle%
        *text = text.replace("%LINEEND%", " \\\\ \n");

        *text = text.replace("%VERSEREALLINESTART%", " & ");
        *text = text.replace("%VERSELINESTART%", " & ");
        *text = text.replace("%VERSELINEEND%", " \\\\ \n");
        latex.push_str(
            format!(
                "{}{}~\\\\\n\\end{{tabular}}\n",
                include_str!("latex/verse_table_start.tex"),
                text
            )
            .as_str(),
        );
    } else {
        latex.push_str("\\begin{spacing}{\\GlossLineSpacing}\n");
        latex.push_str("\\noindent\n");

        *text = text.replace("%StartSubTitle%", "\\begin{center}"); //\hspace{0pt} solves problem when \par\marginsec{} come together
        *text = text.replace("%EndSubTitle%", "\\end{center}");

        *text = text.replace("%StartSubSection%", "\\hspace{0pt}\\marginsec{"); //\hspace{0pt} solves problem when \par\marginsec{} come together
        *text = text.replace("%EndSubSection%", "}");

        *text = text.replace("%parnoindent%", "\n\\par\\noindent\n");

        *text = text.replace("%StartSubSubSection%", "\\hspace{0pt}\\marginseclight{"); //\hspace{0pt} solves problem when \par\marginsec{} come together
        *text = text.replace("%EndSubSubSection%", "}");
        *text = text.replace("%para%", "\n\\par\n");

        *text = text.replace("%StartInnerSubTitle%", "\\par \\textbf{"); //\hspace{0pt} solves problem when \par\marginsec{} come together
        *text = text.replace("%EndInnerSubTitle%", "} ");

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

    *latex = latex.replace(r"& Α´", "Α´"); //hack to fix two type 2s together, lines 866
    *latex = latex.replace(r"& Β´", "Β´"); //hack to fix two type 2s together, lines 872

    latex.to_string()
}

const ARROWED_INDEX_TEMPLATE: &str = r##"
\newpage
\fancyhead[OR]{INDEX OF ARROWED WORDS}
%\begin{spacing}{\GlossLineSpacing}
\noindent
"##;

// const ARROWED_INDEX_TEMPLATE: &str = r##"
// \documentclass[twoside,openright,12pt,letterpaper]{book}
// %\usepackage[margin=1.0in]{geometry}
// \usepackage[twoside, margin=1.0in]{geometry} %bindingoffset=0.5in,
// \usepackage[utf8]{inputenc}
// \usepackage{fontspec}
// \usepackage{array}
// \usepackage{booktabs}
// \usepackage{ragged2e}
// \usepackage{setspace}
// \usepackage{navigator}

// \newcommand{\GlossLineSpacing}{1.5}

// \setmainfont[Scale=MatchUppercase,Ligatures=TeX, BoldFont={*BOLD}, ItalicFont={IFAOGrec.ttf}, ItalicFeatures={FakeSlant=0.2}]{IFAOGrec.ttf}
// %\setmainlanguage[variant=polytonic]{greek}
// \tolerance=10000 % https://www.texfaq.org/FAQ-overfull
// \setlength{\extrarowheight}{8pt}
// \newcolumntype{L}{>{\setlength{\RaggedRight\parindent}{-2em}\leftskip 2em}p}
// \newcolumntype{D}{>{\setlength{\RaggedRight}}p}

// \usepackage{fancyhdr} % http://tug.ctan.org/tex-archive/macros/latex/contrib/fancyhdr/fancyhdr.pdf

// \pagestyle{fancy}
// \fancyhf{}
// \renewcommand{\headrulewidth}{0.0pt}
//   \fancyhead[OL]{LGI - UPPER LEVEL GREEK}% Author on Odd page, Centred
//   \fancyhead[ER]{INDEX OF ARROWED WORDS}% Title on Even page, Centred
// \setlength{\headheight}{14.49998pt}
// \cfoot{\thepage}

// %\usepackage{enumitem}
// %\SetLabelAlign{margin}{\llap{#1~~}}
// %\usepackage{showframe} % just to show the margins
// %https://tex.stackexchange.com/questions/223701/labels-in-the-left-margin

// %https://tex.stackexchange.com/questions/40748/use-sections-inline
// \newcommand{\marginsec}[1]{\vadjust{\vbox to 0pt{\sbox0{\bfseries#1\quad}\kern-0.89em\llap{\box0}}}}
// \newcommand{\marginseclight}[1]{\vadjust{\vbox to 0pt{\sbox0{\footnotesize#1\hspace{0.25em}\quad}\kern-0.85em\llap{\box0}}}}
// \usepackage[none]{hyphenat}
// \usepackage[polutonikogreek,english]{babel} %https://tex.stackexchange.com/questions/13067/utf8x-vs-utf8-inputenc
// \usepackage{microtype}
// \begin{document}
// \clearpage
// \setcounter{page}{1}
// \mbox{}
// \newpage
// \noindent
// "##;
