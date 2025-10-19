use std::io::Write;

use crate::{
    cli::arg::{self},
    exec_err, msg,
};
use iepub::prelude::*;

// 是否覆盖文件
fn is_overiade(global_opts: &[arg::ArgOption], opts: &[arg::ArgOption]) -> bool {
    global_opts
        .iter()
        .find(|s| s.key == "y")
        .is_some_and(|_| true)
        || opts.iter().find(|s| s.key == "y").is_some_and(|_| true)
}

///
/// 获取输入
///
fn get_single_input(message: &str) -> Result<String, IError> {
    print!("{} ", message);
    std::io::stdout().flush()?;
    use std::io::BufRead;
    let mut buffer = String::new();
    let stdin = std::io::stdin();
    let mut handle = stdin.lock();

    handle.read_line(&mut buffer)?;
    Ok(buffer.trim().to_string())
}
/// 创建一个命令，定死了代码基本结构
macro_rules! create_command {
    // create_command!(结构体名称, "命令名称",{ arg::CommandOptionDef{} }, exec函数, 额外的成员函数 ),如果没有额外的成员函数，最后也需要以逗号结尾，所以最后部分代码应该是: ,);
    ($name:ident, $com:expr, $def:block, $exe:item, $($fun:item),*) => {
        #[derive(Default)]
        pub(crate) struct $name;
        impl $name {
            pub(crate) fn def() -> arg::CommandOptionDef {
                $def
            }

            $($fun)*
        }
        impl Command for $name {
            fn name(&self) -> String {
                $com.to_string()
            }

            $exe

        }
    };
    // 这里可以省掉最后的逗号，以 ); 结尾即可
    ($name:ident,$com:expr,$def:block,$exe:item) => {
        create_command!($name,$com,$def,$exe,);
    };
}
fn write_file(path: &str, data: &[u8]) {
    let _ = std::fs::File::options()
        .truncate(true)
        .create(true)
        .write(true)
        .open(path)
        .and_then(|mut f| f.write_all(data))
        .map_err(|e| exec_err!("err: {}", e));
}

fn create_dir(path: &str) {
    if !std::path::Path::new(path).exists() {
        msg!("creating dir {}", path);
        // 创建目录
        match std::fs::create_dir_all(path) {
            Ok(_) => {}
            Err(e) => {
                exec_err!("create dir {} fail, because {}", path, e);
            }
        };
    }
}

enum OwnBook {
    EPUB(EpubBook),
    MOBI(MobiBook),
}

fn read_book(file: &str) -> IResult<OwnBook> {
    msg!("reading file {}", file);
    if std::fs::File::open(file)
        .map_err(|_| false)
        .and_then(|mut f| iepub::prelude::check::is_epub(&mut f).map_err(|_| false))
        .unwrap_or(false)
    {
        read_from_file(file).map(OwnBook::EPUB)
    } else if std::fs::File::open(file)
        .map_err(|_| false)
        .and_then(|mut f| iepub::prelude::check::is_mobi(&mut f).map_err(|_| false))
        .unwrap_or(false)
    {
        let f = std::fs::File::open(file)?;
        iepub::prelude::MobiReader::new(f)
            .and_then(|mut f| f.load())
            .map(OwnBook::MOBI)
    } else {
        Err(IError::UnsupportedArchive("不支持的格式"))
    }
}

pub(crate) mod epub {
    use crate::cli::command::get_single_input;
    use crate::cli::command::is_overiade;
    use crate::cli::command::write_file;
    use crate::exec_err;
    use crate::Book;
    use iepub::prelude::adapter::add_into_epub;
    use iepub::prelude::adapter::epub_to_mobi;
    use iepub::prelude::appender::write_metadata;
    
    
    use iepub::prelude::EpubBuilder;
    use iepub::prelude::EpubNav;
    
    
    use iepub::prelude::MobiWriter;

    use crate::{
        cli::arg::{self, ArgOption, CommandOptionDef, OptionDef, OptionType},
        msg, Command,
    };

