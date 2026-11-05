use quick_xml::events::BytesStart;
use quick_xml::XmlVersion;
use std::borrow::Cow;
use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::{
    io::{Read, Seek},
    ops::Deref,
};

use super::core::EpubReaderTrait;
use crate::prelude::*;
macro_rules! invalid {
    ($x:tt) => {
        Err(IError::InvalidArchive(Cow::from($x)))
    };
    ($x:expr,$y:expr) => {
        $x.or(Err(IError::InvalidArchive(Cow::from($y))))?
    };
}

macro_rules! read_from_zip {
    ($m:ident,$x:expr) => {{
        // 读取 container.xml
        let mut file = invalid!($m.by_name($x), format!("{} not exist", $x));
        let mut content = String::new();
        invalid!(file.read_to_string(&mut content), "read err");
        content
    }};
}

/// 取限定名的本地部分（`opf:item` -> `item`、`ncx:navMap` -> `navMap`）
///
/// 生成器对 OPF / NCX / nav 使用的命名空间前缀并不固定，比较标签名时必须先去前缀，
/// 否则带前缀的包文档会一个分支都进不去（jlreq 整本解析为空）。
#[inline]
pub(crate) fn bare_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

/// 按 `/` 与 `\` 同时切分，用于解析 zip 内部（EPUB 内）的路径
#[inline]
fn zip_split(path: &str) -> impl Iterator<Item = &str> {
    path.split(['/', '\\'])
}

/// 把 `base` 目录（以 `/` 结尾，可为空）与相对引用拼成 zip 内的条目名，并归一化 `.` 与 `..`
///
/// EPUB 里的 href 允许出现 `../images/1.jpg` 这类回退写法，直接字符串拼接得到的结果
/// 与 zip 条目名对不上，目录与章节就匹配不到。
pub(crate) fn zip_join(base: &str, href: &str) -> String {
    let mut levels: Vec<&str> = Vec::new();
    for ele in zip_split(base).chain(zip_split(href)) {
        match ele {
            "" | "." => {}
            ".." => {
                levels.pop();
            }
            _ => levels.push(ele),
        }
    }
    levels.join("/")
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
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) => {
                if bare_name(e.name().as_ref()) == "rootfile" {
                    if let Ok(path) = e.try_get_attribute("full-path") {
                        return match path.map(|f| {
                            f.normalized_value(XmlVersion::Implicit1_0)
                                .map_or_else(|_| String::new(), |v| v.to_string())
                        }) {
                            Some(v) => Ok(v),
                            None => Err(IError::InvalidArchive(Cow::from("has no opf"))),
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

    for a in xml.attributes().flatten() {
        meta.push_attr(a.key.0, a.value.as_ref());
    }
    Ok(meta)
}

fn read_meta_xml(
    reader: &mut quick_xml::reader::Reader<&[u8]>,
    book: &mut EpubBook,
) -> IResult<()> {
    use quick_xml::events::Event;

    let mut reading_text = false;
    let mut text = String::new();
    let t_s = reader.config().trim_text_start;
    let t_e = reader.config().trim_text_end;
    reader.config_mut().trim_text(false);

    // 可能有多个时间，所以需要缓存，然后再分别处理
    let mut date_buf: Vec<(Option<String>, Option<String>)> = Vec::new();

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
                let name =
                    bare_name(e.name().as_ref()).to_string();

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
                } else if name == "date" {
                    if let Some(event) = e
                        .attributes()
                        .flatten()
                        .find(|f| bare_name(f.key.as_ref()) == "event")
                        .map(|f| {
                            f.normalized_value(XmlVersion::Implicit1_0)
                                .map_or_else(|_| String::new(), |v| v.to_string())
                        })
                    {
                        date_buf.push((Some(event), None));
                    } else {
                        date_buf.push((None, None));
                    }
                } else if parent.len() != 2 || parent[1] != "metadata" {
                    return invalid!("not valid opf identifier");
                } else {
                    parent.push(name);
                }
            }
            Ok(Event::Empty(e)) => {
                if bare_name(e.name().as_ref()) == "meta" {
                    if parent.len() != 2 || parent[1] != "metadata" {
                        return invalid!("not valid opf meta empty");
                    } else {
                        let meta = create_meta(&e);
                        if let Ok(m) = meta {
                            book.add_meta(m);
                        }
                    }
                }
            }
            Ok(Event::GeneralRef(e)) => {
                if reading_text {
                    let t = e.into_inner();
                    if t == "amp" {
                        text.push('&');
                    } else if t == "lt" {
                        text.push('<');
                    } else if t == "gt" {
                        text.push('>');
                    } else if t == "apos" {
                        text.push('\'');
                    } else if t == "quot" {
                        text.push('"');
                    }
                }
            }
            Ok(Event::Text(txt)) => {
                if !parent.is_empty() {
                    reading_text = true;
                    text.push_str(txt.into_inner().as_ref());
                }
            }
            Ok(Event::End(e)) => {
                let name =
                    bare_name(e.name().as_ref()).to_string();
                reading_text = false;

                match name.as_str() {
                    "meta" => {
                        if let Some(m) = book.get_meta_mut(book.meta_len() - 1) {
                            m.set_text(text.trim());
                        }
                    }
                    "identifier" => {
                        book.set_identifier(text.trim());
                    }
                    "title" => {
                        book.set_title(text.trim());
                    }
                    "creator" => {
                        book.set_creator(text.trim());
                    }
                    "description" => {
                        book.set_description(text.trim());
                    }
                    "format" => {
                        book.set_format(text.trim());
                    }
                    "publisher" => {
                        book.set_publisher(text.trim());
                    }
                    "subject" => {
                        book.set_subject(text.trim());
                    }
                    "contributor" => {
                        book.set_contributor(text.trim());
                    }
                    "date" => {
                        if let Some(last_mut) = date_buf.last_mut() {
                            last_mut.1 = Some(text.trim().to_string());
                        }
                    }
                    _ => {}
                }
                text.clear();
                if name == "metadata" {
                    if parent.len() != 2 || parent[0] != "package" {
                        return invalid!("not valid opf metadata end");
                    }
                    break;
                }

                if !parent.is_empty() && parent[parent.len() - 1] == name {
                    parent.remove(parent.len() - 1);
                }
            }
            _ => {}
        }
    }

    // 处理时间
    for (event, value) in &date_buf {
        if let Some(event) = event {
            if event == "publication" {
                if let Some(value) = value {
                    book.set_date(value);
                }
            } else if event == "modification" {
                if let Some(value) = value {
                    book.set_last_modify(value);
                }
            }
        } else if let Some(value) = value {
            // 没有event的日期
            book.set_date(value);
        }
    }

    reader.config_mut().trim_text_start = t_s;
    reader.config_mut().trim_text_end = t_e;
    Ok(())
}

