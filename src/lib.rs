use std::{collections::HashMap, slice::Iter};
use std::str::FromStr;

use common::{EpubItem, LinkRel};
use derive::epub_base;
use html::{to_html, to_nav_html, to_opf, to_toc_xml};

pub mod builder;
mod html;
pub mod zip_writer;

shadow_rs::shadow!(build);
#[allow(dead_code)]
/**
 * 链接文件，可能是css
 */
#[derive(Debug)]
pub struct EpubLink {
    pub rel: LinkRel,
    pub file_type: String,
    pub href: String,
}

#[epub_base]
#[derive(Debug, Default)]
pub struct EpubHtml {
    pub lang: String,
    links: Option<Vec<EpubLink>>,
    /// 章节名称
    title: String,
    /// 自定义的css，会被添加到link下
    css: Option<String>,
}

impl EpubHtml {
    pub fn set_title(&mut self, title: &str) {
        self.title.clear();
        self.title.push_str(title);
    }

    pub fn with_title(mut self, title: &str) -> Self {
        self.set_title(title);

        self
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn set_css(&mut self, css: &str) {
        self.css = Some(String::from(css));
    }
    pub fn with_css(mut self, css: &str) -> Self {
        self.set_css(css);
        self
    }
    pub fn css(&self) -> Option<&str> {
        self.css.as_deref()
    }

    fn set_language(&mut self, lang: &str) {
        self.lang = String::from_str(lang).unwrap();
    }

    pub fn add_link(&mut self, link: EpubLink) {
        if let Some(links) = &mut self.links {
            links.push(link);
        } else {
            self.links = Some(vec![link]);
        }
    }

    fn get_links(&mut self) -> Option<&mut Vec<EpubLink>> {
        self.links.as_mut()
    }
}

// impl Deref for EpubNcx {
//     type Target = EpubItem;
//     fn deref(&self) -> &Self::Target {
//         &self.item
//      }

// }

// impl DerefMut for EpubNcx {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         &mut self.item
//     }
// }

///
/// 非章节资源
///
/// 例如css，字体，图片等
///
#[epub_base]
#[derive(Debug, Default)]
pub struct EpubAssets {}

impl EpubAssets {}

///
/// 目录信息
///
/// 支持嵌套
///
#[epub_base]
#[derive(Debug, Default,Clone)]
pub struct EpubNav {
    /// 章节目录
    /// 如果需要序号需要调用方自行处理
    title: String,
    child: Vec<EpubNav>,
}

impl EpubNav {
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn set_title(&mut self, title: &str) {
        self.title.clear();
        self.title.push_str(title);
    }
    pub fn with_title(mut self, title: &str) -> Self {
        self.set_title(title);
        self
    }
    ///
    ///
    /// 添加下级目录
    ///
    pub fn push(&mut self, child: EpubNav) {
        self.child.push(child);
    }
}

///
/// 书籍元数据
///
/// 自定义的数据，不在规范内
///
#[derive(Debug, Default)]
pub struct EpubMetaData {
    /// 属性
    attr: HashMap<String, String>,
    /// 文本
    text: Option<String>,
}

impl EpubMetaData {
    pub fn push_attr(mut self, key: &str, value: &str) -> Self {
        self.attr.insert(String::from(key), String::from(value));
        self
    }

    pub fn set_text(mut self, text: &str) -> Self {
        if let Some(t) = &mut self.text {
            t.clear();
            t.push_str(text);
        } else {
            self.text = Some(String::from(text));
        }
        self
    }

    pub fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }

    pub fn attrs(&self) -> std::collections::hash_map::Iter<'_, String, String> {
        self.attr.iter()
    }
}

#[derive(Debug, Default)]
struct EpubBookInfo {
    /// 书名
    title: String,

    /// 标志，例如imbi
    identifier: String,
    /// 作者
    creator: Option<String>,
    ///
    /// 简介
    ///
    description: Option<String>,
    /// 捐赠者？
    contributor: Option<String>,

    /// 出版日期
    date: Option<String>,

    /// 格式?
    format: Option<String>,
    /// 出版社
    publisher: Option<String>,
    /// 主题？
    subject: Option<String>,
}

/// 书本
#[derive(Debug, Default)]
pub struct EpubBook {
    /// 上次修改时间
    last_modify: Option<String>,
    /// 书本信息
    info: EpubBookInfo,
    /// 元数据
    meta: Vec<EpubMetaData>,
    /// 目录信息
    nav: Vec<EpubNav>,
    /// 资源
    assets: Vec<EpubAssets>,
    /// 章节
    chapters: Vec<EpubHtml>,
    /// 封面
    cover: Option<EpubAssets>,
}