    use super::read_book;
    use super::OwnBook;
    create_command!(
        Concat,
        "concat",
        {
            arg::CommandOptionDef {
                command: "concat".to_string(),
                support_args: 0,
                desc: "合并，基础信息以第一本为准".to_string(),
                opts: vec![
                    OptionDef::create(
                        "child",
                        "其他电子书，不必包括-i参数对应的电子书",
                        OptionType::Array,
                        true,
                    ),
                    OptionDef::create("out", "输出文件位置", OptionType::String, true),
                    OptionDef::create("skip", "跳过指定目录数", OptionType::String, false),
                    OptionDef::create("cover", "封面图片", OptionType::String, false),
                    OptionDef::create("title", "标题", OptionType::String, false),
                    OptionDef::create("author", "作者", OptionType::String, false),
                    OptionDef::create("isbn", "isbn", OptionType::String, false),
                    OptionDef::create("publisher", "出版社", OptionType::String, false),
                    OptionDef::create(
                        "date",
                        "出版日期，格式为:2024-06-28T03:07:07UTC",
                        OptionType::String,
                        false,
                    ),
                    OptionDef::create("desc", "简介", OptionType::String, false),
                    OptionDef::create("format", "format", OptionType::String, false),
                    OptionDef::create("subject", "subject", OptionType::String, false),
                    OptionDef::create("contributor", "contributor", OptionType::String, false),
                ],
            }
        },
        fn exec(
            &self,
            book: &mut Book,
            global_opts: &[ArgOption],
            opts: &[ArgOption],
            _args: &[String],
        ) {
            if let Book::EPUB(book) = book {
                let mut builder = EpubBuilder::new()
                    .with_title(book.title())
                    .custome_nav(true);
                if let Some(v) = book.creator() {
                    builder = builder.with_creator(v);
                }
                if let Some(v) = book.description() {
                    builder = builder.with_description(v);
                }
                if let Some(v) = book.publisher() {
                    builder = builder.with_publisher(v);
                }
                builder = builder.with_title(book.title());
                if let Some(v) = book.date() {
                    builder = builder.with_date(v);
                }
                if let Some(v) = book.contributor() {
                    builder = builder.with_contributor(v);
                }
                if let Some(v) = book.format() {
                    builder = builder.with_format(v);
                }
                if let Some(v) = book.subject() {
                    builder = builder.with_subject(v);
                }

                if let Some(c) = book.cover_mut() {
                    let f = c.file_name().to_string();
                    if let Some(v) = c.data_mut() {
                        builder = builder.cover(f.as_str(), v.to_vec());
                    }
                }

                for ele in opts {
                    if ele.value.as_ref().is_none() {
                        continue;
                    }
                    let v = ele.value.as_ref().unwrap().as_str();
                    match ele.key.as_str() {
                        "title" => builder = builder.with_title(v),
                        "author" => builder = builder.with_creator(v),
                        "isbn" => builder = builder.with_identifier(v),
                        "publisher" => builder = builder.with_publisher(v),
                        "date" => builder = builder.with_date(v),
                        "desc" => builder = builder.with_description(v),
                        "format" => builder = builder.with_format(v),
                        "subject" => builder = builder.with_subject(v),
                        "contributor" => builder = builder.with_contributor(v),
                        "cover" => {
                            builder = builder.cover(
                                "image/cover.png",
                                std::fs::read(v).expect("read cover error"),
                            )
                        }
                        _ => {}
                    }
                }

                if let Some(bs) = opts
                    .iter()
                    .find(|f| f.key == "child")
                    .and_then(|f| f.values.clone())
                {
                    let skip = opts
                        .iter()
                        .find(|f| f.key == "skip")
                        .and_then(|f| f.value.clone())
                        .unwrap_or("0".to_string())
                        .parse()
                        .unwrap();

                    let (mut builder, mut len, mut assets_len) =
                        add_into_epub(builder, book, 0, 0, skip).unwrap();

                    for ele in bs {
                        let f = read_book(ele.as_str()).unwrap();
                        match f {
                            OwnBook::EPUB(mut epub_book) => {
                                let v =
                                    add_into_epub(builder, &mut epub_book, len, assets_len, skip)
                                        .unwrap();
                                builder = v.0;
                                len = v.1;
                                assets_len = v.2;
                            }
                            OwnBook::MOBI(mobi_book) => todo!(),
                        }
                    }
                    if let Some(path) = opts
                        .iter()
                        .find(|f| f.key == "out")
                        .and_then(|f| f.value.clone())
                    {
                        if std::path::Path::new(&path).exists()
                            && !is_overiade(global_opts, opts)
                            && get_single_input("Override file？(y/n)")
                                .unwrap()
                                .to_lowercase()
                                != "y"
                        {
                            return;
                        }
                        msg!("writing book to {}", path);
                        builder.file(path.as_str()).unwrap();
                    }
                }
            }
        }
    );

