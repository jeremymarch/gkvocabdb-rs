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

//https://users.rust-lang.org/t/file-upload-in-actix-web/64871/3

use quick_xml::events::Event;
use quick_xml::Reader;

use quick_xml::name::QName;
use std::collections::HashMap;

use super::*;

pub async fn import(
    db: &SqlitePool,
    course_id: u32,
    info: &ConnectionInfo,
    title: &str,
    xml_string: &str,
) -> ImportResponse {
    let lemmatizer = db::get_lemmatizer(db).await;

    match import_text_xml::process_imported_text(xml_string, &lemmatizer).await {
        Ok(words) => {
            if !words.is_empty() && !title.is_empty() {
                let affected_rows = db::add_text(db, course_id, title, words, info)
                    .await
                    .map_err(|e| ImportResponse {
                        success: false,
                        words_inserted: 0,
                        error: format!("sqlx error: {}", e),
                    });

                ImportResponse {
                    success: true,
                    words_inserted: affected_rows.unwrap(),
                    error: "".to_string(),
                }
            } else {
                ImportResponse {
                    success: false,
                    words_inserted: 0,
                    error: "Error importing text: File and/or Title field(s) is/are empty."
                        .to_string(),
                }
            }
        }
        Err(e) => ImportResponse {
            success: false,
            words_inserted: 0,
            error: format!("Error importing text: XML parse error: {:?}.", e),
        },
    }
}

fn sanitize_greek(s: &str) -> String {
    use regex::Regex;
    let smooth_breathing_re = Regex::new(r"\u{1FBF}(?P<letter>.)").unwrap();
    let r = smooth_breathing_re.replace_all(s, "$letter\u{0313}");

    let rough_breathing_re = Regex::new(r"\u{1FFE}(?P<letter>.)").unwrap();
    let r = rough_breathing_re.replace_all(&r, "$letter\u{0314}");

    let rough_breathing_acute_re = Regex::new(r"\u{1FDE}(?P<letter>.)").unwrap();
    let r = rough_breathing_acute_re.replace_all(&r, "$letter\u{0314}\u{0301}");

    let smooth_breathing_acute_re = Regex::new(r"\u{1FCE}(?P<letter>.)").unwrap();
    let r = smooth_breathing_acute_re.replace_all(&r, "$letter\u{0313}\u{0301}");

    let smooth_breathing_circumflex_re = Regex::new(r"\u{1FCF}(?P<letter>.)").unwrap();
    let r = smooth_breathing_circumflex_re.replace_all(&r, "$letter\u{0313}\u{0342}");

    let rough_breathing_circumflex_re = Regex::new(r"\u{1FCD}(?P<letter>.)").unwrap();
    let r = rough_breathing_circumflex_re.replace_all(&r, "$letter\u{0314}\u{0342}");

    let smooth_breathing_grave_re = Regex::new(r"\u{1FCD}(?P<letter>.)").unwrap();
    let r = smooth_breathing_grave_re.replace_all(&r, "$letter\u{0313}\u{0300}");

    let rough_breathing_grave_re = Regex::new(r"\u{1FDD}(?P<letter>.)").unwrap();
    let r = rough_breathing_grave_re.replace_all(&r, "$letter\u{0314}\u{0300}");

    //https://apagreekkeys.org/technicalDetails.html
    r.replace('\u{1F71}', "\u{03AC}") //acute -> tonos, etc...
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
}

fn split_words(
    text: &str,
    in_speaker: bool,
    in_head: bool,
    in_desc: bool,
    lemmatizer: &HashMap<String, u32>,
) -> Vec<TextWord> {
    let mut words: Vec<TextWord> = vec![];
    let mut last = 0;
    let word_type_word = if in_desc {
        WordType::Desc
    } else {
        WordType::Word
    } as u32;
    if in_head {
        words.push(TextWord {
            word: text.to_string(),
            word_type: WordType::WorkTitle as u32,
            gloss_id: None,
        });
    } else if in_speaker {
        words.push(TextWord {
            word: text.to_string(),
            word_type: WordType::Speaker as u32,
            gloss_id: None,
        });
    } else {
        for (index, matched) in text.match_indices(|c: char| {
            !(c.is_alphanumeric() || c == '\'' || unicode_normalization::char::is_combining_mark(c))
        }) {
            //add words
            if last != index && &text[last..index] != " " {
                let gloss_id = lemmatizer.get(&text[last..index]).copied();
                words.push(TextWord {
                    word: text[last..index].to_string(),
                    word_type: word_type_word,
                    gloss_id,
                });
            }
            //add word separators
            if matched != " " {
                words.push(TextWord {
                    word: matched.to_string(),
                    word_type: WordType::Punctuation as u32,
                    gloss_id: None,
                });
            }
            last = index + matched.len();
        }
        //add last word
        if last < text.len() && &text[last..] != " " {
            let gloss_id = lemmatizer.get(&text[last..]).copied();
            words.push(TextWord {
                word: text[last..].to_string(),
                word_type: word_type_word,
                gloss_id,
            });
        }
    }
    words
}

