//https://users.rust-lang.org/t/file-upload-in-actix-web/64871/3

use quick_xml::Reader;
use quick_xml::events::Event;

use actix_multipart::Multipart;
use futures::{StreamExt, TryStreamExt};

use super::*;

//select b.gloss_id,b.lemma,count(b.gloss_id) c from words a inner join glosses b on a.gloss_id=b.gloss_id group by b.gloss_id order by c;
fn lemmatize_simple(word:&str) -> Option<u32> {
    match word {
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

fn split_words(text: &str, in_speaker:bool, in_head:bool) -> Vec<TextWord> {
    let mut words:Vec<TextWord> = vec![];
    let mut last = 0;
    if in_head {
        words.push(TextWord{word: text.to_string(),word_type:WordType::WorkTitle as u32, gloss_id:None});
    }
    else if in_speaker {
        words.push(TextWord{word: text.to_string(),word_type:WordType::Speaker as u32, gloss_id:None});
    }
    else {
        for (index, matched) in text.match_indices(|c: char| !(c.is_alphanumeric() || c == '\'')) {
            //add words
            if last != index && &text[last..index] != " " {
                let gloss_id = lemmatize_simple(&text[last..index]);
                words.push(TextWord{word: text[last..index].to_string(), word_type: WordType::Word as u32, gloss_id});
            }
            //add word separators
            if matched != " " {
                words.push(TextWord{word:matched.to_string(),word_type:WordType::Punctuation as u32, gloss_id:None});
            }
            last = index + matched.len();
        }
        //add last word
        if last < text.len() && &text[last..] != " " {
            let gloss_id = lemmatize_simple(&text[last..]);
            words.push(TextWord{word:text[last..].to_string(),word_type:WordType::Word as u32, gloss_id});
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
                        words.extend_from_slice(&split_words(&s, in_speaker, in_head)[..]);

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
    /* 
    for a in words {
        println!("{} {}", a.word, a.word_type);
    }*/
    Ok(words)
}