    create_command!(
        BookInfoGetter,
        "get-info",
        {
            arg::CommandOptionDef {
                command: "get-info".to_string(),
                support_args: 0,
                desc: "提取数据元数据".to_string(),
                opts: vec![
                    OptionDef::create("title", "标题", OptionType::NoParamter, false),
                    OptionDef::create("author", "作者", OptionType::NoParamter, false),
                    OptionDef::create("isbn", "isbn", OptionType::NoParamter, false),
                    OptionDef::create("publisher", "出版社", OptionType::NoParamter, false),
                    OptionDef::create("date", "出版日期", OptionType::NoParamter, false),
                    OptionDef::create("desc", "简介", OptionType::NoParamter, false),
                    OptionDef::create("format", "format", OptionType::NoParamter, false),
                    OptionDef::create("subject", "subject", OptionType::NoParamter, false),
                    OptionDef::create("contributor", "contributor", OptionType::NoParamter, false),
                    OptionDef::create("modify", "最后修改时间", OptionType::NoParamter, false),
                    OptionDef::create("generator", "电子书创建者", OptionType::NoParamter, false),
                    OptionDef::create("all", "所有元数据", OptionType::NoParamter, false),
                ],
            }
        },
        fn exec(
            &self,
            book: &mut Book,
            _global_opts: &[ArgOption],
            opts: &[ArgOption],
            _args: &[String],
        ) {
            if let Book::EPUB(book) = book {
                for ele in opts {
                    match ele.key.as_str() {
                        "title" => println!("{}", book.title()),
                        "author" => println!("{}", book.creator().unwrap_or("")),
                        "isbn" => println!("{}", book.identifier()),
                        "publisher" => println!("{}", book.publisher().unwrap_or("")),
                        "date" => println!("{}", book.date().unwrap_or("")),
                        "desc" => println!("{}", book.description().unwrap_or("")),
                        "format" => println!("{}", book.format().unwrap_or("")),
                        "subject" => println!("{}", book.subject().unwrap_or("")),
                        "contributor" => println!("{}", book.contributor().unwrap_or("")),
                        "modify" => println!("{}", book.last_modify().unwrap_or("")),
                        "generator" => println!("{}", book.generator().unwrap_or("")),
                        "all" => {
                            println!("title: {}", book.title());
                            println!("author: {}", book.creator().unwrap_or(""));
                            println!("isbn: {}", book.identifier());
                            println!("publisher: {}", book.publisher().unwrap_or(""));
                            println!("date: {}", book.date().unwrap_or(""));
                            println!("desc: {}", book.description().unwrap_or(""));
                            println!("format: {}", book.format().unwrap_or(""));
                            println!("subject: {}", book.subject().unwrap_or(""));
                            println!("contributor: {}", book.contributor().unwrap_or(""));
                            println!("modify: {}", book.last_modify().unwrap_or(""));
                            println!("generator: {}", book.generator().unwrap_or(""));
                        }
                        _ => {}
                    }
                }
            }
        }
    );

    create_command!(
        GetCover,
        "get-cover",
        {
            arg::CommandOptionDef {
                command: String::from("get-cover"),
                desc: "提取电子书封面, 例如get-cover 1.jpg，输出到1.jpg，不传将输出到默认文件名"
                    .to_string(),
                support_args: -1,
                opts: vec![OptionDef::create(
                    "y",
                    "是否覆盖输出文件",
                    OptionType::NoParamter,
                    false,
                )],
            }
        },
        fn exec(
            &self,
            book: &mut Book,
            global_opts: &[ArgOption],
            opts: &[ArgOption],
            args: &[String],
        ) {
            if let Book::EPUB(book) = book {
                let cover = book.cover_mut().unwrap_or_else(|| {
                    exec_err!("电子书没有封面");
                });
                let is_over = is_overiade(global_opts, opts);

                if args.is_empty() {
                    let mut path = String::new();
                    path.push_str(cover.file_name());

                    if std::path::Path::new(path.as_str()).exists()
                        && !is_over
                        && get_single_input("Override file？(y/n)")
                            .unwrap()
                            .to_lowercase()
                            != "y"
                    {
                        return;
                    }
                    msg!("writing cover to {}", path);

                    let data = cover.data_mut().unwrap();
                    write_file(path.as_str(), data);
                }

                for path in args {
                    if std::path::Path::new(&path).exists()
                        && !is_over
                        && get_single_input("Override file？(y/n)")
                            .unwrap()
                            .to_lowercase()
                            != "y"
                    {
                        continue;
                    }
                    msg!("writing cover to {}", path);
                    write_file(path, cover.data_mut().as_ref().unwrap());
                }
            }
        },
    );

