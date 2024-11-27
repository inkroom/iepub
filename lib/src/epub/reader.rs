use std::{
    io::{Read, Seek},
    ops::Deref,
};

use quick_xml::events::BytesStart;

use super::core::EpubReaderTrait;
use crate::prelude::*;
macro_rules! invalid {
    ($x:tt) => {
        Err(IError::InvalidArchive($x))
    };
    ($x:expr,$y:expr) => {
        $x.or(Err(IError::InvalidArchive($y)))?
    };
}

macro_rules! read_from_zip {
    ($m:ident,$x:expr) => {{
        // 读取 container.xml
        let mut file = invalid!($m.by_name($x), stringify!($x not exist));
        let mut content = String::new();
        invalid!(file.read_to_string(&mut content), "read err");
        content
    }};
}

fn get_opf_location(xml: &str) -> IResult<String> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let mut reader = Reader::from_str(xml);
    // reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => {
                return Ok(String::new());
            }
            Err(e) => {
                return Err(IError::Xml(e));
            }
            Ok(Event::Empty(e)) => {
                if e.name().as_ref() == b"rootfile" {
                    if let Ok(path) = e.try_get_attribute("full-path") {
                        return match path.map(|f| {
                            f.unescape_value()
                                .map_or_else(|_| String::new(), |v| v.to_string())
                        }) {
                            Some(v) => Ok(v),
                            None => Err(IError::InvalidArchive("has no opf")),
                        };
                    }
                }
            }
            _ => (),
        }
        buf.clear();
    }
}

fn create_meta(xml: &BytesStart) -> IResult<EpubMetaData> {
    let mut meta = EpubMetaData::default();

    for ele in xml.attributes() {
        if let Ok(a) = ele {
            meta.push_attr(
                String::from_utf8(a.key.0.to_vec()).unwrap().as_str(),
                String::from_utf8(a.value.to_vec()).unwrap().as_str(),
            );
        }
    }
    Ok(meta)
}