pub async fn process_imported_text(
    xml_string: &str,
    lemmatizer: &HashMap<String, u32>,
) -> Result<Vec<TextWord>, quick_xml::Error> {
    let mut words: Vec<TextWord> = Vec::new();

    let mut reader = Reader::from_str(xml_string);
    reader.trim_text(true);
    let mut buf = Vec::new();

    let mut in_text = false;
    let mut in_speaker = false;
    let mut in_head = false;
    let mut found_tei = false;
    let mut in_desc = false;
    /*
    TEI: verse lines can either be empty <lb n="5"/>blah OR <l n="5">blah</l>
    see Perseus's Theocritus for <lb/> and Euripides for <l></l>
    */

    loop {
        match reader.read_event_into(&mut buf) {
            // for triggering namespaced events, use this instead:
            // match reader.read_namespaced_event(&mut buf) {
            Ok(Event::Start(ref e)) => {
                // for namespaced:
                // Ok((ref namespace_value, Event::Start(ref e)))
                if b"text" == e.name().as_ref() {
                    in_text = true;
                } else if b"speaker" == e.name().as_ref() {
                    in_speaker = true;
                } else if b"head" == e.name().as_ref() {
                    in_head = true;
                } else if b"TEI.2" == e.name().as_ref() || b"TEI" == e.name().as_ref() {
                    found_tei = true;
                } else if b"desc" == e.name().as_ref() {
                    in_desc = true;
                    words.push(TextWord {
                        word: "".to_string(),
                        word_type: WordType::ParaNoIndent as u32,
                        gloss_id: None,
                    });
                } else if b"p" == e.name().as_ref() {
                    words.push(TextWord {
                        word: String::from(""),
                        word_type: WordType::ParaWithIndent as u32,
                        gloss_id: None,
                    });
                } else if b"l" == e.name().as_ref() {
                    let mut line_num = "".to_string();

                    for a in e.attributes() {
                        //.next().unwrap().unwrap();
                        if a.as_ref().unwrap().key == QName(b"n") {
                            line_num = std::str::from_utf8(&a.unwrap().value).unwrap().to_string();
                        }
                    }
                    words.push(TextWord {
                        word: format!("[line]{}", line_num),
                        word_type: WordType::VerseLine as u32,
                        gloss_id: None,
                    });
                }
            }
            // unescape and decode the text event using the reader encoding
            Ok(Event::Text(ref e)) => {
                if in_text {
                    if let Ok(s) = e.unescape() {
                        //let seperator = Regex::new(r"([ ,.;]+)").expect("Invalid regex");
                        let clean_string = sanitize_greek(&s);
                        words.extend_from_slice(
                            &split_words(&clean_string, in_speaker, in_head, in_desc, lemmatizer)[..],
                        );

                        //let mut splits: Vec<String> = s.split_inclusive(&['\t','\n','\r',' ',',', ';','.']).map(|s| s.to_string()).collect();
                        //words2.word.extend_from_slice(&words.word[..]);
                        //words2.word_type.extend_from_slice(&words.word_type[..]);
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                if b"lb" == e.name().as_ref() {
                    //line beginning
                    let mut line_num = "".to_string();

                    for a in e.attributes() {
                        //.next().unwrap().unwrap();
                        if a.as_ref().unwrap().key == QName(b"n") {
                            line_num = std::str::from_utf8(&a.unwrap().value).unwrap().to_string();
                        }
                    }
                    words.push(TextWord {
                        word: format!("[line]{}", line_num),
                        word_type: WordType::VerseLine as u32,
                        gloss_id: None,
                    });
                } else if b"pb" == e.name().as_ref() {
                    //page beginning
                    words.push(TextWord {
                        word: "".to_string(),
                        word_type: WordType::PageBreak as u32,
                        gloss_id: None,
                    });
                }
            }
            Ok(Event::End(ref e)) => {
                if b"text" == e.name().as_ref() {
                    in_text = false;
                } else if b"speaker" == e.name().as_ref() {
                    in_speaker = false;
                } else if b"head" == e.name().as_ref() {
                    in_head = false;
                } else if b"desc" == e.name().as_ref() {
                    in_desc = false;
                    words.push(TextWord {
                        word: "".to_string(),
                        word_type: WordType::ParaNoIndent as u32,
                        gloss_id: None,
                    });
                }
            }
            Ok(Event::Eof) => break, // exits the loop when reaching end of file
            Err(e) => {
                words.clear();
                return Err(e);
            } //return empty vec on error //panic!("Error at position {}: {:?}", reader.buffer_position(), e),
            _ => (), // There are several other `Event`s we do not consider here
        }

        // if we don't keep a borrow elsewhere, we can clear the buffer to keep memory usage low
        buf.clear();
    }
    if !found_tei {
        //using this error for now, if doc does not even try to be tei
        return Err(quick_xml::Error::UnexpectedToken(
            "Missing TEI.2 tags".to_string(),
        ));
    }
    /*
    for a in words {
        println!("{} {}", a.word, a.word_type);
    }*/
    Ok(words)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[actix_rt::test]
    async fn test_import() {
        let mut lemmatizer = HashMap::new();
        lemmatizer.insert("δ".to_string(), 30);

        //<?xml version="1.0" encoding="UTF-8"?> is optional
        let xml_string = r#"<TEI.2>
            <text lang="greek">
                <head>Θύρσις ἢ ᾠδή</head>
                <speaker>Θύρσις</speaker>
                <lb rend="displayNum" n="5" />αἴκα δ᾽ αἶγα λάβῃ τῆνος γέρας, ἐς τὲ καταρρεῖ
                <pb/>
                <l n="10">ὁσίου γὰρ ἀνδρὸς ὅσιος ὢν ἐτύγχανον</l>
                <desc>This is a test.</desc>
            </text>
        </TEI.2>"#;
        let r = process_imported_text(xml_string, &lemmatizer)
            .await
            .unwrap();
        //to see this: cargo test -- --nocapture
        // for a in &r {
        //     println!("{:?}", a);
        // }
        assert_eq!(r.len(), 29);
        assert_eq!(r[0].word_type, import_text_xml::WordType::WorkTitle as u32);
        assert_eq!(r[1].word_type, import_text_xml::WordType::Speaker as u32);
        assert_eq!(r[2].word_type, import_text_xml::WordType::VerseLine as u32);
        assert_eq!(r[2].word, "[line]5");
        assert_eq!(r[3].word_type, import_text_xml::WordType::Word as u32);
        assert_eq!(r[4].gloss_id, Some(30));
        assert_eq!(
            r[10].word_type,
            import_text_xml::WordType::Punctuation as u32
        );
        assert_eq!(r[14].word_type, WordType::PageBreak as u32);
        assert_eq!(r[15].word_type, import_text_xml::WordType::VerseLine as u32);
        assert_eq!(r[15].word, "[line]10");
        assert_eq!(r[22].word, "");
        assert_eq!(
            r[22].word_type,
            import_text_xml::WordType::ParaNoIndent as u32
        );
        assert_eq!(r[23].word, "This");
        assert_eq!(r[23].word_type, import_text_xml::WordType::Desc as u32);
        assert_eq!(r[28].word, "");
        assert_eq!(
            r[28].word_type,
            import_text_xml::WordType::ParaNoIndent as u32
        );
    }

    #[test]
    fn test_split() {
        let lemmatizer = HashMap::new();

        // establish that combining chars are not alphanumeric
        assert!(!'\u{0313}'.is_alphanumeric());

        // therefore: be sure combining diacritics do not divide words (this is why we use unicode_normalization::char::is_combining_mark(c))
        let a = split_words("α\u{0313}α ββ", false, false, false, &lemmatizer);
        assert_eq!(a.len(), 2);

        // be sure ' does not divide words
        let a = split_words("δ' ββ", false, false, false, &lemmatizer);
        assert_eq!(a.len(), 2);
        assert_eq!(a[0].word, "δ'");
    }

    #[test]
    fn test_sanitize_greek() {
        let a = sanitize_greek("\u{1FBF}Αφροδίτ\u{1FBF}Αα ββ");
        assert_eq!(a, "Α\u{0313}φροδίτΑ\u{0313}α ββ");

        let a = sanitize_greek("\u{1FFE}Εκάτα\u{1FFE}Εκάτα ββ");
        assert_eq!(a, "Ε\u{0314}κάταΕ\u{0314}κάτα ββ");

        let a = sanitize_greek("\u{1FDE}Αιδα\u{1FDE}Αιδα ββ");
        assert_eq!(a, "Α\u{0314}\u{0301}ιδαΑ\u{0314}\u{0301}ιδα ββ");

        let a = sanitize_greek("\u{1FCE}Ερως\u{1FCE}Αρτεμι ββ");
        assert_eq!(a, "Ε\u{0313}\u{0301}ρωςΑ\u{0313}\u{0301}ρτεμι ββ");
    }
}