    create_command!(
        NavScanner,
        "nav",
        {
            CommandOptionDef {
                command: "nav".to_string(),
                desc: "导航".to_string(),
                support_args: 0,
                opts: vec![OptionDef::create(
                    "s",
                    "输出目录对应文件名",
                    OptionType::NoParamter,
                    false,
                )],
            }
        },
        fn exec(
            &self,
            book: &mut Book,
            _global_opts: &[ArgOption],
            opts: &[ArgOption],
            _args: &[String],
        ) {
            if let Book::EPUB(book) = book {
                let print_href = opts.iter().find(|s| s.key == "s").is_some_and(|_| true);
                for ele in book.nav() {
                    self.print_nav(0, ele, print_href);
                }
            }
        },
        fn print_dec(&self, dec: i32) {
            for _ in 0..dec {
                print!(" ");
            }
        },
        fn print_nav(&self, dec: i32, nav: &EpubNav, print_href: bool) {
            self.print_dec(dec);
            if print_href {
                println!("{} href=[{}]", nav.title(), nav.file_name());
            } else {
                println!("{}", nav.title());
            }
            for ele in nav.child() {
                self.print_nav(dec + 2, ele, print_href);
            }
        }
    );

    create_command!(
        GetImage,
        "get-image",
        {
            arg::CommandOptionDef {
        command: "get-image".to_string(),
        desc: "提取图片".to_string(),
        support_args: 0,
        opts: vec![
            OptionDef::create("d", "输出目录", OptionType::String,true),
            OptionDef::over(),
            OptionDef::create("p", "文件名前缀，例如-d out -p image,文件将会被写入到 out/image01.jpg，原有文件名将会被忽略", OptionType::String,false),
        ],
    }
        },
        fn exec(
            &self,
            book: &mut Book,
            global_opts: &[ArgOption],
            opts: &[ArgOption],
            _args: &[String],
        ) {
            if let Book::EPUB(book) = book {
                let dir_o = opts
                    .iter()
                    .find(|s| s.key == "d")
                    .and_then(|f| f.value.as_ref());
                let is_over = is_overiade(global_opts, opts);

                let prefix = opts
                    .iter()
                    .find(|s| s.key == "p")
                    .and_then(|f| f.value.as_ref());
                let mut file_size = 1;
                if let Some(dir) = dir_o {
                    for ele in book.assets_mut() {
                        let name = ele.file_name().to_lowercase();
                        if name.ends_with(".jpg")
                            || name.ends_with(".jpeg")
                            || name.ends_with(".gif")
                            || name.ends_with(".png")
                            || name.ends_with(".webp")
                            || name.ends_with(".svg")
                        {
                            let mut file = format!("{dir}/{}", ele.file_name());
                            if let Some(p) = prefix {
                                // 有前缀
                                file = format!(
                                    "{dir}/{p}{}{}",
                                    file_size,
                                    &name[name.rfind('.').unwrap_or(0)..]
                                );
                                file_size += 1;
                            }
                            let n_dir = &file[0..file.rfind('/').unwrap_or(0)];
                            if !std::path::Path::new(n_dir).exists() {
                                msg!("creating dir {}", n_dir);
                                // 创建目录
                                match std::fs::create_dir_all(n_dir) {
                                    Ok(_) => {}
                                    Err(e) => {
                                        eprintln!("create dir {} fail, because {}", n_dir, e);
                                        continue;
                                    }
                                };
                            }

                            // 判断文件是否存在

                            if std::path::Path::new(&file).exists()
                                && !is_over
                                && get_single_input("Override file？(y/n)")
                                    .unwrap()
                                    .to_lowercase()
                                    != "y"
                            {
                                continue;
                            }
                            msg!("writing file to {}", file);
                            // 写入文件
                            write_file(&file, ele.data_mut().unwrap());
                        }
                    }
                }
            }
        },
    );