fn read_meta_xml(
    reader: &mut quick_xml::reader::Reader<&[u8]>,
    book: &mut EpubBook,
) -> IResult<()> {
    use quick_xml::events::Event;

    // 模拟 栈，记录当前的层级
    let mut parent: Vec<String> = vec!["package".to_string(), "metadata".to_string()];
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => {
                break;
            }
            Err(_e) => {
                return invalid!("err");
            }
            Ok(Event::Start(e)) => {
                let name = String::from_utf8(e.name().as_ref().to_vec()).map_err(IError::Utf8)?;

                if name == "meta" {
                    if parent.len() != 2 || parent[1] != "metadata" {
                        return invalid!("not valid opf meta");
                    } else {
                        let meta = create_meta(&e);
                        if let Ok(m) = meta {
                            book.add_meta(m);
                        }
                        parent.push("meta".to_string());
                    }
                } else if parent.len() != 2 || parent[1] != "metadata" {
                    return invalid!("not valid opf identifier");
                } else {
                    parent.push(name);
                }
            }
            Ok(Event::Empty(e)) => match e.name().as_ref() {
                b"meta" => {
                    if parent.len() != 2 || parent[1] != "metadata" {
                        return invalid!("not valid opf meta empty");
                    } else {
                        let meta = create_meta(&e);
                        if let Ok(m) = meta {
                            book.add_meta(m);
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::Text(txt)) => {
                if !parent.is_empty() {
                    match parent[parent.len() - 1].as_str() {
                        "meta" => {
                            if let Some(m) = book.get_meta_mut(book.meta_len() - 1) {
                                m.set_text(txt.unescape()?.deref());
                            }
                        }
                        "dc:identifier" => {
                            book.set_identifier(txt.unescape()?.deref());
                        }
                        "dc:title" => {
                            book.set_title(txt.unescape()?.deref());
                        }
                        "dc:creator" => {
                            book.set_creator(txt.unescape()?.deref());
                        }
                        "dc:description" => {
                            book.set_description(txt.unescape()?.deref());
                        }
                        "dc:format" => {
                            book.set_format(txt.unescape()?.deref());
                        }
                        "dc:publisher" => {
                            book.set_publisher(txt.unescape()?.deref());
                        }
                        "dc:subject" => {
                            book.set_subject(txt.unescape()?.deref());
                        }
                        "dc:contributor" => {
                            book.set_contributor(txt.unescape()?.deref());
                        }
                        "dc:date" => {
                            book.set_date(txt.unescape()?.deref());
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name = String::from_utf8(e.name().as_ref().to_vec()).map_err(IError::Utf8)?;

                if name == "metadata" {
                    if parent.len() != 2 || parent[0] != "package" {
                        return invalid!("not valid opf metadata end");
                    }
                    break;
                }

                if !parent.is_empty() && parent[parent.len() - 1] == name {
                    parent.remove(parent.len() - 1);
                }

                // println!("end {}",String::from_utf8(e.name().as_ref().to_vec()).unwrap());
            }
            _ => {}
        }
    }
    Ok(())
}

fn read_spine_xml(
    reader: &mut quick_xml::reader::Reader<&[u8]>,
    book: &mut EpubBook,
    assets: &mut Vec<EpubAssets>,
) -> IResult<()> {
    use quick_xml::events::Event;

    // 模拟 栈，记录当前的层级
    let _parent: Vec<String> = vec!["package".to_string(), "metadata".to_string()];
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::End(e)) => {
                match e.name().as_ref() {
                    b"spine" => {
                        // 写入书本
                        for ele in assets.iter() {
                            if ele.id() == "toc" {
                                continue;
                            }

                            book.add_assets(ele.clone());
                        }
                    }
                    _ => {}
                }
            }

            Ok(Event::Empty(e)) => match e.name().as_ref() {
                b"itemref" => {
                    if let Ok(href) = e.try_get_attribute("idref") {
                        if let Some(h) = href.map(|f| {
                            f.unescape_value()
                                .map_or_else(|_| String::new(), |v| v.to_string())
                        }) {
                            let xhtml = assets
                                .iter()
                                .enumerate()
                                .find(|(_index, s)| s.id() == h.as_str());
                            if let Some((index, xh)) = xhtml {
                                book.add_chapter(
                                    EpubHtml::default().with_file_name(xh.file_name()),
                                );
                                assets.remove(index);
                            }
                        }
                    }
                }
                _ => {}
            },

            _ => {
                break;
            }
        }
    }

    Ok(())
}
///
/// 获取所有文件信息
///
fn read_manifest_xml(
    reader: &mut quick_xml::reader::Reader<&[u8]>,
    _book: &mut EpubBook,
    assets: &mut Vec<EpubAssets>,
) -> IResult<()> {
    use quick_xml::events::Event;

    // 模拟 栈，记录当前的层级
    let _parent: Vec<String> = vec!["package".to_string(), "metadata".to_string()];
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::End(_e)) => {
                break;
            }
            Ok(Event::Empty(e)) => {
                match e.name().as_ref() {
                    b"item" => {
                        let mut a = EpubAssets::default();
                        if let Ok(href) = e.try_get_attribute("href") {
                            if let Some(h) = href.map(|f| {
                                f.unescape_value()
                                    .map_or_else(|_| String::new(), |v| v.to_string())
                            }) {
                                a.set_file_name(h.as_str());
                            }
                        }
                        if let Ok(href) = e.try_get_attribute("id") {
                            if let Some(h) = href.map(|f| {
                                f.unescape_value()
                                    .map_or_else(|_| String::new(), |v| v.to_string())
                            }) {
                                a.set_id(h.as_str());
                            }
                        }

                        assets.push(a);
                    }
                    _ => {
                        // invalid!("item err")
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn read_opf_xml(xml: &str, book: &mut EpubBook) -> IResult<()> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let mut reader = Reader::from_str(xml);
    let config = reader.config_mut();
    config.trim_text(true);

    let mut buf = Vec::new();
    // 模拟 栈，记录当前的层级
    let mut parent: Vec<String> = Vec::new();
    let mut assets: Vec<EpubAssets> = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => {
                break;
            }
            Err(_e) => {
                return invalid!("err");
            }
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"package" => {
                    parent.push("package".to_string());
                }
                b"metadata" => {
                    if parent.len() != 1 || parent[0] != "package" {
                        return invalid!("not valid opf metadata");
                    } else {
                        parent.push("metadata".to_string());
                    }
                    read_meta_xml(&mut reader, book)?;
                }
                b"manifest" => {
                    read_manifest_xml(&mut reader, book, &mut assets)?;
                }
                b"spine" => {
                    read_spine_xml(&mut reader, book, &mut assets)?;
                }
                _ => {}
            },
            Ok(Event::Empty(e)) => match e.name().as_ref() {
                _ => {}
            },
            Ok(Event::Text(_txt)) => {
                if !parent.is_empty() {
                    match parent[parent.len() - 1].as_str() {
                        _ => {}
                    }
                }
            }
            Ok(Event::End(e)) => {
                match e.name().as_ref() {
                    b"package" => {
                        if parent.len() == 1 && parent[0] == "package" {
                        } else {
                            // xml错误
                        }
                        // parent.remove(to_string());
                    }

                    _ => {}
                }
            }
            _ => {}
        }
        buf.clear();
    }

    let mut last_modify = None;
    let mut cover = None;
    let mut generator = None;
    {
        // 解析meta，获取部分关键数据
        for i in 0..book.meta_len() {
            if let Some(meta) = book.get_meta(i) {
                {
                    if let Some(value) = meta.get_attr("name") {
                        if value == "cover" {
                            // 可能是封面
                            if let Some(content) = meta.get_attr("content") {
                                // 对应的封面的id

                                // 查找封面
                                for ele in book.assets() {
                                    if ele.id() == content {
                                        // 封面
                                        cover = Some(ele.clone());
                                        // book.set_cover(ele.clone());
                                        break;
                                    }
                                }
                            }
                        } else if value == "generator" {
                            // 电子书创建者
                            // 可能是封面
                            if let Some(content) = meta.get_attr("content") {
                                generator = Some(content.clone());
                            }
                        }
                    }
                }
                {
                    if let Some(pro) = meta.get_attr("property") {
                        if pro == "dcterms:modified" && meta.text().is_some() {
                            last_modify = Some(meta.text().unwrap().to_string());
                        }
                    }
                }
            }
        }
    }

    if let Some(l) = last_modify {
        book.set_last_modify(l.as_str());
    }
    if let Some(co) = cover {
        book.set_cover(co);
    }
    if let Some(g) = generator {
        book.set_generator(g.as_str());
    }

    Ok(())
}
fn read_nav_point_xml(
    reader: &mut quick_xml::reader::Reader<&[u8]>,
    nav: &mut EpubNav,
) -> IResult<()> {
    use quick_xml::events::Event;

    let mut buf = Vec::new();
    // 模拟 栈，记录当前的层级
    let mut parent: Vec<String> = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => {
                break;
            }
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"navLabel" => {
                    parent.push("navLavel".to_string());
                }
                b"text" => {
                    parent.push("text".to_string());
                }
                b"content" => {
                    if let Ok(src) = e.try_get_attribute("src") {
                        if let Some(h) = src.map(|f| {
                            f.unescape_value()
                                .map_or_else(|_| String::new(), |v| v.to_string())
                        }) {
                            nav.set_file_name(&h);
                        }
                    }
                }
                b"navPoint" => {
                    // 套娃了
                    let mut n = EpubNav::default();
                    read_nav_point_xml(reader, &mut n)?;
                    nav.push(n);
                }
                _ => {}
            },
            Ok(Event::End(e)) => {
                let name = String::from_utf8(e.name().as_ref().to_vec()).map_err(IError::Utf8)?;

                if name == "navPoint" {
                    break;
                }

                if !parent.is_empty() && parent[parent.len() - 1] == name {
                    parent.remove(parent.len() - 1);
                }
            }
            Ok(Event::Text(e)) => {
                if parent[parent.len() - 1] == "text" {
                    nav.set_title(e.unescape()?.deref());
                }
            }
            Err(_e) => {
                return invalid!("err");
            }
            _ => {}
        }
    }
    Ok(())
}

