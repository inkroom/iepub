use std::collections::HashMap;

use quick_xml::events::Event;

use crate::{EpubBook, EpubError, EpubHtml, EpubNav, EpubResult};

static XHTML_1: &str = r#"<?xml version='1.0' encoding='utf-8'?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" epub:prefix="z3998: http://www.daisy.org/z3998/2012/vocab/structure/#" lang="zh" xml:lang="zh">
  <head>
    <title>"#;

static XHTML_2: &str = r#"</title>
"#;

static XHTML_3: &str = r#"
</head>
  <body>
    <h1>"#;

static XHTML_4: &str = r#"</h1>
"#;
static XHTML_5: &str = r#"
  </body>
</html>"#;

///
/// Examples:
///
/// <?xml version='1.0' encoding='utf-8'?>
/// <!DOCTYPE html>
/// <html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" epub:prefix="z3998: http://www.daisy.org/z3998/2012/vocab/structure/#" lang="zh" xml:lang="zh">
///   <head>
///     <title>{}</title>
///     {}
///   </head>
///   <body>
///     <h1>{}</h1>
///     {}
///   </body>
/// </html>
///
///
///
/// 生成html
pub(crate) fn to_html(chap: &mut EpubHtml) -> String {
    let mut css = String::new();
    if let Some(links) = chap.links.as_ref() {
        for ele in links {
            css.push_str(
                format!(
                    "<link href=\"{}\" rel=\"stylesheet\" type=\"text/css\"/>",
                    ele.href
                )
                .as_str(),
            );
        }
    }

    let cus_css = chap.css();
    if let Some(v) = cus_css {
        css.push_str(format!("\n<style type=\"text/css\">{}</style>", v).as_str());
    }
    let mut body = String::new();
    {
        body.insert_str(
            0,
            String::from_utf8(chap.data().as_ref().unwrap().to_vec())
                .unwrap()
                .as_str(),
        );
        // 正文
    }
    format!(
        "{}{}{}{}{}{}{}{}{}",
        XHTML_1,
        chap.title,
        XHTML_2,
        css, // css link
        XHTML_3,
        chap.title,
        XHTML_4,
        body, // 正文
        XHTML_5
    )
    // format_args!(XHTML,chap.title,"",chap.title,chap.data.unwrap())
}

fn to_nav_xml(nav: &[EpubNav]) -> String {
    let mut xml = String::new();
    xml.push_str("<ol>");
    for ele in nav {
        if ele.child.is_empty() {
            // 没有下一级
            xml.push_str(
                format!(
                    "<li><a href=\"{}\">{}</a></li>",
                    ele.file_name(),
                    ele.title()
                )
                .as_str(),
            );
        } else {
            xml.push_str(
                format!(
                    "<li><a href=\"{}\">{}</a>{}</li>",
                    ele.child[0].file_name(),
                    ele.title(),
                    to_nav_xml(&ele.child).as_str()
                )
                .as_str(),
            );
        }
    }
    xml.push_str("</ol>");
    xml
}

/// 生成自定义的导航html
pub(crate) fn to_nav_html(book_title: &str, nav: &[EpubNav]) -> String {
    let ex = r#"<?xml version='1.0' encoding='utf-8'?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" lang="zh" xml:lang="zh">
  <head>
    <title>{book_title}</title>
  </head>
  <body>
    <nav epub:type="toc" id="id" role="doc-toc">
      <h2>{book_title}</h2>
    {nav_xml}
    </nav>
  </body>
</html>"#;
    let mut html = ex.replace("{book_title}", book_title);
    html = html.replace("{nav_xml}", to_nav_xml(nav).as_str());
    html
}