    create_command!(
        GetChapter,
        "get-chapter",
        {
            arg::CommandOptionDef {
                command: "get-chapter".to_string(),
                desc: "提取章节".to_string(),
                support_args: 0,
                opts: vec![
                    OptionDef::create(
                        "c",
                        "文件路径，可以从nav命令中获取",
                        OptionType::Array,
                        true,
                    ),
                    OptionDef::create(
                        "d",
                        "输出目录，没有该参数则直接输出到终端",
                        OptionType::String,
                        false,
                    ),
                    OptionDef::over(),
                    OptionDef::create(
                        "b",
                        "只输出body部分，否则输出完整的xhtml(可能跟原文有所区别)",
                        OptionType::NoParamter,
                        false,
                    ),
                ],
            }
        },
        fn exec(
            &self,
            book: &mut Book,
            global_opts: &[ArgOption],
            opts: &[ArgOption],
            _args: &[String],
        ) {
            if let Book::EPUB(book) = book {
                let dir = opts
                    .iter()
                    .find(|f| f.key == "d")
                    .and_then(|f| f.value.as_ref());

                let chaps: Vec<&String> = opts
                    .iter()
                    .filter(|s| s.key == "c" && s.values.is_some())
                    .flat_map(|f| f.values.as_ref().unwrap())
                    .collect();

                let is_over = is_overiade(global_opts, opts);

                let print_body = opts.iter().any(|f| f.key == "b");

                for ele in chaps {
                    if let Some(chap) = book.get_chapter_mut(ele) {
                        if let Some(d) = dir {
                            let mut p_dir: std::path::PathBuf =
                                std::path::Path::new(&d).join(chap.file_name());
                            p_dir.pop(); // 获取在文件所在目录了

                            if !p_dir.exists() {
                                msg!("creating dir {:?}", p_dir);
                                match std::fs::create_dir_all(&p_dir) {
                                    Ok(_) => {}
                                    Err(e) => {
                                        exec_err!(
                                            "mkdir {:?} fail, because {}",
                                            p_dir,
                                            e.to_string()
                                        );
                                    }
                                };
                            }
                            let file = format!("{}/{}", d, chap.file_name());

                            if std::path::Path::new(&file).exists()
                                && !is_over
                                && get_single_input("Override file？(y/n)")
                                    .unwrap()
                                    .to_lowercase()
                                    != "y"
                            {
                                continue;
                            }
                            if print_body {
                                write_file(file.as_str(), chap.data_mut().unwrap())
                            } else {
                                let d = chap.format().unwrap_or("".to_string());
                                write_file(file.as_str(), d.as_bytes());
                            }
                        } else {
                            // 直接输出到终端
                            println!(
                                "{}",
                                if print_body {
                                    String::from_utf8(chap.data_mut().unwrap().to_vec()).unwrap()
                                } else {
                                    chap.format().unwrap_or("".to_string())
                                }
                            );
                        }
                    } else {
                        exec_err!("chap {} not exists", ele);
                    }
                }
            }
        },
    );