fn read_nav_xml(xml: &str, book: &mut EpubBook) -> IResult<()> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let mut reader = Reader::from_str(xml);
    let config = reader.config_mut();
    config.trim_text(true);
    config.expand_empty_elements = true;

    let mut buf = Vec::new();
    // 模拟 栈，记录当前的层级
    let mut parent: Vec<String> = Vec::new();
    let mut assets: Vec<EpubNav> = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => {
                break;
            }
            Err(_e) => {
                return invalid!("err");
            }
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"ncx" => {
                    parent.push("ncx".to_string());
                }
                b"navMap" => {
                    if parent.len() != 1 || parent[0] != "ncx" {
                        return invalid!("err nav 1");
                    }
                    parent.push("navMap".to_string());
                }
                b"navPoint" => {
                    if parent.len() != 2 || parent[1] != "navMap" {
                        return invalid!("err nav 2");
                    }
                    let mut nav = EpubNav::default();
                    read_nav_point_xml(&mut reader, &mut nav)?;
                    assets.push(nav);
                }
                _ => {}
            },
            Ok(Event::End(e)) => {
                let name = String::from_utf8(e.name().as_ref().to_vec()).map_err(IError::Utf8)?;

                if name == "navMap" {
                    break;
                }

                if !parent.is_empty() && parent[parent.len() - 1] == name {
                    parent.remove(parent.len() - 1);
                }
            }
            _ => {}
        }
    }
    for ele in assets {
        book.add_nav(ele);
    }
    Ok(())
}
#[derive(Debug, Clone)]
struct EpubReader<T> {
    /// 是否懒加载
    lazy: bool,
    inner: zip::ZipArchive<T>,
}