/// 从xml中获取封面页
fn read_guide_xml(
    reader: &mut quick_xml::reader::Reader<&[u8]>,
    book: &mut EpubBook,
) -> IResult<()> {
    use quick_xml::events::Event;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                if bare_name(e.name().as_ref()) == "reference" {
                    // <reference href="cover.xhtml" title="cover" type="cover"/>
                    // 读取封面页的时候，不一定已经获取到了章节
                    if e
                        .attributes()
                        .flatten()
                        .find(|f| bare_name(f.key.as_ref()) == "type")
                        .map(|f| {
                            f.normalized_value(XmlVersion::Implicit1_0)
                                .map_or_else(|_| String::new(), |v| v.to_string())
                        })
                        .filter(|f| f == "cover")
                        .is_some()
                    {
                        if let Some(href) = e
                            .attributes()
                            .flatten()
                            .find(|f| bare_name(f.key.as_ref()) == "href")
                            .map(|f| {
                                f.normalized_value(XmlVersion::Implicit1_0)
                                    .map_or_else(|_| String::new(), |v| v.to_string())
                            })
                        {
                            book.cover_chapter = Some(EpubHtml::default().with_file_name(href));
                        }
                    }
                }
            }
            Ok(Event::End(e)) => {
                if bare_name(e.name().as_ref()) == "guide" {
                    break;
                }
            }
            Ok(Event::Eof) => {
                break;
            }
            Err(_e) => {
                return invalid!("err");
            }
            // 注释、文本等不能终止解析
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}

/// 判断 manifest 项是不是目录文档
///
/// 目录文档不参与成章，也不作为普通章节资源暴露
#[inline]
fn is_toc_item(item: &EpubAssets, spine_toc: &Option<String>) -> bool {
    item.has_property("nav")
        || item.media_type == "application/x-dtbncx+xml"
        || spine_toc.as_deref().map(|f| f == item.id()).unwrap_or(false)
        || item.id() == "toc"
        || item.id() == "ncx"
}

/// 判断 manifest 项能不能当成正文章节
///
/// xhtml/html 之外，`image/svg+xml` 在 EPUB3 里也是合法的文档项（固定版式书页），
/// 只有真正的二进制资源（图片、字体、css）才不该成为章节
#[inline]
fn is_html_item(item: &EpubAssets) -> bool {
    item.media_type.contains("html")
        || item.media_type == "image/svg+xml"
        || item.file_name().ends_with(".xhtml")
        || item.file_name().ends_with(".html")
        || item.file_name().ends_with(".svg")
}