fn to_toc_xml_point(nav: &[EpubNav], parent: usize) -> String {
    let mut xml = String::new();
    for (index, ele) in nav.iter().enumerate() {
        xml.push_str(format!("<navPoint id=\"{}-{}\">", parent, index).as_str());
        if ele.child.is_empty() {
            xml.push_str(
                format!(
                    "<navLabel><text>{}</text></navLabel><content src=\"{}\"></content>",
                    ele.title(),
                    ele.file_name()
                )
                .as_str(),
            );
        } else {
            xml.push_str(
                format!(
                    "<navLabel><text>{}</text></navLabel><content src=\"{}\"></content>{}",
                    ele.title(),
                    ele.child[0].file_name(),
                    to_toc_xml_point(&ele.child, index).as_str()
                )
                .as_str(),
            );
        }
        xml.push_str("</navPoint>");
    }
    xml
}

/// 生成epub中的toc.ncx文件
pub(crate) fn to_toc_xml(book_title: &str, nav: &[EpubNav]) -> String {
    let e = r#"<?xml version='1.0' encoding='utf-8'?>
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
  <head>
    <meta content="1394" name="dtb:uid"/>
    <meta content="0" name="dtb:depth"/>
    <meta content="0" name="dtb:totalPageCount"/>
    <meta content="0" name="dtb:maxPageNumber"/>
  </head>
  <docTitle>
    <text>"#;
    let mut xml = String::from(e);
    xml.push_str(book_title);
    xml.push_str("</text></docTitle><navMap>");
    // 正文
    xml.push_str(to_toc_xml_point(nav, 0).as_str());

    // 结束
    xml.push_str("</navMap></ncx>");

    xml
}

impl From<quick_xml::Error> for EpubError {
    fn from(value: quick_xml::Error) -> Self {
        match value {
            quick_xml::Error::Io(e) => EpubError::Io(std::io::Error::other(e)),
            _ => EpubError::Xml(value),
        }
    }
}

fn get_media_type(file_name: &str) -> String {
    let f = file_name.to_lowercase();

    let mut types = HashMap::new();
    types.insert(".gif", String::from("image/gif"));
    types.insert(".jpg", String::from("image/jpeg"));
    types.insert(".jpeg", String::from("image/jpeg"));
    types.insert(".png", String::from("image/png"));
    types.insert(".svg", String::from("image/svg+xml"));
    types.insert(".webp", String::from("image/webp"));
    types.insert(".mp3", String::from("audio/mpeg"));
    types.insert(".mp4", String::from("audio/mp4"));
    types.insert(".css", String::from("text/css"));
    types.insert(".ttf", String::from("application/font-sfnt"));
    types.insert(".oft", String::from("application/font-sfnt"));
    types.insert(".woff", String::from("application/font-woff"));
    types.insert(".woff", String::from("font/woff2"));
    types.insert(".xhtml", String::from("application/xhtml+xml"));
    types.insert(".js", String::from("application/javascript"));
    types.insert(".opf", String::from("application/x-dtbncx+xml"));
    let x: &[_] = &['.'];
    if let Some(index) = f.rfind(x) {
        let sub = &f[index..f.len()];
        return match types.get(&sub) {
            Some(t) => String::from(t),
            None => String::new(),
        };
    };

    String::new()
}

