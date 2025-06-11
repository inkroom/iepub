use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::io::Write;
use std::rc::Rc;
use std::str::FromStr;

use super::common::{self};
use super::html::{get_html_info, to_html};
use crate::common::{IError, IResult};
use crate::epub::common::LinkRel;
use crate::epub_base_field;

pub(crate) mod info {
    include!(concat!(env!("OUT_DIR"), "/version.rs"));
}

/**
 * 链接文件，可能是css
 */
#[derive(Debug)]
pub struct EpubLink {
    pub rel: LinkRel,
    pub file_type: String,
    pub href: String,
}

epub_base_field! {
    #[derive(Default)]
    pub struct EpubHtml {
        pub lang: String,
        links: Option<Vec<EpubLink>>,
        /// 章节名称
        title: String,
        /// 自定义的css，会被添加到link下
        css: Option<String>,
    }
}

impl Debug for EpubHtml {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EpubHtml")
            .field("id", &self.id)
            .field("_file_name", &self._file_name)
            .field("media_type", &self.media_type)
            .field("_data", &self._data)
            .field("lang", &self.lang)
            .field("links", &self.links)
            .field("title", &self.title)
            .field("css", &self.css)
            .finish()
    }
}

impl EpubHtml {
    pub fn data(&mut self) -> Option<&[u8]> {
        let (id, origin) = if let Some(index) = self._file_name.find(|f| f == '#') {
            (
                Some(&self._file_name[(index + 1)..]),
                self._file_name[0..index].to_string(),
            )
        } else {
            (None, self.file_name().to_string())
        };
        let mut f = String::from(self._file_name.as_str());
        let prefixs = vec!["", common::EPUB, "EPUB/"];
        if self._data.is_none() && self.reader.is_some() && !f.is_empty() {
            for prefix in prefixs.iter() {
                // 添加 前缀再次读取
                f = format!("{prefix}{origin}");
                let s = self.reader.as_mut().unwrap();
                let d = (*s.borrow_mut()).read_string(f.as_str());
                match d {
                    Ok(v) => {
                        if let Ok((title, data)) = get_html_info(v.as_str(), id) {
                            if !title.is_empty() {
                                self.set_title(&title);
                            }
                            self.set_data(data);
                        }
                        break;
                    }
                    Err(IError::FileNotFound) => {}
                    Err(e) => {
                        break;
                    }
                }
            }
        }
        self._data.as_deref()
    }

    pub fn release_data(&mut self) {
        if let Some(data) = &mut self._data {
            data.clear();
        }
        self._data = None;
    }

    pub fn format(&mut self) -> Option<String> {
        self.data();
        Some(to_html(self, false))
    }

    ///
    /// 获取数据，当处于读模式时自动读取epub文件内容
    ///
    fn read_data(&mut self) -> Option<&[u8]> {
        let f = self._file_name.as_str();
        if self._data.is_none() && self.reader.is_some() && !f.is_empty() {
            // 可读
            // let d = self.reader.as_mut().unwrap().read_file(f);
            // if let Ok(v) = d {
            //     self.set_data(v);
            // }
        }

        self._data.as_deref()
    }

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

