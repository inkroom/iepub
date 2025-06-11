use crate::common::time_format;
use crate::prelude::*;

///
/// 简化epub构建
///
pub struct EpubBuilder {
    book: EpubBook,

    /// 是否自定义导航
    /// 默认为false
    custome_nav: bool,
    append_title: bool,
    nav: Vec<EpubNav>,
    /// 自动创建封面
    /// 默认为false
    auto_gen_cover: bool,
    /// 字体文件位置
    /// 用于生成封面图片
    font: Option<String>,
    /// 字体文件内容
    /// 用于生成封面图片
    font_byte: Option<Vec<u8>>,
}

impl Default for EpubBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl EpubBuilder {
    pub fn new() -> Self {
        EpubBuilder {
            book: EpubBook::default(),
            custome_nav: false,
            nav: Vec::new(),
            append_title: true,
            auto_gen_cover: false,
            font: None,
            font_byte: None,
        }
    }
    /// 是否添加标题，默认true
    pub fn append_title(mut self, append_title: bool) -> Self {
        self.append_title = append_title;
        self
    }

    pub fn with_version(mut self, version: &str) -> Self {
        self.book.set_version(version);
        self
    }

    pub fn with_title(mut self, title: &str) -> Self {
        self.book.set_title(title);
        self
    }

    pub fn with_identifier(mut self, identifier: &str) -> Self {
        self.book.set_identifier(identifier);
        self
    }
    pub fn with_creator(mut self, creator: &str) -> Self {
        self.book.set_creator(creator);
        self
    }
    pub fn with_description(mut self, description: &str) -> Self {
        self.book.set_description(description);
        self
    }
    pub fn with_contributor(mut self, contributor: &str) -> Self {
        self.book.set_contributor(contributor);
        self
    }
    pub fn with_date(mut self, date: &str) -> Self {
        self.book.set_date(date);
        self
    }
    pub fn with_format(mut self, format: &str) -> Self {
        self.book.set_format(format);
        self
    }
    pub fn with_publisher(mut self, publisher: &str) -> Self {
        self.book.set_publisher(publisher);
        self
    }
    pub fn with_subject(mut self, subject: &str) -> Self {
        self.book.set_subject(subject);
        self
    }

    pub fn with_last_modify(mut self, last_modify: &str) -> Self {
        self.book.set_last_modify(last_modify);
        self
    }

    pub fn custome_nav(mut self, value: bool) -> Self {
        self.custome_nav = value;
        self
    }
    /// 设置自动创建封面
    pub fn auto_gen_cover(mut self, value: bool) -> Self {
        self.auto_gen_cover = value;
        self
    }

    /// 设置字体文件路径
    pub fn with_font(mut self, font_file: &str) -> Self {
        self.font = Some(font_file.to_string());
        self
    }

    /// 设置字体文件内容
    pub fn with_font_bytes(mut self, font: Vec<u8>) -> Self {
        self.font_byte = Some(font);
        self
    }

    ///
    /// 添加 metadata
    ///
    /// 每一对kv都会生成新的meta元素
    ///
    pub fn metadata(mut self, key: &str, value: &str) -> Self {
        self.book
            .add_meta(EpubMetaData::default().with_attr(key, value));
        self
    }

    ///
    /// 添加资源文件
    ///
    pub fn add_assets(mut self, file_name: &str, data: Vec<u8>) -> Self {
        self.book.add_assets(
            EpubAssets::default()
                .with_file_name(file_name)
                .with_data(data),
        );
        self
    }

    ///
    /// 设置封面
    ///
    /// [file_name] epub中的文件名，不是本地文件名
    /// [data] 数据
    ///
    pub fn cover(mut self, file_name: &str, data: Vec<u8>) -> Self {
        self.book.set_cover(
            EpubAssets::default()
                .with_file_name(file_name)
                .with_data(data),
        );

        self
    }

    ///
    /// 添加文章
    ///
    /// 注意，将会按照文章添加顺序生成一个简易的目录页
    ///
    /// 如果需要更为复杂的自定义目录，需要调用 custome_nav(true) 方法
    ///
    ///
    pub fn add_chapter(mut self, chapter: EpubHtml) -> Self {
        self.book.add_chapter(chapter);
        self
    }

    ///
    /// 添加目录导航
    ///
    pub fn add_nav(mut self, nav: EpubNav) -> Self {
        self.nav.push(nav);
        self
    }