impl EpubBook {
    derive::option_string_method!(info, creator);
    derive::option_string_method!(info, description);
    derive::option_string_method!(info, contributor);
    derive::option_string_method!(info, date);
    derive::option_string_method!(info, format);
    derive::option_string_method!(info, publisher);
    derive::option_string_method!(info, subject);
    // /
    // / 设置epub最后修改时间
    // /
    // / # Examples
    // /
    // / ```
    // / let mut epub = EpubBook::default();
    // / epub.set_last_modify("2024-06-28T08:07:07UTC");
    // / ```
    // /
    derive::option_string_method!(last_modify);
}

// 元数据
impl EpubBook {
    pub fn set_title(&mut self, title: &str) {
        self.info.title.clear();
        self.info.title.push_str(title);
    }
    pub fn title(&self) -> &str {
        self.info.title.as_str()
    }
    pub fn identifier(&self) -> &str {
        self.info.identifier.as_str()
    }
    pub fn set_identifier(&mut self, identifier: &str) {
        self.info.identifier.clear();
        self.info.identifier.push_str(identifier);
    }

    ///
    /// 添加元数据
    ///
    /// # Examples
    ///
    /// ```
    /// use iepub::EpubBook;
    /// let mut epub = EpubBook::default();
    /// epub.add_meta(EpubMetaData::default().push_attr("k", "v").set_text("text"));
    /// ```
    ///
    pub fn add_meta(&mut self, meta: EpubMetaData) {
        self.meta.push(meta);
    }
    pub fn meta(&self) -> &[EpubMetaData] {
        &self.meta
    }
    ///
    /// 添加目录
    ///
    #[inline]
    pub fn add_nav(&mut self, nav: EpubNav) {
        self.nav.push(nav);
    }

    pub fn add_assets(&mut self, assets: EpubAssets) {
        self.assets.push(assets);
    }

    pub fn add_chapter(&mut self, chap: EpubHtml) {
        self.chapters.push(chap);
    }

    #[inline]
    pub fn chapters(&self) -> Iter<EpubHtml> {
        self.chapters.iter()
    }

    pub fn set_cover(&mut self, cover: EpubAssets) {
        self.cover = Some(cover);
    }
}

type EpubResult<T> = Result<T, EpubError>;

///
/// epub输出实现，可通过实现该trait从而自定义输出方案。
///
/// 具体实现应该是写入到zip文件
///
pub trait EpubWriter {
    /// 新建
    /// file 输出的epub文件路径
    ///
    fn new(file: &str) -> EpubResult<Self>
    where
        Self: Sized;

    ///
    /// file epub中的文件目录
    /// data 要写入的数据
    ///
    fn write(&mut self, file: &str, data: &[u8]) -> EpubResult<()>;
}

