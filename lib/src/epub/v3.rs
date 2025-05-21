use super::core::EpubNav;
use crate::prelude::*;
use quick_xml::{events::Event, Reader};
use std::borrow::Cow;
macro_rules! invalid {
    ($x:tt) => {
        Err(IError::InvalidArchive(Cow::from($x)))
    };
    ($x:expr,$y:expr) => {
        $x.or(Err(IError::InvalidArchive(Cow::from($y))))?
    };
}

pub fn read_nav_xhtml(toc_str: &str) -> IResult<Vec<EpubNav>> {
    let mut reader = Reader::from_str(toc_str);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut items = Vec::new();
    let mut current_title = String::new();
    let mut current_href = String::new();
    let mut in_toc = false;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                if e.name().as_ref() == b"nav" {
                    for attr in e.attributes() {
                        if let Ok(attr) = attr {
                            if attr.key.as_ref() == b"epub:type" && attr.value.as_ref() == b"toc" {
                                in_toc = true;
                                break;
                            }
                        }
                    }
                } else if in_toc && e.name().as_ref() == b"a" {
                    for attr in e.attributes() {
                        if let Ok(attr) = attr {
                            if attr.key.as_ref() == b"href" {
                                current_href = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                    }
                }
            }
            Ok(Event::Text(e)) => {
                if in_toc {
                    current_title = e.unescape().unwrap().to_string();
                }
            }
            Ok(Event::End(ref e)) => {
                if in_toc && e.name().as_ref() == b"a" {
                    if !current_title.is_empty() && !current_href.is_empty() {
                        let mut nav = EpubNav::default();
                        nav.set_title(&current_title);
                        nav.set_file_name(&current_href);
                        items.push(nav);
                        current_title.clear();
                        current_href.clear();
                    }
                } else if e.name().as_ref() == b"nav" {
                    in_toc = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(_e) => return invalid!("err"),
            _ => {}
        }
        buf.clear();
    }

    Ok(items)
}