    pub fn links(&self) -> Option<std::slice::Iter<EpubLink>> {
        self.links.as_ref().map(|f| f.iter())
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

epub_base_field! {
///
/// 非章节资源
///
/// 例如css，字体，图片等
///
#[derive(Default,Clone)]
pub struct EpubAssets {
    version:String,
}
}

impl EpubAssets {
    pub fn with_version(&mut self, version: &str) {
        self.version.push_str(version);
    }

    pub fn data(&mut self) -> Option<&[u8]> {
        let mut f = String::from(self._file_name.as_str());
        if self._data.is_none() && self.reader.is_some() && !f.is_empty() {
            let prefixs = vec!["", common::EPUB, common::EPUB3];
            if self._data.is_none() && self.reader.is_some() && !f.is_empty() {
                for prefix in prefixs.iter() {
                    let s = self.reader.as_mut().unwrap();
                    // 添加 前缀再次读取
                    f = format!("{prefix}{}", self._file_name);
                    let d = (*s.borrow_mut()).read_file(f.as_str());
                    if let Ok(v) = d {
                        self.set_data(v);
                        break;
                    }
                }
            }
        }
        self._data.as_deref()
    }

    pub fn write_to<W: Write>(&mut self, writer: &mut W) -> IResult<()> {
        if let Some(data) = self.data() {
            writer.write_all(&data)?;
            writer.flush()?;
        }
        Ok(())
    }

    pub fn save_to(&mut self, file_path: &str) -> IResult<()> {
        let mut f = String::from(self._file_name.as_str());
        if self.reader.is_some() && !f.is_empty() {
            let prefixs = vec!["", common::EPUB, common::EPUB3];
            for prefix in prefixs.iter() {
                let s = self.reader.as_mut().unwrap();
                f = format!("{prefix}{}", self._file_name);
                let d: Result<(), IError> = (*s.borrow_mut()).read_to_path(f.as_str(), file_path);
                if d.is_ok() {
                    break;
                }
            }
        }
        Ok(())
    }

    pub fn release_data(&mut self) {
        if let Some(data) = &mut self._data {
            data.clear();
        }
        self._data = None;
    }
}

impl Debug for EpubAssets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EpubAssets")
            .field("id", &self.id)
            .field("_file_name", &self._file_name)
            .field("media_type", &self.media_type)
            .field("_data", &self._data)
            .field("reader_mode", &self.reader.is_some())
            .finish()
    }
}

// impl Clone for EpubAssets {
//     fn clone(&self) -> Self {
//         Self {
//             id: self.id.clone(),
//             _file_name: self._file_name.clone(),
//             media_type: self.media_type.clone(),
//             _data: self._data.clone(),
//             reader: self.reader.clone(),
//         }
//     }
// }
epub_base_field! {
///
/// 目录信息
///
/// 支持嵌套
///
#[derive(Default)]
pub struct EpubNav {
    /// 章节目录
    /// 如果需要序号需要调用方自行处理
    title: String,
    child: Vec<EpubNav>,
}
}
impl Debug for EpubNav {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EpubNav")
            .field("id", &self.id)
            .field("_file_name", &self._file_name)
            .field("media_type", &self.media_type)
            .field("_data", &self._data)
            .field("title", &self.title)
            .field("child", &self.child)
            .finish()
    }
}

impl Clone for EpubNav {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            _file_name: self._file_name.clone(),
            media_type: self.media_type.clone(),
            _data: self._data.clone(),
            reader: None,
            title: self.title.clone(),
            child: self.child.clone(),
        }
    }
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

    pub fn child(&self) -> &[EpubNav] {
        &self.child
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
    pub fn with_attr(mut self, key: &str, value: &str) -> Self {
        self.push_attr(key, value);
        self
    }
    pub fn push_attr(&mut self, key: &str, value: &str) {
        self.attr.insert(String::from(key), String::from(value));
    }
    pub fn with_text(mut self, text: &str) -> Self {
        self.set_text(text);
        self
    }

    pub fn set_text(&mut self, text: &str) {
        if let Some(t) = &mut self.text {
            t.clear();
            t.push_str(text);
        } else {
            self.text = Some(String::from(text));
        }
    }

    pub fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }

    pub fn attrs(&self) -> std::collections::hash_map::Iter<'_, String, String> {
        self.attr.iter()
    }

    pub fn get_attr(&self, key: &str) -> Option<&String> {
        self.attr.get(key)
    }
}

/// 书本
#[derive(Default)]
pub struct EpubBook {
    /// 上次修改时间
    last_modify: Option<String>,
    /// epub电子书创建者信息
    generator: Option<String>,
    /// 书本信息
    info: crate::common::BookInfo,
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
    /// 版本号
    version: String,
    /// 处于读模式
    reader: Option<Rc<RefCell<Box<dyn EpubReaderTrait>>>>,
    /// PREFIX
    pub(crate) prefix: String,
}