#[derive(Debug)]
pub enum EpubError {
    /// io 错误
    Io(std::io::Error),
    /// invalid Zip archive: {0}
    InvalidArchive(&'static str),

    /// unsupported Zip archive: {0}
    UnsupportedArchive(&'static str),

    /// specified file not found in archive
    FileNotFound,

    /// The password provided is incorrect
    InvalidPassword,

    Utf8(std::string::FromUtf8Error),

    Xml(quick_xml::Error),
    Unknown,
}

impl std::fmt::Display for EpubError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

static CONTAINER_XML: &str = r#"<?xml version='1.0' encoding='utf-8'?>
<container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0">
  <rootfiles>
    <rootfile media-type="application/oebps-package+xml" full-path="{opf}"/>
  </rootfiles>
</container>
"#;

impl EpubBook {
    /// 写入基础的文件
    fn write_base(&self, writer: &mut impl EpubWriter) -> EpubResult<()> {
        writer.write(
            "META-INF/container.xml",
            CONTAINER_XML.replace("{opf}", common::OPF).as_bytes(),
        )?;
        writer.write("mimetype", "application/epub+zip".as_bytes())?;

        writer.write(
            common::OPF,
            to_opf(
                self,
                format!("{}-{}", crate::build::PROJECT_NAME, crate::build::VERSION).as_str(),
            )
            .as_bytes(),
        )?;

        Ok(())
    }

    /// 写入资源文件
    fn write_assets(&self, writer: &mut impl EpubWriter) -> EpubResult<()> {
        let m = &self.assets;
        for ele in m {
            if ele.data().is_none() {
                continue;
            }
            writer.write(
                format!("{}{}", common::EPUB, ele.file_name()).as_str(),
                ele.data().unwrap(),
            )?;
        }
        Ok(())
    }

    /// 写入章节文件
    fn write_chapters(&self, writer: &mut impl EpubWriter) -> EpubResult<()> {
        let chap = &self.chapters;
        for ele in chap {
            if ele.data().is_none() {
                continue;
            }

            let html = to_html(ele);

            writer.write(
                format!("{}{}", common::EPUB, ele.file_name()).as_str(),
                html.as_bytes(),
            )?;
        }

        Ok(())
    }
    /// 写入目录
    fn write_nav(&self, writer: &mut impl EpubWriter) -> EpubResult<()> {
        // 目录包括两部分，一是自定义的用于书本导航的html，二是epub规范里的toc.ncx文件
        writer.write(common::NAV, to_nav_html(self.title(), &self.nav).as_bytes())?;
        writer.write(common::TOC, to_toc_xml(self.title(), &self.nav).as_bytes())?;

        Ok(())
    }

    ///
    /// 生成封面
    ///
    /// 拷贝资源文件以及生成对应的xhtml文件
    ///
    fn write_cover(&self, writer: &mut impl EpubWriter) -> EpubResult<()> {
        if let Some(cover) = &self.cover {
            writer.write(
                format!("{}{}", common::EPUB, cover.file_name()).as_str(),
                cover.data().as_ref().unwrap(),
            )?;

            let mut html = EpubHtml::default();
            html.set_data(
                format!("<img src=\"{}\" alt=\"Cover\"/>", cover.file_name())
                    .as_bytes()
                    .to_vec(),
            );
            html.title = String::from("Cover");
            writer.write(common::COVER, to_html(&html).as_bytes())?;
        }
        Ok(())
    }
    ///
    ///
    /// 写入到指定文件
    ///
    /// [file] 文件路径，一般以.epub结尾
    ///
    pub fn write(&self, file: &str) -> EpubResult<()> {
        let mut writer = zip_writer::ZipFileWriter::new(file)?;
        self.write_with_writer(&mut writer)
    }

    ///
    /// 使用自定义输出方案
    ///
    /// # Examples
    ///
    /// 1. 写入内存
    ///
    /// ```rust
    /// let mut writer = zip_writer::ZipMemoeryWriter::new("无用").unwrap();
    ///
    /// let mut book = EpubBook::default();
    /// book.write_with_writer(&mut writer);
    ///
    /// ```
    ///
    ///
    pub fn write_with_writer(&self, writer: &mut impl EpubWriter) -> EpubResult<()> {
        self.write_base(writer)?;
        self.write_assets(writer)?;
        self.write_chapters(writer)?;
        self.write_nav(writer)?;
        self.write_cover(writer)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use std::{fs::File, io::Read, path::Path};

    use common::EpubItem;

    use crate::{EpubAssets, EpubBook, EpubHtml, EpubNav};

    #[test]
    fn write_assets() {
        let mut book = EpubBook::default();

        // 添加文本资源文件

        let mut css = EpubAssets::default();
        css.set_file_name("style/1.css");
        css.set_data(String::from("ok").as_bytes().to_vec());

        book.add_assets(css);

        // 添加目录，注意目录和章节并无直接关联关系，需要自行维护保证导航到正确位置
        let mut n = EpubNav::default();
        n.set_title("作品说明");
        n.set_file_name("chaps/0.xhtml");

        let mut n1 = EpubNav::default();
        n1.set_title("第一卷");

        let mut n2 = EpubNav::default();
        n2.set_title("第一卷 第一章");
        n2.set_file_name("chaps/1.xhtml");

        let mut n3 = EpubNav::default();
        n3.set_title("第一卷 第二章");
        n3.set_file_name("chaps/2.xhtml");
        n1.push(n2);

        book.add_nav(n);
        book.add_nav(n1);
        // 添加章节
        let mut chap = EpubHtml::default();
        chap.set_file_name("chaps/0.xhtml");
        chap.set_title("标题1");
        // 章节的数据并不需要填入完整的html，只需要片段即可，输出时会结合其他数据拼接成完整的html
        chap.set_data(String::from("<p>章节内容html片段</p>").as_bytes().to_vec());

        book.add_chapter(chap);

        chap = EpubHtml::default();
        chap.set_file_name("chaps/1.xhtml");
        chap.set_title("标题2");
        chap.set_data(String::from("第一卷 第一章content").as_bytes().to_vec());

        book.add_chapter(chap);
        chap = EpubHtml::default();
        chap.set_file_name("chaps/2.xhtml");
        chap.set_title("标题2");
        chap.set_data(String::from("第一卷 第二章content").as_bytes().to_vec());

        book.add_chapter(chap);

        book.set_title("书名");
        book.set_creator("作者");
        book.set_identifier("id");
        book.set_description("desc");
        // epub.cover = Some(EpubAssets::default());

        let mut cover = EpubAssets::default();
        cover.set_file_name("cover.jpg");

        let p = Path::new("cover.jpg");
        println!("{:?}", std::env::current_dir());
        let mut cf = File::open(p).unwrap();
        let mut data: Vec<u8> = Vec::new();
        cf.read_to_end(&mut data).unwrap();

        cover.set_data(data);

        book.set_cover(cover);

        book.write("target/test.epub").expect("write error");
    }
}
