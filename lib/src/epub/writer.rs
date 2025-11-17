use std::{
    fs::File,
    io::{Seek, Write},
    path::Path,
};

use zip::ZipWriter;

use crate::prelude::*;

use super::{
    common,
    core::info,
    html::{to_html, to_nav_html, to_opf, to_toc_xml},
};

impl From<zip::result::ZipError> for IError {
    fn from(value: zip::result::ZipError) -> Self {
        match value {
            zip::result::ZipError::Io(io) => IError::Io(io),
            zip::result::ZipError::InvalidArchive(v) => IError::InvalidArchive(v),
            zip::result::ZipError::UnsupportedArchive(v) => IError::UnsupportedArchive(v),
            zip::result::ZipError::InvalidPassword => IError::InvalidPassword,
            zip::result::ZipError::FileNotFound => IError::FileNotFound,
            _ => IError::Unknown,
        }
    }
}
///
/// epub输出实现，可通过实现该trait从而自定义输出方案。
///
/// 具体实现应该是写入到zip文件
///
pub(crate) trait EpubWriterTrait {
    ///
    /// file epub中的文件目录
    /// data 要写入的数据
    ///
    fn write_file(&mut self, file: &str, data: &[u8]) -> IResult<()>;
}
///
/// 写入到文件
///
pub struct EpubWriter<T: Write + Seek> {
    pub(crate) inner: zip::ZipWriter<T>,
    pub(crate) append_title: bool,
}
static CONTAINER_XML: &str = r#"<?xml version='1.0' encoding='utf-8'?>
<container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0">
  <rootfiles>
    <rootfile media-type="application/oebps-package+xml" full-path="{opf}"/>
  </rootfiles>
</container>
"#;

impl EpubWriter<File> {
    /// 写入文件
    pub fn write_to_file<P: AsRef<Path>>(
        file: P,
        book: &mut EpubBook,
        append_title: bool,
    ) -> IResult<()> {
        std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(file)
            .map_or_else(
                |e| Err(IError::Io(e)),
                |f| Ok(EpubWriter::new(f).with_append_title(append_title)),
            )
            .and_then(|mut w| w.write(book))
    }
}

impl EpubWriter<std::io::Cursor<Vec<u8>>> {
    /// 写入内存
    pub fn write_to_mem(book: &mut EpubBook, append_title: bool) -> IResult<Vec<u8>> {
        let mut v = std::io::Cursor::new(Vec::new());
        EpubWriter::new(&mut v)
            .with_append_title(append_title)
            .write(book)?;

        Ok(v.into_inner())
    }
}

impl<T: Write + Seek> EpubWriter<T> {
    pub fn new(inner: T) -> Self {
        EpubWriter {
            inner: ZipWriter::new(inner),
            append_title: true,
        }
    }

    pub fn with_append_title(mut self, append_title: bool) -> Self {
        self.append_title = append_title;
        self
    }

    pub fn write(&mut self, book: &mut EpubBook) -> IResult<()> {
        self.write_cover(book)?;

        self.write_base(book)?;
        self.write_assets(book)?;
        self.write_chapters(book)?;
        self.write_nav(book)?;

        Ok(())
    }

    /// 生成章节页
    fn gen_cover_chapter(&mut self, book: &mut EpubBook) {
        if let Some(cover) = book.cover() {
            // 生成 cover 页
            let src = crate::path::Path::system("text").releative(cover.file_name());
            let c = EpubHtml::default()
                .with_file_name("text/cover.xhtml")
                .with_title("cover")
                .with_data(format!(r##"<img src="{src}"/>"##).as_bytes().to_vec());

            book.cover_chapter = Some(c);
        }
    }

    /// 写入基础的文件
    fn write_base(&mut self, book: &mut EpubBook) -> IResult<()> {
        if book.version().is_empty() {
            book.set_version("2.0");
        }
        self.write_file(
            "META-INF/container.xml",
            CONTAINER_XML.replace("{opf}", common::OPF).as_bytes(),
        )?;
        self.write_file("mimetype", "application/epub+zip".as_bytes())?;

        self.write_file(
            common::OPF,
            to_opf(
                book,
                format!("{}-{}", info::PROJECT_NAME, info::PKG_VERSION).as_str(),
            )
            .as_bytes(),
        )?;

        Ok(())
    }

    /// 写入资源文件
    fn write_assets(&mut self, book: &mut EpubBook) -> IResult<()> {
        let m = book.assets_mut();
        for ele in m {
            if ele.data_mut().is_none() {
                continue;
            }
            self.write_file(
                format!("{}{}", common::EPUB, ele.file_name()).as_str(),
                ele.data_mut().unwrap(),
            )?;
        }
        Ok(())
    }

    /// 写入章节文件
    fn write_chapters(&mut self, book: &mut EpubBook) -> IResult<()> {
        let dir = book.direction.as_ref().cloned();
        let chap = book.chapters_mut();
        for ele in chap {
            if ele.data_mut().is_none() {
                continue;
            }

            let html = to_html(ele, self.append_title, &dir);

            self.write_file(
                format!("{}{}", common::EPUB, ele.file_name()).as_str(),
                html.as_bytes(),
            )?;
        }

        Ok(())
    }
    /// 写入目录
    fn write_nav(&mut self, book: &mut EpubBook) -> IResult<()> {
        // 目录包括两部分，一是自定义的用于书本导航的html，二是epub规范里的toc.ncx文件
        self.write_file(
            common::NAV,
            to_nav_html(
                book.title(),
                book.nav(),
                book.language().unwrap_or(""),
                &book.direction,
            )
            .as_bytes(),
        )?;
        self.write_file(common::TOC, to_toc_xml(book.title(), book.nav()).as_bytes())?;

        Ok(())
    }

    ///
    /// 生成封面
    ///
    /// 拷贝资源文件以及生成对应的xhtml文件
    ///
    fn write_cover(&mut self, book: &mut EpubBook) -> IResult<()> {
        book.cover_chapter = if let Some(cover) = book.cover_mut() {
            self.write_file(
                format!("{}{}", common::EPUB, cover.file_name()).as_str(),
                cover.data_mut().as_ref().unwrap(),
            )?;

            // 生成 cover 页
            let src = cover.file_name();
            let mut c = EpubHtml::default()
                .with_file_name(common::COVER.replace(common::EPUB, ""))
                .with_title("cover")
                .with_css("html,body,div,img{width: 100%;}")
                .with_data(format!(r##"<body><div><img src="{src}"/></div></body>"##).as_bytes().to_vec());

            self.write_file(
                common::COVER,
                to_html(&mut c, false, &book.direction).as_bytes(),
            )?;
            Some(c)
        } else {
            None
        };
        Ok(())
    }
}

impl<T: Write + Seek> EpubWriterTrait for EpubWriter<T> {
    fn write_file(&mut self, file: &str, data: &[u8]) -> IResult<()> {
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .unix_permissions(0o755);
        self.inner.start_file(file, options)?;
        self.inner.write_all(data)?;
        Ok(())
    }
}