    create_command!(
        BookInfoSetter,
        "set-info",
        {
            CommandOptionDef {
                command: "set-info".to_string(),
                desc: "设置电子书元数据".to_string(),
                support_args: 0,
                opts: vec![
                    OptionDef::create("title", "标题", OptionType::String, false),
                    OptionDef::create("author", "作者", OptionType::String, false),
                    OptionDef::create("isbn", "isbn", OptionType::String, false),
                    OptionDef::create("publisher", "出版社", OptionType::String, false),
                    OptionDef::create(
                        "date",
                        "出版日期，格式为:2024-06-28T03:07:07UTC",
                        OptionType::String,
                        false,
                    ),
                    OptionDef::create("desc", "简介", OptionType::String, false),
                    OptionDef::create("format", "format", OptionType::String, false),
                    OptionDef::create("subject", "subject", OptionType::String, false),
                    OptionDef::create("contributor", "contributor", OptionType::String, false),
                ],
            }
        },
        fn exec(
            &self,
            book: &mut Book,
            global_opts: &[ArgOption],
            opts: &[ArgOption],
            _args: &[String],
        ) {
            if let Book::EPUB(book) = book {
                // 修改数据
                for ele in opts {
                    let v = ele.value.as_ref().unwrap().as_str();
                    match ele.key.as_str() {
                        "title" => book.set_title(v),
                        "author" => book.set_creator(v),
                        "isbn" => book.set_identifier(v),
                        "publisher" => book.set_publisher(v),
                        "date" => book.set_date(v),
                        "desc" => book.set_description(v),
                        "format" => book.set_format(v),
                        "subject" => book.set_subject(v),
                        "contributor" => book.set_contributor(v),
                        _ => {}
                    }
                }

                msg!("metadata update finished, writing file now");
                // 输出文件
                let file = global_opts
                    .iter()
                    .find(|f| f.key == "i")
                    .and_then(|f| f.value.as_ref())
                    .unwrap();
                match write_metadata(file, book) {
                    Ok(_) => {}
                    Err(e) => {
                        exec_err!("write file fail, because {:?}", e);
                    }
                };
            }
        }
    );
    create_command!(
        FormatConvert,
        "convert",
        {
            arg::CommandOptionDef {
                command: "convert".to_string(),
                support_args: 0,
                desc: "转换成mobi".to_string(),
                opts: vec![
                    OptionDef::create("f", "输出文件路径", OptionType::String, true),
                    OptionDef::create("n", "不添加标题，默认添加", OptionType::NoParamter, false),
                    OptionDef::create("i", "缩进字符数", OptionType::Number, false),
                    OptionDef::over(),
                ],
            }
        },
        fn exec(
            &self,
            book: &mut Book,
            global_opts: &[ArgOption],
            opts: &[ArgOption],
            _args: &[String],
        ) {
            let path = opts
                .iter()
                .find(|f| f.key == "f")
                .and_then(|f| f.value.clone())
                .unwrap();

            let append_title = !opts.iter().any(|f| f.key == "n");

            if let Book::EPUB(book) = book {
                let _ = epub_to_mobi(book)
                    .map(|mobi| {
                        (
                            mobi,
                            !std::path::Path::new(path.as_str()).exists()
                                || is_overiade(global_opts, opts)
                                || get_single_input("Override file？(y/n)")
                                    .unwrap()
                                    .to_lowercase()
                                    == "y",
                        )
                    })
                    .map(|(mobi, over)| {
                        if over {
                            msg!("writing file {}", path);
                            return MobiWriter::write_to_file_with_ident(
                                path.as_str(),
                                &mobi,
                                append_title,
                                opts.iter()
                                    .find(|f| f.key == "i")
                                    .and_then(|f| f.value.clone())
                                    .and_then(|f| f.parse::<usize>().ok())
                                    .unwrap_or(0),
                            );
                        }
                        Ok(())
                    })
                    .is_err_and(|e| {
                        exec_err!("err: {}", e);
                    });
            }
        }
    );
}

pub(crate) mod mobi {

    use iepub::prelude::{adapter::mobi_to_epub, EpubWriter, MobiNav};

    use crate::{
        cli::arg::{self, ArgOption, OptionDef, OptionType},
        exec_err, msg, Book, Command,
    };

    use super::{create_dir, get_single_input, is_overiade, write_file};

    create_command!(
        BookInfoGetter,
        "get-info",
        {
            arg::CommandOptionDef {
                command: "get-info".to_string(),
                support_args: 0,
                desc: "提取数据元数据".to_string(),
                opts: vec![
                    OptionDef::create("title", "标题", OptionType::NoParamter, false),
                    OptionDef::create("author", "作者", OptionType::NoParamter, false),
                    OptionDef::create("isbn", "isbn", OptionType::NoParamter, false),
                    OptionDef::create("publisher", "出版社", OptionType::NoParamter, false),
                    OptionDef::create("date", "出版日期", OptionType::NoParamter, false),
                    OptionDef::create("desc", "简介", OptionType::NoParamter, false),
                    OptionDef::create("format", "format", OptionType::NoParamter, false),
                    OptionDef::create("subject", "subject", OptionType::NoParamter, false),
                    OptionDef::create("contributor", "contributor", OptionType::NoParamter, false),
                    OptionDef::create("modify", "最后修改时间", OptionType::NoParamter, false),
                    OptionDef::create("generator", "电子书创建者", OptionType::NoParamter, false),
                    OptionDef::create("all", "所有元数据", OptionType::NoParamter, false),
                ],
            }
        },
        fn exec(
            &self,
            book: &mut Book,
            _global_opts: &[ArgOption],
            opts: &[ArgOption],
            _args: &[String],
        ) {
            if let Book::MOBI(book) = book {
                for ele in opts {
                    match ele.key.as_str() {
                        "title" => println!("{}", book.title()),
                        "author" => println!("{}", book.creator().unwrap_or("")),
                        "isbn" => println!("{}", book.identifier()),
                        "publisher" => println!("{}", book.publisher().unwrap_or("")),
                        "date" => println!("{}", book.date().unwrap_or("")),
                        "desc" => println!("{}", book.description().unwrap_or("")),
                        "format" => println!("{}", book.format().unwrap_or("")),
                        "subject" => println!("{}", book.subject().unwrap_or("")),
                        "contributor" => println!("{}", book.contributor().unwrap_or("")),
                        "modify" => println!("{}", book.last_modify().unwrap_or("")),
                        "generator" => println!("{}", book.generator().unwrap_or("")),
                        "all" => {
                            println!("title: {}", book.title());
                            println!("author: {}", book.creator().unwrap_or(""));
                            println!("isbn: {}", book.identifier());
                            println!("publisher: {}", book.publisher().unwrap_or(""));
                            println!("date: {}", book.date().unwrap_or(""));
                            println!("desc: {}", book.description().unwrap_or(""));
                            println!("format: {}", book.format().unwrap_or(""));
                            println!("subject: {}", book.subject().unwrap_or(""));
                            println!("contributor: {}", book.contributor().unwrap_or(""));
                            println!("modify: {}", book.last_modify().unwrap_or(""));
                            println!("generator: {}", book.generator().unwrap_or(""));
                        }
                        _ => {}
                    }
                }
            }
        }
    );