fn write_metadata(
    book: &EpubBook,
    generator: &str,
    xml: &mut quick_xml::Writer<std::io::Cursor<Vec<u8>>>,
) -> EpubResult<()> {
    use quick_xml::events::{BytesStart, BytesText, Event};

    // metadata
    let mut metadata = BytesStart::new("metadata");
    metadata.push_attribute(("xmlns:dc", "http://purl.org/dc/elements/1.1/"));
    metadata.push_attribute(("xmlns:opf", "http://www.idpf.org/2007/opf"));

    xml.write_event(Event::Start(metadata.borrow()))?;

    // metadata 内元素
    let now = book.last_modify().map_or_else(
        || {
            format!("{}", chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%Z")) // chrono 可以自己实现
        },
        String::from,
    );

    xml.create_element("meta")
        .with_attribute(("property", "dcterms:modified"))
        .write_text_content(BytesText::new(now.as_str()))?;

    if let Some(v) = book.date() {
        xml.create_element("dc:date")
            .with_attribute(("id", "date"))
            .write_text_content(BytesText::new(v))?;
    }

    xml.create_element("meta")
        .with_attribute(("name", "generator"))
        .with_attribute(("content", generator))
        .write_empty()?;

    xml.create_element("dc:identifier")
        .with_attribute(("id", "id"))
        .write_text_content(BytesText::new(book.identifier()))?;
    xml.create_element("dc:title")
        .write_text_content(BytesText::new(book.title()))?;
    // xml
    // .create_element("dc:lang")
    // .write_text_content(BytesText::new(book.info.title.as_str()));
    if let Some(creator) = book.creator() {
        xml.create_element("dc:creator")
            .with_attribute(("id", "creator"))
            .write_text_content(BytesText::new(creator))?;
    }
    if let Some(desc) = book.description() {
        xml.create_element("dc:description")
            .write_text_content(BytesText::new(desc))?;

        xml.create_element("meta")
            .with_attribute(("property", "desc"))
            .write_text_content(BytesText::new(desc))?;
    }
    if book.cover.is_some() {
        xml.create_element("meta")
            .with_attribute(("name", "cover"))
            .with_attribute(("content", "cover-img"))
            .write_empty()?;
    }

    if let Some(v) = book.format() {
        xml.create_element("dc:format")
            .with_attribute(("id", "format"))
            .write_text_content(BytesText::new(v))?;
    }
    if let Some(v) = book.publisher() {
        xml.create_element("dc:publisher")
            .with_attribute(("id", "publisher"))
            .write_text_content(BytesText::new(v))?;
    }
    if let Some(v) = book.subject() {
        xml.create_element("dc:subject")
            .with_attribute(("id", "subject"))
            .write_text_content(BytesText::new(v))?;
    }
    if let Some(v) = book.contributor() {
        xml.create_element("dc:contributor")
            .with_attribute(("id", "contributor"))
            .write_text_content(BytesText::new(v))?;
    }

    // 自定义的meta
    for ele in book.meta() {
        let mut x = xml.create_element("meta");
        for (key, value) in ele.attrs() {
            x = x.with_attribute((key.as_str(), value.as_str()));
        }
        if let Some(t) = ele.text() {
            x.write_text_content(BytesText::new(t))?;
        } else {
            x.write_empty()?;
        }
    }

    xml.write_event(Event::End(metadata.to_end()))?;

    Ok(())
}

pub(crate) fn do_to_opf(book: &EpubBook, generator: &str) -> EpubResult<String> {
    let vue: Vec<u8> = Vec::new();
    let mut xml: quick_xml::Writer<std::io::Cursor<Vec<u8>>> = quick_xml::Writer::new(std::io::Cursor::new(vue));
    use quick_xml::events::*;

    xml.write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;

    let mut html = BytesStart::new("package");
    html.push_attribute(("xmlns", "http://www.idpf.org/2007/opf"));
    html.push_attribute(("unique-identifier", "id"));
    html.push_attribute(("version", "3.0"));
    html.push_attribute(("prefix", "rendition: http://www.idpf.org/vocab/rendition/#"));

    xml.write_event(Event::Start(html.borrow()))?;

    // 写入 metadata
    write_metadata(book, generator, &mut xml)?;

    // manifest
    let manifest = BytesStart::new("manifest");
    xml.write_event(Event::Start(manifest.borrow()))?;

    // manifest 内 item
    if let Some(cover) = &book.cover {
        xml.create_element("item")
            .with_attribute(("href", cover.file_name()))
            .with_attribute(("id", "cover-img"))
            .with_attribute(("media-type", get_media_type(cover.file_name()).as_str()))
            .with_attribute(("properties", "cover-image"))
            .write_empty()?;
        xml.create_element("item")
            .with_attribute(("href", common::COVER.replace(common::EPUB, "").as_str()))
            .with_attribute(("id", "cover"))
            .with_attribute(("media-type", "application/xhtml+xml"))
            .write_empty()?;
    }

    for (index, ele) in book.chapters.iter().enumerate() {
        xml.create_element("item")
            .with_attribute(("href", ele.file_name()))
            .with_attribute(("id", format!("chap_{}", index).as_str()))
            .with_attribute(("media-type", "application/xhtml+xml"))
            .write_empty()?;
    }

    for (index, ele) in book.assets.iter().enumerate() {
        xml.create_element("item")
            .with_attribute(("href", ele.file_name()))
            .with_attribute(("id", format!("assets_{}", index).as_str()))
            .with_attribute(("media-type", get_media_type(ele.file_name()).as_str()))
            .write_empty()?;
    }
    // toc
    xml.create_element("item")
        .with_attribute(("href", common::TOC.replace(common::EPUB, "").as_str()))
        .with_attribute(("id", "toc"))
        .with_attribute(("media-type", "application/x-dtbncx+xml"))
        .write_empty()?;
    // nav
    xml.create_element("item")
        .with_attribute(("href", common::NAV.replace(common::EPUB, "").as_str()))
        .with_attribute(("id", "nav"))
        .with_attribute(("media-type", "application/xhtml+xml"))
        .with_attribute(("properties", "nav"))
        .write_empty()?;

    xml.write_event(Event::End(manifest.to_end()))?;

    let mut spine = BytesStart::new("spine");
    spine.push_attribute(("toc", "ncx"));
    xml.write_event(Event::Start(spine.borrow()))?;
    // 把导航放第一个 nav
    xml.create_element("itemref")
        .with_attribute(("idref", "nav"))
        .write_empty()?;
    // spine 内的 itemref
    for (index, _ele) in book.chapters.iter().enumerate() {
        xml.create_element("itemref")
            .with_attribute(("idref", format!("chap_{}", index).as_str()))
            .write_empty()?;
    }
    xml.write_event(Event::End(spine.to_end()))?;

    xml.write_event(Event::End(html.to_end()))?;

    match String::from_utf8(xml.into_inner().into_inner()) {
        Ok(v) => Ok(v),
        Err(e) => Err(EpubError::Utf8(e)),
    }
}

/// 生成OPF
pub(crate) fn to_opf(book: &EpubBook, generator: &str) -> String {
    match do_to_opf(book, generator) {
        Ok(s) => s,
        Err(_) => String::new(),
    }
}

///
/// 解析html获取相关数据
///
pub(crate) fn get_html_info(html: &str, chap: &mut EpubHtml) -> EpubResult<()> {
    use quick_xml::reader::Reader;
    let mut reader = Reader::from_str(html);
    // reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut parent: Vec<&str> = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => {
                return Ok(());
            }
            Err(e) => {
                return Err(EpubError::Xml(e));
            }
            Ok(Event::Start(body)) => match body.name().as_ref() {
                b"html" => {
                    parent.push("html");
                }
                b"head" => {
                    if parent.len() != 1 || parent[0] != "html" {
                        return Err(EpubError::Unknown);
                    }
                    parent.push("head");
                }
                b"title" => {
                    if parent.len() == 2 && parent[0] == "html" && parent[1] == "head" {
                        parent.push("title");
                    }
                }
                b"body" => {
                    let m = reader
                        .read_text(body.to_end().to_owned().name())
                        .map(|f| f.to_string())
                        .map_err(|f| EpubError::Xml(f));
                    if m.is_ok() {
                        chap.set_data(m.unwrap().into_bytes());
                    }
                }
                _ => {}
            },
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"title" => {
                    if !parent.is_empty() {
                        parent.remove(parent.len() - 1);
                    }
                }
                b"head" => {
                    if !parent.is_empty() {
                        parent.remove(parent.len() - 1);
                    }
                }
                b"body" => {
                    if !parent.is_empty() {
                        parent.remove(parent.len() - 1);
                    }
                }
                b"html" => {
                    if !parent.is_empty() {
                        parent.remove(parent.len() - 1);
                    }
                }
                _ => {}
            },
            Ok(Event::Text(e)) => {
                if parent.len() == 3 && parent[2] == "title" {
                    let v = String::from_utf8(e.into_inner().to_vec())
                        .map_err(|f| EpubError::Utf8(f))?;
                    chap.set_title(v.as_str().trim());
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod test {
    use common::{EpubItem, LinkRel};

    use crate::{
        html::{get_html_info, get_media_type, to_html, to_toc_xml},
        EpubAssets, EpubBook, EpubHtml, EpubLink, EpubMetaData, EpubNav,
    };

    use super::{to_nav_html, to_opf};

    #[test]
    fn test_to_html() {
        let mut t = EpubHtml::default();
        t.set_title("title");
        t.set_data(String::from("ok").as_bytes().to_vec());
        t.set_css("#id{width:10%}");
        let link = EpubLink {
            href: String::from("href"),
            file_type: String::from("css"),
            rel: LinkRel::CSS,
        };

        t.add_link(link);
        let html = to_html(&mut t);

        println!("{}", html);

        assert_eq!(
            html,
            r###"<?xml version='1.0' encoding='utf-8'?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" epub:prefix="z3998: http://www.daisy.org/z3998/2012/vocab/structure/#" lang="zh" xml:lang="zh">
  <head>
    <title>title</title>
<link href="href" rel="stylesheet" type="text/css"/>
<style type="text/css">#id{width:10%}</style>
</head>
  <body>
    <h1>title</h1>
ok
  </body>
</html>"###
        );
    }

    #[test]
    fn test_to_nav_html() {
        let mut n = EpubNav::default();
        n.set_title("作品说明");
        n.set_file_name("file_name");

        let mut n1 = EpubNav::default();
        n1.set_title("第一卷");

        let mut n2 = EpubNav::default();
        n2.set_title("第一卷 第一章");
        n2.set_file_name("0.xhtml");

        let mut n3 = EpubNav::default();
        n3.set_title("第一卷 第二章");
        n3.set_file_name("1.xhtml");
        n1.push(n2);

        let nav = vec![n, n1];

        let html = to_nav_html("book_title", &nav);

        println!("{}", html);

        assert_eq!(
            r###"<?xml version='1.0' encoding='utf-8'?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" lang="zh" xml:lang="zh">
  <head>
    <title>book_title</title>
  </head>
  <body>
    <nav epub:type="toc" id="id" role="doc-toc">
      <h2>book_title</h2>
    <ol><li><a href="file_name">作品说明</a></li><li><a href="0.xhtml">第一卷</a><ol><li><a href="0.xhtml">第一卷 第一章</a></li></ol></li></ol>
    </nav>
  </body>
</html>"###,
            html
        );
    }

    #[test]
    fn test_to_toc_xml() {
        let mut n = EpubNav::default();
        n.set_title("作品说明");
        n.set_file_name("file_name");

        let mut n1 = EpubNav::default();
        n1.set_title("第一卷");

        let mut n2 = EpubNav::default();
        n2.set_title("第一卷 第一章");
        n2.set_file_name("0.xhtml");

        let mut n3 = EpubNav::default();
        n3.set_title("第一卷 第二章");
        n3.set_file_name("1.xhtml");
        n1.push(n2);

        let nav = vec![n, n1];

        let html = to_toc_xml("book_title", &nav);

        println!("{}", html);

        assert_eq!(
            r###"<?xml version='1.0' encoding='utf-8'?>
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
  <head>
    <meta content="1394" name="dtb:uid"/>
    <meta content="0" name="dtb:depth"/>
    <meta content="0" name="dtb:totalPageCount"/>
    <meta content="0" name="dtb:maxPageNumber"/>
  </head>
  <docTitle>
    <text>book_title</text></docTitle><navMap><navPoint id="0-0"><navLabel><text>作品说明</text></navLabel><content src="file_name"></content></navPoint><navPoint id="0-1"><navLabel><text>第一卷</text></navLabel><content src="0.xhtml"></content><navPoint id="1-0"><navLabel><text>第一卷 第一章</text></navLabel><content src="0.xhtml"></content></navPoint></navPoint></navMap></ncx>"###,
            html
        );
    }

    #[test]
    fn test_to_opf() {
        let mut epub = EpubBook::default();

        epub.set_title("中文");
        epub.set_creator("作者");
        epub.set_date("29939");
        epub.set_subject("subject");
        epub.set_format("format");
        epub.set_publisher("publisher");
        epub.set_contributor("contributor");
        epub.set_description("description");
        epub.set_identifier("identifier");

        let mut n = EpubNav::default();
        n.set_title("作品说明");
        n.set_file_name("file_name");

        let mut n1 = EpubNav::default();
        n1.set_title("第一卷");

        let mut n2 = EpubNav::default();
        n2.set_title("第一卷 第一章");
        n2.set_file_name("0.xhtml");

        let mut n3 = EpubNav::default();
        n3.set_title("第一卷 第二章");
        n3.set_file_name("1.xhtml");
        n1.push(n2);

        epub.add_nav(n);
        epub.add_nav(n1);

        epub.add_assets(EpubAssets::default());

        epub.add_chapter(EpubHtml::default());

        epub.set_cover(EpubAssets::default());

        epub.add_meta(
            EpubMetaData::default()
                .with_attr("ok", "ov")
                .with_text("new"),
        );

        epub.set_date("2024-06-28T08:07:07UTC");
        epub.set_last_modify("2024-06-28T03:07:07UTC");

        let res = to_opf(&epub, "epub-rs");
        println!("[{}]", res);

        let ass: &str = r###"<?xml version="1.0" encoding="utf-8"?><package xmlns="http://www.idpf.org/2007/opf" unique-identifier="id" version="3.0" prefix="rendition: http://www.idpf.org/vocab/rendition/#"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf"><meta property="dcterms:modified">2024-06-28T03:07:07UTC</meta><dc:date id="date">2024-06-28T08:07:07UTC</dc:date><meta name="generator" content="epub-rs"/><dc:identifier id="id">identifier</dc:identifier><dc:title>中文</dc:title><dc:creator id="creator">作者</dc:creator><dc:description>description</dc:description><meta property="desc">description</meta><meta name="cover" content="cover-img"/><dc:format id="format">format</dc:format><dc:publisher id="publisher">publisher</dc:publisher><dc:subject id="subject">subject</dc:subject><dc:contributor id="contributor">contributor</dc:contributor><meta ok="ov">new</meta></metadata><manifest><item href="" id="cover-img" media-type="" properties="cover-image"/><item href="cover.xhtml" id="cover" media-type="application/xhtml+xml"/><item href="" id="chap_0" media-type="application/xhtml+xml"/><item href="" id="assets_0" media-type=""/><item href="toc.ncx" id="toc" media-type="application/x-dtbncx+xml"/><item href="nav.xhtml" id="nav" media-type="application/xhtml+xml" properties="nav"/></manifest><spine toc="ncx"><itemref idref="nav"/><itemref idref="chap_0"/></spine></package>"###;

        assert_eq!(ass, res.as_str());
    }

    #[test]
    fn test_get_media_type() {
        assert_eq!(
            String::from("application/javascript"),
            get_media_type("1.js")
        );
        assert_eq!(String::from("text/css"), get_media_type("1.css"));
        assert_eq!(
            String::from("application/xhtml+xml"),
            get_media_type("1.xhtml")
        );
        assert_eq!(String::from("image/jpeg"), get_media_type("1.jpeg"));
    }

    #[test]
    fn test_get_html_info() {
        let mut book = EpubHtml::default();

        get_html_info(
            r"<html>
    <head><title> 测试标题 </title></head>
    <body>
    <p>段落1</p>ok
    </body>
         </html>",
            &mut book,
        )
        .unwrap();

        assert_eq!(r"测试标题", book.title());

        assert_eq!(
            r"
    <p>段落1</p>ok
    ",
            String::from_utf8(book.data().unwrap().to_vec()).unwrap()
        );
    }
}