///
/// spine：章节顺序、`spine@page-progression-direction`、`spine@toc`
///
/// 非线性项（`itemref@linear="no"`）、目录文档本身、以及图片/SVG 这类非 html 项都不会成为章节；
/// 非 html 项声明了 `item@fallback` 时，改用 fallback 指向的文档成章，图片本体继续留在资源里。
/// 以上修正只作用于 EPUB3，EPUB2 完全沿用原有行为，避免影响既有解析结果。
///
fn read_spine_xml(
    reader: &mut quick_xml::reader::Reader<&[u8]>,
    book: &mut EpubBook,
    assets: &mut Vec<EpubAssets>,
    spine_toc: &Option<String>,
) -> IResult<()> {
    use quick_xml::events::Event;

    // metadata 先于 spine 解析，此时 version 已经拿到
    let epub3 = book.version().starts_with('3');
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let tag = bare_name(e.name().as_ref()).to_string();
                if tag == "itemref" {
                    let mut idref = String::new();
                    let mut linear = String::new();
                    let mut properties = String::new();
                    for attr in e.attributes().flatten() {
                        let key = bare_name(attr.key.as_ref()).to_string();
                        let value = attr
                            .normalized_value(XmlVersion::Implicit1_0)
                            .map_or_else(|_| String::new(), |f| f.to_string());
                        if key == "idref" {
                            idref = value;
                        } else if key == "linear" {
                            linear = value;
                        } else if key == "properties" {
                            properties = value;
                        }
                    }
                    if idref.is_empty() {
                        continue;
                    }
                    let index = match assets.iter().position(|s| s.id() == idref.as_str()) {
                        Some(i) => i,
                        None => continue,
                    };
                    if !epub3 {
                        // EPUB2：沿用既有行为，spine 引用什么就成章
                        let mut chap =
                            EpubHtml::default().with_file_name(assets[index].file_name());
                        chap.set_properties(properties);
                        book.add_chapter(chap);
                        if !assets[index].id().eq_ignore_ascii_case("toc")
                            && assets[index].file_name().contains(".xhtml")
                        {
                            assets.remove(index);
                        }
                        continue;
                    }
                    // EPUB3：目录文档本身、非线性项不参与成章
                    if linear == "no" || is_toc_item(&assets[index], spine_toc) {
                        continue;
                    }
                    // 非 html 的 spine 项（漫画、SVG）需要回落到 fallback 文档
                    let mut target = assets[index].file_name().to_string();
                    if !is_html_item(&assets[index]) {
                        let fallback = assets[index].fallback.clone();
                        if fallback.is_empty() {
                            continue;
                        }
                        match assets.iter().find(|s| s.id() == fallback.as_str()) {
                            Some(fb) => target = fb.file_name().to_string(),
                            None => continue,
                        }
                    }
                    // 同一文件被多次引用时只保留一章
                    if book.get_chapter(target.as_str()).is_some() {
                        continue;
                    }
                    let mut chap = EpubHtml::default().with_file_name(target.as_str());
                    chap.set_properties(properties);
                    book.add_chapter(chap);
                    // 成章的文件从资源表里摘除；靠 fallback 成章时，本体（图片）仍留在资源里
                    if assets[index].file_name() == target.as_str() {
                        assets.remove(index);
                    }
                }
            }
            Ok(Event::End(e)) => {
                if bare_name(e.name().as_ref()) == "spine" {
                    break;
                }
            }
            Ok(Event::Eof) => {
                break;
            }
            Err(_e) => {
                return invalid!("err");
            }
            // spine 里的注释、文本等不能终止整段解析
            _ => {}
        }
        buf.clear();
    }

    Ok(())
}
///
/// 获取所有文件信息
///
fn read_manifest_xml(
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
                // 注释、文本等都不能终止解析，只有 </manifest> 才退出
                if bare_name(e.name().as_ref()) == "manifest" {
                    break;
                }
            }
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) => {
                match bare_name(e.name().as_ref()) {
                    "item" => {
                        let mut a = EpubAssets::default();
                        let mut is_cover = false;
                        for attr in e.attributes().flatten() {
                            match bare_name(attr.key.as_ref()) {
                                "href" => {
                                    let h = attr
                                        .normalized_value(XmlVersion::Implicit1_0)
                                        .unwrap_or_default()
                                        .to_string();
                                    a.set_file_name(h.as_str());
                                }
                                "id" => {
                                    let h = attr
                                        .normalized_value(XmlVersion::Implicit1_0)
                                        .unwrap_or_default()
                                        .to_string();
                                    if h.eq_ignore_ascii_case("cover") {
                                        is_cover = true;
                                    }
                                    a.set_id(h.as_str());
                                }
                                "media-type" => {
                                    let h = attr
                                        .normalized_value(XmlVersion::Implicit1_0)
                                        .unwrap_or_default()
                                        .to_string();
                                    a.media_type = h;
                                }
                                "properties" => {
                                    let h = attr
                                        .normalized_value(XmlVersion::Implicit1_0)
                                        .unwrap_or_default()
                                        .to_string();
                                    a.set_properties(h.as_str());
                                }
                                "fallback" => {
                                    let h = attr
                                        .normalized_value(XmlVersion::Implicit1_0)
                                        .unwrap_or_default()
                                        .to_string();
                                    a.set_fallback(h.as_str());
                                }
                                _ => {}
                            }
                        }

                        if a.has_property("cover-image") {
                            is_cover = true;
                        }
                        if is_cover {
                            let href = a.file_name();
                            if href.ends_with(".xhtml") || href.ends_with(".html") {
                                book.cover_chapter = Some(EpubHtml::default().with_file_name(href));
                            } else if a.media_type.starts_with("image/") {
                                book.set_cover(a.clone());
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
    // spine@toc 指向的 manifest 项 id
    let mut spine_toc: Option<String> = None;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => {
                break;
            }
            Err(_e) => {
                return invalid!("err");
            }
            Ok(Event::Start(e)) => match bare_name(e.name().as_ref()) {
                "package" => {
                    parent.push("package".to_string());
                    for attr in e.attributes().flatten() {
                        if bare_name(attr.key.as_ref()) == "version" {
                            let ver = attr
                                .normalized_value(XmlVersion::Implicit1_0)?
                                .trim()
                                .to_string();
                            if !ver.is_empty() {
                                book.set_version(&ver);
                            } else {
                                book.set_version("2.0");
                            }
                        }
                    }
                }
                "metadata" => {
                    if parent.len() != 1 || parent[0] != "package" {
                        return invalid!("not valid opf metadata");
                    } else {
                        parent.push("metadata".to_string());
                    }
                    read_meta_xml(&mut reader, book)?;
                }
                "manifest" => {
                    read_manifest_xml(&mut reader, book, &mut assets)?;
                }
                "spine" => {
                    // spine 自身的属性在这里就读掉，read_spine_xml 拿到时已经在标签内部
                    for attr in e.attributes().flatten() {
                        let key = bare_name(attr.key.as_ref()).to_string();
                        let value = attr
                            .normalized_value(XmlVersion::Implicit1_0)
                            .map_or_else(|_| String::new(), |f| f.to_string());
                        if key == "page-progression-direction" {
                            if !value.is_empty() {
                                book.set_direction(Direction::from(value));
                            }
                        } else if key == "toc" && !value.is_empty() {
                            spine_toc = Some(value);
                        }
                    }
                    read_spine_xml(&mut reader, book, &mut assets, &spine_toc)?;
                }
                "guide" => {
                    read_guide_xml(&mut reader, book)?;
                }
                _ => {}
            },
            Ok(Event::Empty(e)) => {
                // `<spine/>`：没有 itemref，但属性仍然要读
                if bare_name(e.name().as_ref()) == "spine" {
                    for attr in e.attributes().flatten() {
                        let key = bare_name(attr.key.as_ref()).to_string();
                        let value = attr
                            .normalized_value(XmlVersion::Implicit1_0)
                            .map_or_else(|_| String::new(), |f| f.to_string());
                        if key == "page-progression-direction" {
                            if !value.is_empty() {
                                book.set_direction(Direction::from(value));
                            }
                        } else if key == "toc" && !value.is_empty() {
                            spine_toc = Some(value);
                        }
                    }
                }
            }
            Ok(Event::Text(_txt)) => {}
            Ok(Event::End(e)) => {
                if bare_name(e.name().as_ref()) == "package" {
                    if parent.len() == 1 && parent[0] == "package" {
                    } else {
                        // xml错误
                    }
                    // parent.remove(to_string());
                }
            }
            _ => {}
        }
        buf.clear();
    }

    // 目录文档先定下来，再落资源表
    let epub3 = book.version().starts_with('3');
    if let Some(i) = pick_toc_index(assets.as_slice(), spine_toc.as_ref(), epub3) {
        let mut toc = assets.remove(i);
        if toc.prefix.is_empty() {
            toc.prefix = book.prefix().to_string();
        }
        book.set_toc(toc);
    }

    // 资源表在这里统一落库，不再依赖 </spine> 是否被读到
    for ele in assets.drain(..) {
        if ele.media_type == "application/x-dtbncx+xml" {
            // toc.ncx 不作为调用方可见的普通资源
            continue;
        }
        book.add_assets(ele);
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

///
/// 选出目录文档在 manifest 项里的位置
///
/// EPUB3 以 `item@properties="nav"` 为准，EPUB2 以 `spine@toc` 指向的 NCX 为准，
/// id 字面量判据只保留为兜底（生成器取名是任意的，实测 40 本里有 15 本根本不存在 id 为 toc/ncx 的项）
///
fn pick_toc_index(assets: &[EpubAssets], spine_toc: Option<&String>, epub3: bool) -> Option<usize> {
    // 按 properties="nav" 定位的 EPUB3 目录
    let by_nav = assets.iter().position(|f| f.has_property("nav"));
    // spine@toc 指向的项（EPUB2 目录，EPUB3 里也常作为兼容目录保留）
    let by_spine_toc =
        spine_toc.and_then(|id| assets.iter().position(|f| f.id() == id.as_str()));
    // media-type 声明的 NCX
    let by_ncx_type = assets
        .iter()
        .position(|f| f.media_type == "application/x-dtbncx+xml");
    // 历史判据：id 恰好叫 toc 或 ncx
    let by_id = assets
        .iter()
        .position(|f| f.id() == "toc" || f.id() == "ncx");
    if epub3 {
        by_nav.or(by_spine_toc).or(by_ncx_type).or(by_id)
    } else {
        by_id.or(by_spine_toc).or(by_ncx_type).or(by_nav)
    }
}

/// NCX 的单个 navPoint
///
/// `root_path` 是 NCX 文件自身所在目录（相对包文档），`content@src` 相对它解析
fn read_nav_point_xml(
    reader: &mut quick_xml::reader::Reader<&[u8]>,
    nav: &mut EpubNav,
    root_path: &str,
) -> IResult<()> {
    use quick_xml::events::Event;

    let mut buf = Vec::new();
    // 模拟 栈，记录当前的层级
    let mut parent: Vec<String> = Vec::new();
    let mut title = String::new();

    // 因为文本会被拆分，所以不能trim，否则可能导致在中间的空格被trim掉
    let t_s = reader.config().trim_text_start;
    let t_e = reader.config().trim_text_end;
    reader.config_mut().trim_text(false);

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => {
                break;
            }
            Ok(Event::Start(e)) => match bare_name(e.name().as_ref()) {
                "navLabel" => {
                    parent.push("navLavel".to_string());
                }
                "text" => {
                    parent.push("text".to_string());
                }
                "content" => {
                    if let Some(src) = e
                        .attributes()
                        .flatten()
                        .find(|f| bare_name(f.key.as_ref()) == "src")
                        .map(|f| {
                            f.normalized_value(XmlVersion::Implicit1_0)
                                .map_or_else(|_| String::new(), |v| v.to_string())
                        })
                    {
                        // src 可能带有 `../`，需要归一化后才能和章节文件名对上
                        nav.set_file_name(zip_join(root_path, src.as_str()));
                    }
                }
                "navPoint" => {
                    // 套娃了
                    let mut n = EpubNav::default();
                    read_nav_point_xml(reader, &mut n, root_path)?;
                    nav.push(n);
                }
                _ => {}
            },
            Ok(Event::End(e)) => {
                let name =
                    bare_name(e.name().as_ref()).to_string();

                if name == "navPoint" {
                    break;
                }
                if parent.last().map(|f| f == "text").unwrap_or(false) {
                    nav.set_title(title.trim());
                    title.clear();
                }
                if !parent.is_empty() && parent[parent.len() - 1] == name {
                    parent.remove(parent.len() - 1);
                }
            }
            Ok(Event::Text(e)) => {
                if parent.last().map(|f| f == "text").unwrap_or(false) {
                    // quick_xml 0.38.0 之后，对于被转义后的xml文本，会把文本拆成多段
                    title.push_str(e.into_inner().as_ref());
                }
            }
            Ok(Event::GeneralRef(e)) => {
                // 当文本中出现 &amp;之类的转义时，amp会被放到这里
                if parent.last().map(|f| f == "text").unwrap_or(false) {
                    let t = e.into_inner();
                    if t == "amp" {
                        title.push('&');
                    } else if t == "lt" {
                        title.push('<');
                    } else if t == "gt" {
                        title.push('>');
                    } else if t == "apos" {
                        title.push('\'');
                    } else if t == "quot" {
                        title.push('"');
                    }
                }
            }
            Err(_e) => {
                return invalid!("err");
            }
            _ => {}
        }
    }
    reader.config_mut().trim_text_start = t_s;
    reader.config_mut().trim_text_end = t_e;
    Ok(())
}

/// NCX（EPUB2 目录，EPUB3 也常作为兼容目录保留）
///
/// `root_path` 是 NCX 文件自身所在目录（相对包文档），例如 `Navigation/`
fn read_nav_xml(xml: &str, root_path: String, book: &mut EpubBook) -> IResult<()> {
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
            Ok(Event::Start(e)) => match bare_name(e.name().as_ref()) {
                "ncx" => {
                    parent.push("ncx".to_string());
                }
                "navMap" => {
                    if parent.len() != 1 || parent[0] != "ncx" {
                        return invalid!("err nav 1");
                    }
                    parent.push("navMap".to_string());
                }
                "navPoint" => {
                    if parent.len() != 2 || parent[1] != "navMap" {
                        return invalid!("err nav 2");
                    }
                    let mut nav = EpubNav::default();
                    read_nav_point_xml(&mut reader, &mut nav, root_path.as_str())?;
                    assets.push(nav);
                }
                _ => {}
            },
            Ok(Event::End(e)) => {
                let name =
                    bare_name(e.name().as_ref()).to_string();

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

/// EPUB3 的 nav.xhtml 目录
///
/// `root_path` 是 nav 文档自身所在目录（相对包文档），`<a href>` 相对它解析
fn read_nav_xhtml(xhtml: &str, root_path: String, book: &mut EpubBook) -> IResult<()> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let mut reader = Reader::from_str(xhtml);
    reader.config_mut().trim_text(true);
    // 打开的 `<ol>` 层级，栈顶就是当前正在填充的目录层
    let mut ol_stack: Vec<Vec<EpubNav>> = Vec::new();
    // 打开的 `<li>`，栈顶是当前尚未闭合的条目，子目录挂在它下面
    let mut li_stack: Vec<EpubNav> = Vec::new();
    let mut items: Vec<EpubNav> = Vec::new();
    let mut in_toc_nav = false;
    let mut label = String::new();
    let mut in_label = false;
    let mut anchor = String::new();
    let mut in_anchor = false;
    loop {
        match reader.read_event()? {
            Event::Start(e) => match bare_name(e.name().as_ref()) {
                "nav" => {
                    if has_epub_type(&e, "toc") {
                        in_toc_nav = true;
                    }
                }
                "ol" if in_toc_nav => ol_stack.push(Vec::new()),
                "li" if in_toc_nav => li_stack.push(EpubNav::default()),
                "a" if in_toc_nav => {
                    in_anchor = true;
                    anchor.clear();
                    if li_stack.is_empty() {
                        // `<ol>` 下直接跟 `<a>` 的不规范写法，也要能收下这一条
                        li_stack.push(EpubNav::default());
                    }
                    for attr in e.attributes().flatten() {
                        if bare_name(attr.key.as_ref()) == "href" {
                            let raw = attr
                                .normalized_value(XmlVersion::Implicit1_0)
                                .map_or_else(|_| String::new(), |f| f.to_string());
                            // href 相对 nav 文档，可能带 `../`，归一化后才能和章节文件名对上
                            if let Some(li) = li_stack.last_mut() {
                                li.set_file_name(zip_join(root_path.as_str(), raw.as_str()));
                            }
                        }
                    }
                }
                "span" if in_toc_nav => {
                    for attr in e.attributes().flatten() {
                        if bare_name(attr.key.as_ref()) == "class"
                            && attr.value.split_whitespace().any(|f| f == "toc-label")
                        {
                            in_label = true;
                            label.clear();
                        }
                    }
                }
                _ => (),
            },
            Event::Text(e) => {
                let text = e.into_inner();
                if in_label {
                    label.push_str(&text);
                } else if in_anchor {
                    anchor.push_str(&text);
                }
            }
            Event::End(e) => {
                // 只有 toc 块内部的事件才能驱动状态机，正文和其他 `<nav>` 不能
                if !in_toc_nav {
                    continue;
                }
                match bare_name(e.name().as_ref()) {
                    "nav" => in_toc_nav = false,
                    "a" => {
                        in_anchor = false;
                        // `toc-label` 优先，缺失时才用 anchor 文本作为标题
                        if !anchor.trim().is_empty() {
                            if let Some(li) = li_stack.last_mut() {
                                if li.title().is_empty() {
                                    li.set_title(anchor.trim());
                                }
                            }
                        }
                        anchor.clear();
                    }
                    "span" => {
                        if in_label {
                            if let Some(li) = li_stack.last_mut() {
                                li.set_title(label.trim());
                            }
                            label.clear();
                            in_label = false;
                        }
                    }
                    "li" => {
                        if let Some(item) = li_stack.pop() {
                            match ol_stack.last_mut() {
                                Some(layer) => layer.push(item),
                                None => items.push(item),
                            }
                        }
                    }
                    "ol" => {
                        if let Some(done) = ol_stack.pop() {
                            if let Some(parent) = li_stack.last_mut() {
                                // 子目录在父 `<li>` 闭合之前结束（分级目录的常规写法）
                                for ele in done {
                                    parent.push(ele);
                                }
                            } else if let Some(layer) = ol_stack.last_mut() {
                                // 父 `<li>` 已经闭合的非常规写法：并进上一层，不崩溃也不丢条目
                                for ele in done {
                                    layer.push(ele);
                                }
                            } else {
                                items = done;
                            }
                        }
                    }
                    _ => (),
                }
            }
            Event::Eof => break,
            _ => (),
        }
    }
    for nav in items {
        book.add_nav(nav);
    }
    Ok(())
}

fn has_epub_type(e: &BytesStart, value: &str) -> bool {
    e.attributes().any(|a| {
        if let Ok(attr) = a {
            // `epub:type` 的前缀由生成器决定，比较时要去掉；值本身可以是空格分隔的多个类型
            bare_name(attr.key.as_ref()) == "type"
                && attr.value.split_whitespace().any(|f| f == value)
        } else {
            false
        }
    })
}

#[derive(Debug, Clone)]
struct EpubReader<T: Read + Seek> {
    inner: zip::ZipArchive<T>,
}

impl<T: Read + Seek> Drop for EpubReader<T> {
    fn drop(&mut self) {}
}

impl<T: Read + Seek> EpubReader<T> {
    pub fn new(value: T) -> IResult<Self> {
        let r = zip::ZipArchive::new(value)?;
        Ok(EpubReader { inner: r })
    }
}

impl<T: Read + Seek + Sync + Send> EpubReaderTrait for EpubReader<T> {
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
                // zip 内的分隔符恒为 `/`，包文档所在目录就是所有章节与资源的相对基准
                book.prefix = match path.rfind('/') {
                    Some(i) => path[..i + 1].to_string(),
                    None => String::new(),
                };
                let opf = read_from_zip!(reader, path.as_str());
                read_opf_xml(opf.as_str(), book)?;

                {
                    // 读取导航
                    let mut toc_name = String::new();
                    let mut toc_type = String::new();
                    if let Some(toc) = book.toc() {
                        toc_name = toc.file_name().to_string();
                        toc_type = toc.media_type.clone();
                    }
                    if !toc_name.is_empty() {
                        // 目录文档在 zip 内的真实条目名
                        let mut t = zip_join(book.prefix.as_str(), toc_name.as_str());
                        if reader.by_name(t.as_str()).is_err() {
                            // 前缀拼不上时退回原样，兼容包文档与实际存放位置不一致的书
                            t = toc_name.clone();
                        }
                        if reader.by_name(t.as_str()).is_ok() {
                            let content = read_from_zip!(reader, t.as_str());
                            // 目录内部的引用相对的是目录文档自身所在目录
                            let root = toc_name
                                [..toc_name.rfind('/').map(|f| f + 1).unwrap_or(0)]
                                .to_string();
                            // 按类型分派解析器：nav.xhtml 与 toc.ncx 的结构完全不同，
                            // 喂错解析器只会扫到结尾返回空目录
                            if toc_type.contains("html")
                                || toc_name.ends_with(".xhtml")
                                || toc_name.ends_with(".html")
                            {
                                read_nav_xhtml(content.as_str(), root, book)?;
                            } else {
                                read_nav_xml(content.as_str(), root, book)?;
                            }
                        }
                    }
                }
            }
        }
        // 从封面页获取封面
        {
            if let Some(co) = &mut book.cover_chapter {
                co.read_data(self);
                // let _ = co.data_mut().unwrap(); // 死锁，因为是不可重入锁
            }
            if let Some(co) = book
                .cover_chapter
                .as_ref()
                .filter(|_| book.cover().is_none())
                .as_mut()
                .and_then(|co| co.data().map(|s| (s, co.file_name())))
            {
                // 查找第一个img标签，然后找到对应的文件
                if let Some(cover) = get_img_src(co.0, "img", "src")
                    .or_else(|| get_img_src(co.0, "image", "xlink:href"))
                    .or_else(|| get_img_src(co.0, "image", "href"))
                    .and_then(|f| String::from_utf8(f).ok())
                    .map(|src| {
                        // 封面页里的图片引用相对封面页所在目录
                        let dir = co.1[..co.1.rfind('/').map(|f| f + 1).unwrap_or(0)].to_string();
                        zip_join(dir.as_str(), src.as_str())
                    })
                    .and_then(|img| book.assets().find(|f| f.file_name() == img.as_str()))
                {
                    book.set_cover((*cover).clone());
                }
            }
        }
        book.update_chapter();
        book.update_assets();

        Ok(())
    }

    fn read_file(&mut self, file_name: &str) -> IResult<Vec<u8>> {
        let mut file = self
            .inner
            .by_name(file_name)
            .or(Err(IError::FileNotFound))?;
        let mut content = Vec::new();
        invalid!(file.read_to_end(&mut content), "read err");
        Ok(content)
    }

    fn read_string(&mut self, file_name: &str) -> IResult<String> {
        let mut file = self
            .inner
            .by_name(file_name)
            .or(Err(IError::FileNotFound))?;
        let mut content = String::new();
        invalid!(file.read_to_string(&mut content), "read err");
        Ok(content)
    }

    fn read_to_path(&mut self, file_name: &str, file_path: &str) -> IResult<()> {
        let mut file = self
            .inner
            .by_name(file_name)
            .or(Err(IError::FileNotFound))?;
        let output_file = File::create(file_path)?;
        let mut writer = BufWriter::new(output_file);
        let mut buffer = [0u8; 16384];
        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            writer.write_all(&buffer[..bytes_read])?;
        }
        Ok(())
    }
}