    create_command!(
        GetImage,
        "get-image",
        {
            arg::CommandOptionDef {
                command: "get-image".to_string(),
                desc: "提取图片".to_string(),
                support_args: 0,
                opts: vec![
                    OptionDef::create("d", "输出目录", OptionType::String,true),
                    OptionDef::over(),
                    OptionDef::create("p", "文件名前缀，例如-d out -p image,文件将会被写入到 out/image01.jpg，原有文件名将会被忽略", OptionType::String,false),
                ],
            }
        },
        fn exec(
            &self,
            book: &mut Book,
            global_opts: &[ArgOption],
            opts: &[ArgOption],
            _args: &[String],
        ) {
            if let Book::MOBI(book) = book {
                let dir_o = opts
                    .iter()
                    .find(|s| s.key == "d")
                    .and_then(|f| f.value.as_ref());
                let is_over = is_overiade(global_opts, opts);

                let prefix = opts
                    .iter()
                    .find(|s| s.key == "p")
                    .and_then(|f| f.value.as_ref());
                let mut file_size = 1;
                if let Some(dir) = dir_o {
                    for ele in book.assets_mut() {
                        let name = ele.file_name().to_lowercase();
                        if name.ends_with(".jpg")
                            || name.ends_with(".jpeg")
                            || name.ends_with(".gif")
                            || name.ends_with(".png")
                            || name.ends_with(".webp")
                            || name.ends_with(".svg")
                        {
                            let mut file = format!("{dir}/{}", ele.file_name());
                            if let Some(p) = prefix {
                                // 有前缀
                                file = format!(
                                    "{dir}/{p}{}{}",
                                    file_size,
                                    &name[name.rfind('.').unwrap_or(0)..]
                                );
                                file_size += 1;
                            }
                            let n_dir = &file[0..file.rfind('/').unwrap_or(0)];
                            if !std::path::Path::new(n_dir).exists() {
                                msg!("creating dir {}", n_dir);
                                // 创建目录
                                match std::fs::create_dir_all(n_dir) {
                                    Ok(_) => {}
                                    Err(e) => {
                                        eprintln!("create dir {} fail, because {}", n_dir, e);
                                        continue;
                                    }
                                };
                            }

                            // 判断文件是否存在

                            if std::path::Path::new(&file).exists()
                                && !is_over
                                && get_single_input("Override file？(y/n)")
                                    .unwrap()
                                    .to_lowercase()
                                    != "y"
                            {
                                continue;
                            }
                            msg!("writing file to {}", file);
                            // 写入文件
                            write_file(&file, ele.data().unwrap());
                        }
                    }
                }
            }
        }
    );

    create_command!(
        GetCover,
        "get-cover",
        {
            arg::CommandOptionDef {
                command: String::from("get-cover"),
                desc: "提取电子书封面, 例如get-cover 1.jpg，输出到1.jpg，不传将输出到默认文件名"
                    .to_string(),
                support_args: -1,
                opts: vec![OptionDef::create(
                    "y",
                    "是否覆盖输出文件",
                    OptionType::NoParamter,
                    false,
                )],
            }
        },
        fn exec(
            &self,
            book: &mut Book,
            global_opts: &[ArgOption],
            opts: &[ArgOption],
            args: &[String],
        ) {
            if let Book::MOBI(book) = book {
                let cover = book.cover_mut().unwrap_or_else(|| {
                    exec_err!("电子书没有封面");
                });
                let is_over = is_overiade(global_opts, opts);
                if args.is_empty() {
                    if std::path::Path::new(cover.file_name()).exists()
                        && !is_over
                        && get_single_input("Override file？(y/n)")
                            .unwrap()
                            .to_lowercase()
                            != "y"
                    {
                        return;
                    }
                    msg!("write cover to {}", cover.file_name());
                    write_file(cover.file_name(), cover.data().unwrap());
                }
                for path in args {
                    if std::path::Path::new(&path).exists()
                        && !is_over
                        && get_single_input("Override file？(y/n)")
                            .unwrap()
                            .to_lowercase()
                            != "y"
                    {
                        continue;
                    }
                    write_file(path, cover.data().unwrap());
                }
            }
        }
    );