impl Display for EpubBook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f,"last_modify={:?},info={:?},meta={:?},nav={:?},assets={:?},chapters={:?},cover={:?},is in read mode={}",
        self.last_modify,
        self.info,
        self.meta,
        self.nav,
        self.assets,
        self.chapters,
        self.cover,
        self.reader.is_some()
    )
    }
}

impl EpubBook {
    iepub_derive::option_string_method!(info, creator);
    iepub_derive::option_string_method!(info, description);
    iepub_derive::option_string_method!(info, contributor);
    iepub_derive::option_string_method!(info, date);
    iepub_derive::option_string_method!(info, format);
    iepub_derive::option_string_method!(info, publisher);
    iepub_derive::option_string_method!(info, subject);
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
    iepub_derive::option_string_method!(last_modify);
    iepub_derive::option_string_method!(generator);
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
    pub fn with_title(mut self, title: &str) -> Self {
        self.set_title(title);
        self
    }
    pub fn identifier(&self) -> &str {
        self.info.identifier.as_str()
    }
    pub fn set_identifier(&mut self, identifier: &str) {
        self.info.identifier.clear();
        self.info.identifier.push_str(identifier);
    }
    pub fn with_identifier(mut self, identifier: &str) -> Self {
        self.set_identifier(identifier);
        self
    }

    ///
    /// 添加元数据
    ///
    /// # Examples
    ///
    /// ```
    /// use iepub::prelude::*;
    /// let mut epub = EpubBook::default();
    /// epub.add_meta(EpubMetaData::default().with_attr("k", "v").with_text("text"));
    /// ```
    ///
    pub fn add_meta(&mut self, meta: EpubMetaData) {
        self.meta.push(meta);
    }
    pub fn meta(&self) -> &[EpubMetaData] {
        &self.meta
    }

    pub fn get_meta_mut(&mut self, index: usize) -> Option<&mut EpubMetaData> {
        self.meta.get_mut(index)
    }
    pub fn get_meta(&self, index: usize) -> Option<&EpubMetaData> {
        self.meta.get(index)
    }

    pub fn meta_len(&self) -> usize {
        self.meta.len()
    }

    pub(crate) fn set_reader(&mut self, reader: Rc<RefCell<Box<dyn EpubReaderTrait>>>) {
        self.reader = Some(reader)
    }

    ///
    /// 添加目录
    ///
    #[inline]
    pub fn add_nav(&mut self, nav: EpubNav) {
        self.nav.push(nav);
    }

    pub fn add_assets(&mut self, mut assets: EpubAssets) {
        if let Some(r) = &self.reader {
            assets.reader = Some(Rc::clone(r));
        }
        self.assets.push(assets);
    }

    ///
    /// 查找章节
    ///
    /// [file_name] 不需要带有 EPUB 目录
    ///
    pub fn get_assets(&mut self, file_name: &str) -> Option<&mut EpubAssets> {
        self.assets.iter_mut().find(|s| s.file_name() == file_name)
        // .map(|f| {
        //     if let Some(r) = &self.reader {
        //         f.reader = Some(Rc::clone(r));
        //     }
        //     f
        // })
    }

    pub fn assets(&self) -> std::slice::Iter<EpubAssets> {
        self.assets.iter()
    }

    pub fn assets_mut(&mut self) -> std::slice::IterMut<EpubAssets> {
        self.assets.iter_mut()
    }

    pub fn add_chapter(&mut self, mut chap: EpubHtml) {
        if let Some(r) = &self.reader {
            chap.reader = Some(Rc::clone(r));
        }
        self.chapters.push(chap);
    }

    pub fn chapters_mut(&mut self) -> std::slice::IterMut<EpubHtml> {
        self.chapters.iter_mut()
    }

    pub fn chapters(&self) -> std::slice::Iter<EpubHtml> {
        self.chapters.iter()
    }