// impl <T: Read + Seek> From<Vec<u8>> for EpubReader<T> {
//     fn from(value: Vec<u8>) -> Self {

//         EpubReader{
//             inner:
//         }

//     }
// }
impl<T: Read + Seek> EpubReader<T> {
    pub fn new(value: T) -> IResult<Self> {
        let r = zip::ZipArchive::new(value)?;
        Ok(EpubReader {
            inner: r,
            lazy: true,
        })
    }
}
impl<T: Read + Seek> EpubReaderTrait for EpubReader<T> {
    fn read(&mut self, book: &mut EpubBook) -> IResult<()> {
        let reader = &mut self.inner;

        {
            // 判断文件格式
            let content = read_from_zip!(reader, "mimetype");

            if content != "application/epub+zip" {
                return invalid!("not a epub file");
            }
        }

        {
            let content = read_from_zip!(reader, "META-INF/container.xml");

            let opf_path = get_opf_location(content.as_str());

            if let Ok(path) = opf_path {
                let opf = read_from_zip!(reader, path.as_str());

                read_opf_xml(opf.as_str(), book)?;

                {
                    // 读取导航
                    if let Some(toc) = book.assets().find(|s| s.id() == "ncx") {
                        let dir: Vec<_> = path.split("/").collect();

                        let t = format!("{}/{}", dir[0], toc.file_name());
                        if reader.by_name(t.as_str()).is_ok() {
                            let content = read_from_zip!(reader, t.as_str());

                            read_nav_xml(content.as_str(), book)?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn read_file(&mut self, file_name: &str) -> IResult<Vec<u8>> {
        let mut file = invalid!(self.inner.by_name(file_name), "not exist");
        let mut content = Vec::new();
        invalid!(file.read_to_end(&mut content), "read err");
        Ok(content)
    }

    fn read_string(&mut self, file_name: &str) -> IResult<String> {
        let mut file = invalid!(self.inner.by_name(file_name), "not exist");
        let mut content = String::new();
        invalid!(file.read_to_string(&mut content), "read err");
        Ok(content)
    }
}

///
/// 从内存读取epub
///
pub fn read_from_vec(data: Vec<u8>) -> IResult<EpubBook> {
    read_from_reader(std::io::Cursor::new(data))
}

///
/// 从文件读取epub
///
pub fn read_from_file(file: &str) -> IResult<EpubBook> {
    read_from_reader(std::fs::File::open(file)?)
}

///
/// 从任意reader读取epub
///
pub fn read_from_reader<T: Read + Seek + 'static>(value: T) -> IResult<EpubBook> {
    let reader = EpubReader::new(value)?;
    let mut book = EpubBook::default();
    let re: std::rc::Rc<std::cell::RefCell<Box<dyn EpubReaderTrait>>> =
        std::rc::Rc::new(std::cell::RefCell::new(Box::new(reader)));
    book.set_reader(std::rc::Rc::clone(&re));

    (*re.borrow_mut()).read(&mut book)?;
    Ok(book)
}

/// 判断是否是epub文件
pub fn is_epub<T: Read>(value: &mut T) -> IResult<bool> {
    let mut v = Vec::new();
    value.take(4).read_to_end(&mut v)?;
    let magic: [u8; 4] = [0x50, 0x4B, 0x03, 0x04];
    Ok(v == magic)
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    use super::{is_epub, read_nav_xml};

    #[test]
    fn test_is_epub() {
        let mut magic: [u8; 4] = [0x50, 0x4B, 0x03, 0x04];

        assert_eq!(true, is_epub(&mut std::io::Cursor::new(magic)).unwrap());

        magic = [0x50, 0x4B, 0x03, 0x03];
        assert_eq!(false, is_epub(&mut std::io::Cursor::new(magic)).unwrap());

        let n_magic: [u8; 3] = [0x50, 0x4B, 0x03];
        assert_eq!(false, is_epub(&mut std::io::Cursor::new(n_magic)).unwrap());

        let n2_magic: [u8; 5] = [0x50, 0x4B, 0x03, 0x04, 0x05];
        assert_eq!(true, is_epub(&mut std::io::Cursor::new(n2_magic)).unwrap());

        let empty = [0u8; 32];
        assert_eq!(false, is_epub(&mut std::io::Cursor::new(empty)).unwrap());

        let null = [0u8; 0];
        assert_eq!(false, is_epub(&mut std::io::Cursor::new(null)).unwrap());
    }

    #[test]
    fn test_reader() {
        #[inline]
        fn create_book() -> EpubBuilder {
            EpubBuilder::new()
                .with_title("书名")
                .with_format("f书名")
                .with_creator("作者")
                .with_date("2024-03-14")
                .with_description("一本好书")
                .with_identifier("isbn")
                .with_publisher("行星出版社")
                .with_last_modify("last_modify")
                .add_assets("style.css", "ok".as_bytes().to_vec())
                .add_chapter(
                    EpubHtml::default()
                        .with_title("ok")
                        .with_file_name("0.xhtml")
                        .with_data("html".as_bytes().to_vec()),
                )
        }

        let data = create_book().mem().unwrap();

        let book = read_from_vec(data);
        let mut nb = book.unwrap();
        println!("\n{}", nb);
        let b = create_book().book().unwrap();
        assert_eq!(b.title(), nb.title());
        assert_eq!(b.date(), nb.date());
        assert_eq!(b.creator(), nb.creator());
        assert_eq!(b.description(), nb.description());
        assert_eq!(b.contributor(), nb.contributor());
        assert_eq!(b.format(), nb.format());
        assert_eq!(b.subject(), nb.subject());
        assert_eq!(b.publisher(), nb.publisher());
        assert_eq!(b.identifier(), nb.identifier());
        assert_eq!(b.last_modify(), nb.last_modify());
        // 多出来一个 导航 toc.ncx
        assert_eq!(b.assets().len() + 1, nb.assets().len());
        // 多出来的一个是导航 nav.xhtml
        assert_eq!(b.chapters().len() + 1, nb.chapters().len());

        // 读取html
        let chapter = nb.get_chapter("0.xhtml");
        // assert_ne!(None, chapter);
        assert!(chapter.is_some());
        let c = &mut chapter.unwrap();

        let data = c.data();

        assert!(data.is_some());
        let d = String::from_utf8(data.unwrap().to_vec()).unwrap();
        println!("d [{}]", d);

        assert_eq!(
            r#"
    <h1 style="texe-align: center">ok</h1>
html
  "#,
            d
        );

        for a in nb.assets_mut() {
            println!(
                "ass=[{}]",
                String::from_utf8(a.data().unwrap().to_vec()).unwrap()
            );
        }

        println!("nav ={:?}", nb.nav());
        assert_eq!(1, nb.nav().len());
        assert_eq!("0.xhtml", nb.nav()[0].file_name());

        // println!("{}",c.data().map(|f|String::from_utf8(f.to_vec())).unwrap().unwrap());
    }

    #[test]
    fn test_read_empty_toc() {
        let xml = r#"<?xml version='1.0' encoding='utf-8'?><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><head><meta content="1394" name="dtb:uid"/><meta content="0" name="dtb:depth"/><meta content="0" name="dtb:totalPageCount"/><meta content="0" name="dtb:maxPageNumber"/></head><docTitle><text>book_title</text></docTitle><navMap><navPoint id="0-0"><navLabel><text></text></navLabel><content src="0.xhtml"></content><navPoint id="0-0"><navLabel><text></text></navLabel><content src="0.xhtml"></content></navPoint></navPoint></navMap></ncx>"#;
        let mut book = EpubBook::default();
        let _ = read_nav_xml(xml, &mut book).unwrap();

        let n = book.nav();

        assert_eq!(1, n.len());
        assert_eq!("", n[0].title());
        assert_eq!(1, n[0].child().len());
        assert_eq!("", n[0].child()[0].title());
    }
}