    create_command!(
        Unpack,
        "unpack",
        {
            arg::CommandOptionDef {
                command: String::from("unpack"),
                desc: "解包mobi到指定文件夹".to_string(),
                support_args: -1,
                opts: vec![OptionDef::create("d", "输出目录", OptionType::String, true)],
            }
        },
        fn exec(
            &self,
            book: &mut Book,
            _global_opts: &[ArgOption],
            opts: &[ArgOption],
            _args: &[String],
        ) {
            if let Book::MOBI(book) = book {
                if let Some(path) = opts
                    .iter()
                    .find(|f| f.key == "d")
                    .and_then(|f| f.value.as_ref())
                {
                    // 创建目录
                    let img_dir = format!("{path}/images");
                    let html_dir = format!("{path}/html");
                    create_dir(img_dir.as_str());
                    create_dir(html_dir.as_str());
                    // 首先输出图片
                    for ele in book.assets_mut() {
                        write_file(
                            format!("{img_dir}/{}", ele.file_name()).as_str(),
                            ele.data().unwrap(),
                        );
                    }
                    let nav = book.nav();
                    if nav.len() > 0 {
                        // 然后输出html
                        for (index, chap) in book.chapters().enumerate() {
                            if let Some(p) = get_nav_value(nav.as_slice(), chap.nav_id()) {
                                // println!("title = {} path={:?}",chap.title(),p);
                                let dir = format!("{html_dir}/{}", p.join("/"));
                                create_dir(dir.as_str());
                                write_file(
                                    format!("{dir}/{:02}.{}.html", index, chap.title()).as_str(),
                                    self.format_html(chap.string_data().as_str(), chap.title())
                                        .as_bytes(),
                                );
                            }
                        }
                    }

                    // 最后输出元数据
                }
            }
        },
        fn format_html(&self, data: &str, title: &str) -> String {
            format!(
                r#"<html><head><title>{}</title></head><body>{}</body></html>"#,
                title, data
            )
        }
    );

    fn get_nav_value(nav: &[MobiNav], id: usize) -> Option<Vec<String>> {
        for (index, ele) in nav.iter().enumerate() {
            if ele.id() == id {
                return Some(Vec::new());
            }
            if let Some(mut v) = get_nav_value(ele.children().as_slice(), id) {
                v.insert(0, format!("{:02}.{}", index, ele.title()));
                return Some(v);
            }
        }

        None
    }

    create_command!(
        FormatConvert,
        "convert",
        {
            arg::CommandOptionDef {
                command: "convert".to_string(),
                support_args: 0,
                desc: "转换成epub".to_string(),
                opts: vec![
                    OptionDef::create("f", "输出文件路径", OptionType::String, true),
                    OptionDef::create("n", "不添加标题，默认添加", OptionType::NoParamter, false),
                    OptionDef::over(),
                ],
            }
        },
        fn exec(
            &self,
            book: &mut Book,
            global_opts: &[ArgOption],
            opts: &[ArgOption],
            _args: &[String],
        ) {
            let path = opts
                .iter()
                .find(|f| f.key == "f")
                .and_then(|f| f.value.clone())
                .unwrap();
            let append_title = !opts.iter().any(|f| f.key == "n");

            if let Book::MOBI(book) = book {
                let _ = mobi_to_epub(book)
                    .map(|f| {
                        (
                            f,
                            !std::path::Path::new(path.as_str()).exists()
                                || is_overiade(global_opts, opts)
                                || get_single_input("Override file？(y/n)")
                                    .unwrap()
                                    .to_lowercase()
                                    == "y",
                        )
                    })
                    .map(|(mut f, over)| {
                        if over {
                            msg!("writing file {}", path);
                            return EpubWriter::write_to_file(path.as_str(), &mut f, append_title);
                        }
                        Ok(())
                    })
                    .is_err_and(|e| {
                        exec_err!("err: {}", e);
                    });
            }
        }
    );
}