    fn gen_nav(&mut self) {
        if self.custome_nav {
            for ele in &mut self.nav {
                self.book.add_nav(ele.clone());
            }
        } else {
            // 生成简单目录
            let mut nav: Vec<EpubNav> = Vec::new();
            for ele in self.book.chapters() {
                // 不能一次循环直接添加，因为会出现重复借用
                nav.push(
                    EpubNav::default()
                        .with_title(ele.title())
                        .with_file_name(ele.file_name()),
                );
            }

            for ele in nav {
                self.book.add_nav(ele);
            }
        }
    }

    fn gen_last_modify(&mut self) {
        if self.book.last_modify().is_none() {
            self.book.set_last_modify(&time_format());
        }
    }

    fn gen_cover(&mut self) -> IResult<()> {
        if self.auto_gen_cover && self.book.cover().is_none() {
            let font_bytes = match self.font_byte.clone() {
                Some(v) => v,
                None => self
                    .font
                    .as_ref()
                    .and_then(|f| std::fs::read(f.as_str()).ok())
                    .unwrap_or_else(|| Vec::new()),
            };
            if font_bytes.is_empty() {
                return Err(IError::Cover("no font set".to_string()));
            }
            let c = crate::cover::gen_cover(self.book.title(), &font_bytes)?;

            self.book.set_cover(
                EpubAssets::default()
                    .with_file_name("cover.jpeg")
                    .with_data(c),
            );
        }
        Ok(())
    }

    ///
    /// 返回epub实例，将会消耗构造器所有权
    ///
    ///
    pub fn book(mut self) -> IResult<EpubBook> {
        self.gen_last_modify();
        self.gen_nav();
        self.gen_cover()?;
        Ok(self.book)
    }

    ///
    /// 输出到文件
    ///
    pub fn file(mut self, file: &str) -> IResult<()> {
        self.gen_last_modify();
        self.gen_nav();
        self.gen_cover()?;

        std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(file)
            .map_or_else(
                |e| Err(IError::Io(e)),
                |f| Ok(EpubWriter::new(f).with_append_title(self.append_title)),
            )
            .and_then(|mut w| w.write(&mut self.book))
    }

    ///
    /// 输出到内存
    ///
    pub fn mem(mut self) -> IResult<Vec<u8>> {
        self.gen_last_modify();
        self.gen_nav();
        self.gen_cover()?;
        let mut v = std::io::Cursor::new(Vec::new());
        EpubWriter::new(&mut v)
            .with_append_title(self.append_title)
            .write(&mut self.book)?;

        Ok(v.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use crate::prelude::EpubHtml;

    use super::EpubBuilder;

    #[test]
    fn test() {
        let f = if std::path::Path::new("target").exists() {
            "target/SourceHanSansSC-Bold.otf"
        } else {
            "../target/SourceHanSansSC-Bold.otf"
        };
        let font = std::fs::read(f).or_else( |_|{
             crate::common::tests::get_req("https://github.com/adobe-fonts/source-han-serif/raw/refs/heads/release/SubsetOTF/CN/SourceHanSerifCN-Bold.otf").send().map(|v|{
                let s =v.as_bytes().to_vec();
                println!("{} {:?}",s.len(),v.headers);
                if &s.len().to_string() != v.headers.get("content-length").unwrap_or(&String::new()) && v.status_code !=200 {
                    panic!("字体文件下载失败");
                }
                let _ = std::fs::write(f, s.clone());
                s
            })
        }).unwrap();

        EpubBuilder::default()
            .auto_gen_cover(if cfg!(feature = "cover") { true } else { false })
            .with_font_bytes(font)
            .with_title("书名")
            .with_creator("作者")
            .with_date("2024-03-14")
            .with_description("一本好书")
            .with_identifier("isbn")
            .with_publisher("行星出版社")
            .add_chapter(
                EpubHtml::default()
                    .with_file_name("0.xml")
                    .with_data("<p>锻炼</p>".to_string().as_bytes().to_vec()),
            )
            .add_assets("1.css", "p{color:red}".to_string().as_bytes().to_vec())
            .metadata("s", "d")
            .metadata("h", "m")
            .file(if std::path::Path::new("target").exists() {
                "target/build.epub"
            } else {
                "../target/build.epub"
            })
            .unwrap();
    }
}