    ///
    /// 查找章节
    ///
    /// [file_name] 不需要带有 EPUB 目录
    ///
    pub fn get_chapter(&mut self, file_name: &str) -> Option<&mut EpubHtml> {
        self.chapters.iter_mut().find(|s| {
            return s.file_name() == file_name;
        })
    }
    pub fn set_version(&mut self, version: &str) {
        self.version.clear();
        self.version.push_str(version);
    }

    pub fn version(&mut self) -> &str {
        self.version.as_ref()
    }

    /// 获取目录
    pub fn nav(&self) -> &[EpubNav] {
        &self.nav
    }

    pub fn set_cover(&mut self, cover: EpubAssets) {
        self.cover = Some(cover);
    }

    pub fn cover(&self) -> Option<&EpubAssets> {
        self.cover.as_ref()
    }

    pub fn cover_mut(&mut self) -> Option<&mut EpubAssets> {
        self.cover.as_mut()
    }

    /// 读取完成后更新文章
    pub(crate) fn update_chapter(&mut self) {
        let f = flatten_nav(&self.nav);

        let mut map = HashMap::new();
        for (index, ele) in self.chapters.iter_mut().enumerate() {
            if let Some(v) = self.nav.iter().find(|f| f.file_name() == ele.file_name()) {
                ele.set_title(v.title());
            } else {
                // 如果 chapter 在 nav中不存在，有两种情况，一是cover之类的本身就不存在，二是epub3，在一个文件里使用id分章节
                let id_nav: Vec<&&EpubNav> = f
                    .iter()
                    .filter(|f| {
                        f.file_name().contains("#") && f.file_name().starts_with(ele.file_name())
                    })
                    .collect();
                if !id_nav.is_empty() {
                    // epub3,去除该 chap,重新填入
                    map.insert(index, id_nav);
                }
            }
        }
        // 修正章节
        let mut offset = 0;
        for (index, nav) in map {
            for ele in nav {
                let mut chap = EpubHtml::default()
                    .with_title(ele.title())
                    .with_file_name(ele.file_name());
                if let Some(r) = &self.reader {
                    chap.reader = Some(Rc::clone(r));
                }
                self.chapters.insert(index + offset, chap);
                offset = offset + 1;
            }
        }
    }

    pub(crate) fn update_assets(&mut self) {
        let version = self.version().to_string();
        for assets in self.assets_mut() {
            assets.with_version(&version);
        }
    }
}

/// 获取最低层级的目录
fn flatten_nav(nav: &[EpubNav]) -> Vec<&EpubNav> {
    let mut n = Vec::new();
    for ele in nav {
        if ele.child.is_empty() {
            n.push(ele);
        } else {
            n.append(&mut flatten_nav(&ele.child));
        }
    }
    n
}
pub(crate) trait EpubReaderTrait {
    fn read(&mut self, book: &mut EpubBook) -> IResult<()>;
    ///
    /// file epub中的文件目录
    ///
    fn read_file(&mut self, file_name: &str) -> IResult<Vec<u8>>;

    ///
    /// file epub中的文件目录
    ///
    fn read_string(&mut self, file_name: &str) -> IResult<String>;

    ///
    /// file epub中的文件目录
    ///
    fn read_to_path(&mut self, file_name: &str, file_path: &str) -> IResult<()>;
}

#[cfg(test)]
mod tests {

    use crate::prelude::*;

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
        book.set_version("2.0");
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
        book.set_date("29939");
        book.set_subject("subject");
        book.set_format("format");
        book.set_publisher("publisher");
        book.set_contributor("contributor");
        // epub.cover = Some(EpubAssets::default());

        let mut cover = EpubAssets::default();
        cover.set_file_name("cover.jpg");

        let data = vec![2];
        cover.set_data(data);

        book.set_cover(cover);

        // EpubWriter::write_to_file("file", &mut book).unwrap();

        EpubWriter::write_to_mem(&mut book, true).unwrap();

        // EpubWriter::<std::fs::File>write_to_file("../target/test.epub", &mut book).expect("write error");
    }
}
