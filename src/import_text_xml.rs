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

use quick_xml::Reader;
use quick_xml::events::Event;

use actix_multipart::Multipart;
use futures::{StreamExt, TryStreamExt};

use super::*;

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
    ParaNoIndent = 10,
    PageBreak = 11,
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

pub async fn import_text((session, payload, req): (Session, Multipart, HttpRequest)) -> Result<HttpResponse> {
    let db = req.app_data::<SqlitePool>().unwrap();

    let course_id = 1;

    if let Some(user_id) = login::get_user_id(session) {
        let timestamp = get_timestamp();
        let updated_ip = get_ip(&req).unwrap_or_else(|| "".to_string());
        let user_agent = get_user_agent(&req).unwrap_or("");

        match import_text_xml::get_xml_string(payload).await {
            Ok((xml_string, title)) => {

                match import_text_xml::process_imported_text(xml_string).await {
                    Ok(words) => {
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
                                error: "Error importing text: File and/or Title field(s) is/are empty.".to_string(),
                            };
                            Ok(HttpResponse::Ok().json(res))
                        }
                    },
                    Err(e) => {
                        let res = ImportResponse {
                            success: false,
                            words_inserted: 0,
                            error: format!("Error importing text: XML parse error: {:?}.", e),
                        };
                        Ok(HttpResponse::Ok().json(res))
                    }
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

fn sanitize_greek(s:&str) -> String {

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
    let r = r.replace("\u{1F71}", "\u{03AC}") //acute -> tonos, etc...
        .replace("\u{1FBB}", "\u{0386}") 
        .replace("\u{1F73}", "\u{03AD}")
        .replace("\u{1FC9}", "\u{0388}")
        .replace("\u{1F75}", "\u{03AE}")
        .replace("\u{1FCB}", "\u{0389}")
        .replace("\u{1F77}", "\u{03AF}")
        .replace("\u{1FDB}", "\u{038A}")
        .replace("\u{1F79}", "\u{03CC}")
        .replace("\u{1FF9}", "\u{038C}")
        .replace("\u{1F7B}", "\u{03CD}")
        .replace("\u{1FEB}", "\u{038E}")
        .replace("\u{1F7D}", "\u{03CE}")
        .replace("\u{1FFB}", "\u{038F}")
        .replace("\u{1FD3}", "\u{0390}") //iota + diaeresis + acute
        .replace("\u{1FE3}", "\u{03B0}") //upsilon + diaeresis + acute
        .replace("\u{037E}", "\u{003B}") //semicolon
        .replace("\u{0387}", "\u{00B7}") //middle dot
        .replace("\u{0344}", "\u{0308}\u{0301}"); //combining diaeresis with acute
               
    r.to_string()
}

fn split_words(text: &str, in_speaker:bool, in_head:bool) -> Vec<TextWord> {
    let mut words:Vec<TextWord> = vec![];
    let mut last = 0;
    if in_head {
        words.push(TextWord{word: text.to_string(), word_type:WordType::WorkTitle as u32, gloss_id:None});
    }
    else if in_speaker {
        words.push(TextWord{word: text.to_string(), word_type:WordType::Speaker as u32, gloss_id:None});
    }
    else {
        for (index, matched) in text.match_indices(|c: char| !(c.is_alphanumeric() || c == '\'' || unicode_normalization::char::is_combining_mark(c))) {
            //add words
            if last != index && &text[last..index] != " " {
                let gloss_id = lemmatize_simple(&text[last..index]);
                words.push(TextWord{word: text[last..index].to_string(), word_type: WordType::Word as u32, gloss_id});
            }
            //add word separators
            if matched != " " {
                words.push(TextWord{word:matched.to_string(), word_type:WordType::Punctuation as u32, gloss_id:None});
            }
            last = index + matched.len();
        }
        //add last word
        if last < text.len() && &text[last..] != " " {
            let gloss_id = lemmatize_simple(&text[last..]);
            words.push(TextWord{word:text[last..].to_string(), word_type:WordType::Word as u32, gloss_id});
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

pub async fn process_imported_text(xml_string: String) -> Result<Vec<TextWord>, quick_xml::Error> {
    let mut words:Vec<TextWord> = Vec::new();

    let mut reader = Reader::from_str(&xml_string);
    reader.trim_text(true);
    let mut buf = Vec::new();

    let mut in_text = false;
    let mut in_speaker = false;
    let mut in_head = false;
    let mut found_tei = false;
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
                else if b"TEI.2" == e.name() { found_tei = true }
                else if b"l" == e.name() { 
                    let mut line_num = "".to_string();
                    
                    for a in e.attributes() { //.next().unwrap().unwrap();
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
                        let clean_string = sanitize_greek(&s);
                        words.extend_from_slice(&split_words(&clean_string, in_speaker, in_head)[..]);

                        //let mut splits: Vec<String> = s.split_inclusive(&['\t','\n','\r',' ',',', ';','.']).map(|s| s.to_string()).collect();
                        //words2.word.extend_from_slice(&words.word[..]);
                        //words2.word_type.extend_from_slice(&words.word_type[..]);
                    }
                }
            },
            Ok(Event::Empty(ref e)) => {
                if b"lb" == e.name() { 
                    let mut line_num = "".to_string();
                    
                    for a in e.attributes() { //.next().unwrap().unwrap();
                        if std::str::from_utf8(a.as_ref().unwrap().key).unwrap() == "n" {         
                            line_num = std::str::from_utf8(&*a.unwrap().value).unwrap().to_string();
                        }
                    }
                    words.push( TextWord{ word: format!("[line]{}", line_num), word_type: WordType::VerseLine as u32,gloss_id:None }); 
                }
                else if b"gkvocab_page_break" == e.name() { 
                    words.push( TextWord{ word: "".to_string(), word_type: WordType::PageBreak as u32,gloss_id:None }); 
                }
            },
            Ok(Event::End(ref e)) => {
                if b"text" == e.name() { in_text = false }     
                else if b"speaker" == e.name() { in_speaker = false }
                else if b"head" == e.name() { in_head = false }
            },
            Ok(Event::Eof) => break, // exits the loop when reaching end of file
            Err(e) => { 
                words.clear(); 
                return Err(e); 
            }, //return empty vec on error //panic!("Error at position {}: {:?}", reader.buffer_position(), e),
            _ => (), // There are several other `Event`s we do not consider here
        }
    
        // if we don't keep a borrow elsewhere, we can clear the buffer to keep memory usage low
        buf.clear();
    }
    if !found_tei {
        //using this error for now, if doc does not even try to be tei
        return Err(quick_xml::Error::UnexpectedToken("Missing TEI.2 tags".to_string()));
    }
    /* 
    for a in words {
        println!("{} {}", a.word, a.word_type);
    }*/
    Ok(words)
}

//select b.gloss_id,b.lemma,count(b.gloss_id) c from words a inner join glosses b on a.gloss_id=b.gloss_id group by b.gloss_id order by c;
fn lemmatize_simple(word:&str) -> Option<u32> {
    match word {
        "ὁ" => Some(16),
        "τοῦ" => Some(16),
        "τὸν" => Some(16),
        "τῷ" => Some(16),
        "οἱ" => Some(16),
        "τῶν" => Some(16),
        "τοὺς" => Some(16),
        "τοῖς" => Some(16),
        "ἡ" => Some(16),
        "τῆς" => Some(16),
        "τὴν" => Some(16),
        "τῇ" => Some(16),
        "αἱ" => Some(16),
        "τὰς" => Some(16),
        "ταῖς" => Some(16),
        "τὸ" => Some(16),
        "τὰ" => Some(16),

        "οὗτος" => Some(16),
        "τοῦτο" => Some(16),
        "ταῦτα" => Some(16),
        "ἐκεῖνο" => Some(16),

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
        "με" => Some(392),
        "μέ" => Some(392),
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
        "μοι" => Some(392),
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
        "σοι" => Some(408),
        "σε" => Some(408),
        "σὲ" => Some(408),
        "σέ" => Some(408),
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

#[test]
fn test_split() {
    assert_eq!('\u{0313}'.is_alphanumeric(), false); //combining chars are not alphanumeric, so
    // be sure combining diacritics do not divide words (this is why we use unicode_normalization::char::is_combining_mark(c))
    let a = split_words("α\u{0313}α ββ", false, false);
    assert_eq!(a.len(), 2);

    // be sure ' does not divide words
    let a = split_words("δ' ββ", false, false);
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