/// 获取img的src值
pub(crate) fn get_img_src(html: &[u8], tag: &str, attribute: &str) -> Option<Vec<u8>> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let mut reader = Reader::from_reader(html);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                if e.name().as_ref().eq_ignore_ascii_case(tag) {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref().eq_ignore_ascii_case(attribute) {
                            return Some(attr.value.as_bytes().to_vec());
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    None
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
pub fn read_from_file<P: AsRef<Path>>(file: P) -> IResult<EpubBook> {
    read_from_reader(std::fs::File::open(file)?)
}

///
/// 从任意reader读取epub
///
pub fn read_from_reader<T: Read + Seek + Sync + Send + 'static>(value: T) -> IResult<EpubBook> {
    let reader = EpubReader::new(value)?;
    let mut book = EpubBook::default();
    let re: std::sync::Arc<std::sync::Mutex<Box<dyn EpubReaderTrait + Sync + Send>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Box::new(reader)));
    book.set_reader(std::sync::Arc::clone(&re));

    re.lock().unwrap().read(&mut book)?;
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
    use crate::{
        common::tests::download_epub_file,
        epub::reader::{get_img_src, read_meta_xml},
        prelude::*,
    };

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
                .with_version("2.0")
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
        let mut b = create_book().book().unwrap();
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

        println!("version = {}", b.version());

        // 多出来的一个是导航 nav.xhtml
        assert_eq!(b.assets().len() + 1, nb.assets().len());
        // 多出来的一个是导航 nav.xhtml
        assert_eq!(b.chapters().len() + 1, nb.chapters().len());
        assert_ne!(0, b.assets_mut().next().unwrap().data_mut().unwrap().len());

        // 读取html
        let chapter = nb.get_chapter_mut("0.xhtml");
        // assert_ne!(None, chapter);
        assert!(chapter.is_some());

        if let Some(c) = chapter {
            //body data
            let data = c.data_mut();
            assert!(data.is_some());
            let d = String::from_utf8(data.unwrap().to_vec()).unwrap();
            println!("d [{}]", d);
            assert_eq!(
                r#"
    <h1 style="text-align: center">ok</h1>
html
  "#,
                d
            );

            //raw_data
            let raw_data = c.raw_data();
            assert!(raw_data.is_some());
        }

        for a in nb.assets_mut() {
            println!(
                "ass=[{}]",
                String::from_utf8(a.data_mut().unwrap().to_vec()).unwrap()
            );
        }

        println!("nav ={:?}", nb.nav());
        assert_eq!(1, nb.nav().len());
        assert_eq!("0.xhtml", nb.nav().as_slice()[0].file_name());

        // println!("{}",c.data().map(|f|String::from_utf8(f.to_vec())).unwrap().unwrap());
    }

    #[test]
    fn test_read_empty_toc() {
        let xml = r#"<?xml version='1.0' encoding='utf-8'?><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><head><meta content="1394" name="dtb:uid"/><meta content="0" name="dtb:depth"/><meta content="0" name="dtb:totalPageCount"/><meta content="0" name="dtb:maxPageNumber"/></head><docTitle><text>book_title</text></docTitle><navMap><navPoint id="0-0"><navLabel><text></text></navLabel><content src="0.xhtml"></content><navPoint id="0-0"><navLabel><text></text></navLabel><content src="0.xhtml"></content></navPoint></navPoint></navMap></ncx>"#;
        let mut book = EpubBook::default();
        let _ = read_nav_xml(xml, "".to_string(), &mut book).unwrap();

        let n = book.nav().as_slice();

        assert_eq!(1, n.len());
        assert_eq!("", n[0].title());
        assert_eq!(1, n[0].child().len());
        assert_eq!("", n[0].child().as_slice()[0].title());
    }

    #[test]
    fn test_no_oebps_prefix_path() {
        use crate::common::tests::download_zip_file;
        // 测试不同的toc.ncx文件位置
        // 相关文件没有存放在 OEBPS 目录内
        let name = "epub-book.epub";

        let mut book = read_from_file(
            download_zip_file(
                name,
                "https://github.com/user-attachments/files/19544787/epub-book.epub.zip",
            )
            .as_str(),
        )
        .unwrap();

        let nav = book.nav().as_slice();

        assert_ne!(0, nav.len());
        assert_ne!("", nav[0].title());
        let mut chap = book.chapters();
        assert!(chap.next().is_some());
        chap.next();
        chap.next();

        assert_ne!("", chap.next().unwrap().title());
        assert_ne!(None, book.chapters_mut().next().unwrap().data_mut());
    }

    #[test]
    fn test_read_epub3() {
        let name = "../target/epub3.epub";
        let url = "https://github.com/IDPF/epub3-samples/releases/download/20230704/childrens-literature.epub";
        download_epub_file(name, url);

        let mut book = read_from_file(name).unwrap();

        let nav = book.nav().as_slice();

        assert_ne!(0, nav.len());
        assert_ne!("", nav[0].title());
        println!(
            "title = {:?}",
            book.chapters_mut()
                .map(|f| format!("{}-{}", f.title(), f.file_name()))
                .collect::<Vec<String>>()
        );

        let mut chap = book.chapters_mut();
        // s04.xhtml 用 nav 里的 id 分片成章，整篇不再与分片并存
        assert_eq!(21, chap.len());
        assert_eq!(75, chap.next().unwrap().data_mut().unwrap().len());

        // println!("{}", String::from_utf8( chap.next().unwrap().data().unwrap().to_vec()).unwrap());
        println!("title = {}", chap.next().unwrap().title());
        println!("title = {}", chap.next().unwrap().title());

        let nt = chap.next().unwrap();
        println!("nt = {}", nt.title());
        assert_eq!(2392, nt.data_mut().unwrap().to_vec().len());

        for i in chap {
            if let Some(p) = i.parser() {
                assert_ne!(0, p.extract_plain_text().len());
            }
        }
        assert!(book.get_chapter("s04.xhtml#pgepubid00536").is_some());
        // assert!(chap.next().is_some());
        // chap.next();
        // chap.next();
        // assert_ne!("", chap.next().unwrap().title());
        // assert_ne!(None, book.chapters_mut().next().unwrap().data());
    }

    /// 测试使用xhtml格式存储目录的 epub3
    #[test]
    fn test_read_epub3_toc_xhtml() {
        let name = "../target/epub3/xhtml.epub";
        download_epub_file(name, "https://github.com/IDPF/epub3-samples/releases/download/20230704/cc-shared-culture.epub");

        let mut epub = read_from_file(name).unwrap();

        assert_ne!(0, epub.nav().len());
        assert_ne!(0, epub.chapters().len());
        assert_ne!(
            0,
            String::from_utf8(
                epub.chapters_mut()
                    .next()
                    .unwrap()
                    .data_mut()
                    .unwrap()
                    .to_vec()
            )
            .unwrap()
            .len()
        );
    }
    /// 测试epub3的读取资源文件
    #[test]
    fn test_read_epub3_assets() {
        let name = "../target/epub3.epub";
        let url = "https://github.com/IDPF/epub3-samples/releases/download/20230704/childrens-literature.epub";
        download_epub_file(name, url);

        let mut book = read_from_file(name).unwrap();
        assert_ne!(0, book.assets().len());
        for ele in book.assets_mut() {
            assert_ne!(0, ele.data_mut().unwrap().len());
        }
    }

    #[test]
    fn test_read_escaped() {
        let f = if std::path::Path::new("target").exists() {
            "target/es.epub"
        } else {
            "../target/es.epub"
        };

        EpubBuilder::default()
            .with_title("Test Story `~!@#$%^&*()_+ and []\\{}| and ;':\" and ,./<>?")
            .with_creator("Test Creator `~!@#$%^&*()_+ and []\\{}| and ;':\" and ,./<>?")
            .with_date("2024-03-14")
            .with_description("Test Description `~!@#$%^&*()_+ and []\\{}| and ;':\" and ,./<>?")
            .with_identifier("isbn")
            .with_publisher("Test Publisher `~!@#$%^&*()_+ and []\\{}| and ;':\" and ,./<>?")
            .add_chapter(
                EpubHtml::default()
                    .with_file_name("0.xml")
                    .with_title("Test Title `~!@#$%^&*()_+ and []\\{}| and2 ;':\" and3 ,./<>?")
                    .with_data("<p>Test Content </p>".as_bytes().to_vec()),
            )
            .add_assets("1.css", "p{color:red}".as_bytes().to_vec())
            .metadata("s", "d")
            .metadata("h", "m")
            .file(f)
            .unwrap();
        let book = read_from_file(f).unwrap();
        assert_eq!(1, book.nav().len());
        assert_eq!(
            "1. Test Title `~!@#$%^&*()_+ and []\\{}| and2 ;':\" and3 ,./<>?",
            book.nav().next().unwrap().title()
        );

        assert_eq!(
            "Test Story `~!@#$%^&*()_+ and []\\{}| and ;':\" and ,./<>?",
            book.title()
        );
        assert_eq!(
            "Test Publisher `~!@#$%^&*()_+ and []\\{}| and ;':\" and ,./<>?",
            book.publisher().unwrap()
        );
    }

    #[test]
    fn test_read_meta_xml() {
        let xml = r##"<meta property="dcterms:modified">2025-10-18T06:23:37Z</meta>
        <dc:date id="date">2024-03-14</dc:date>
        <meta name="generator" content="iepub-1.2.0"/>
        <dc:identifier id="id">isbn</dc:identifier>
        <dc:title>Test Story `~!@#$%^&amp;*()_+ and []\{}| and ;&apos;:&quot; and ,./&lt;&gt;?</dc:title>
        <dc:creator id="creator">Test Creator `~!@#$%^&amp;*()_+ and []\{}| and ;&apos;:&quot; and ,./&lt;&gt;?</dc:creator>
        <dc:description>Test Description `~!@#$%^&amp;*()_+ and []\{}| and ;&apos;:&quot; and ,./&lt;&gt;?</dc:description>
        <meta property="desc">Test Description `~!@#$%^&amp;*()_+ and []\{}| and ;&apos;:&quot; and ,./&lt;&gt;?</meta>
        <dc:publisher id="publisher">Test Publisher `~!@#$%^&amp;*()_+ and []\{}| and ;&apos;:&quot; and ,./&lt;&gt;?</dc:publisher>
        <meta s="d"/>
        <meta h="m"/>"##;

        let mut book = EpubBook::default();
        let mut reader = quick_xml::Reader::from_str(xml);
        reader.config_mut().trim_text(true);
        read_meta_xml(&mut reader, &mut book).unwrap();
        assert_eq!(
            book.title(),
            r#"Test Story `~!@#$%^&*()_+ and []\{}| and ;':" and ,./<>?"#
        );
        assert_eq!(
            book.creator().unwrap(),
            r#"Test Creator `~!@#$%^&*()_+ and []\{}| and ;':" and ,./<>?"#
        );
        assert_eq!(
            book.description().unwrap(),
            r#"Test Description `~!@#$%^&*()_+ and []\{}| and ;':" and ,./<>?"#
        );
        assert_eq!(
            book.publisher().unwrap(),
            r#"Test Publisher `~!@#$%^&*()_+ and []\{}| and ;':" and ,./<>?"#
        );
    }

    #[test]
    fn test_get_img_src() {
        let html = r#"<div class="center" style="margin: 3.5em 0 0 0;">
        <div class="illus duokan-image-single" style="text-align: center; padding: 0pt; margin: 0pt;">
            <svg xmlns="http://www.w3.org/2000/svg" height="100%" preserveAspectRatio="xMidYMid meet" version="1.1" viewBox="0 0 567 799" width="100%" xmlns:xlink="http://www.w3.org/1999/xlink">
                <image height="799" width="567" xlink:href="../Images/cover.jpg"/>
            </svg>
        </div>
    </div>"#;
        let s = get_img_src(html.as_bytes(), "image", "xlink:href");
        assert_ne!(None, s);
        println!("{}", String::from_utf8(s.unwrap()).unwrap());

        let html = r#"<div class="center" style="margin: 3.5em 0 0 0;">
        <div class="illus duokan-image-single" style="text-align: center; padding: 0pt; margin: 0pt;">
            <svg xmlns="http://www.w3.org/2000/svg" height="100%" preserveAspectRatio="xMidYMid meet" version="1.1" viewBox="0 0 567 799" width="100%" xmlns:xlink="http://www.w3.org/1999/xlink">
                <imagexlink:href="../Images/cover.jpg"/>
            </svg>
        </div>
    </div>"#;
        let s = get_img_src(html.as_bytes(), "image", "xlink:href");
        assert_eq!(None, s);
    }

    #[test]
    fn test_read_cover_from_guide() {
        let name = if std::path::Path::new("target").exists() {
            "target/cover_from_guide.epub"
        } else {
            "../target/cover_from_guide.epub"
        };
        let url = "https://github.com/user-attachments/files/23620295/05.zip";
        download_epub_file(name, url);

        let book = read_from_file(name).unwrap();
        assert!(book.cover().is_some());
    }

    ///
    /// toc.ncx 不再作为assets的一部分，也就不对调用方公开了
    ///
    #[test]
    fn test_remove_toc_from_assets() {
        let name = if std::path::Path::new("target").exists() {
            "target/cover_from_guide.epub"
        } else {
            "../target/cover_from_guide.epub"
        };
        let url = "https://github.com/user-attachments/files/23620295/05.zip";
        download_epub_file(name, url);

        let book = read_from_file(name).unwrap();

        assert!(book.toc().is_some());

        assert!(!book.assets().any(|f| f.id() == "toc" || f.id() == "ncx"));
        assert_eq!(10, book.nav().len());
    }
}
