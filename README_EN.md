# iepub

[EPUB](https://www.w3.org/TR/2023/REC-epub-33-20230525/)、[MOBI](https://wiki.mobileread.com/wiki/MOBI)reading and writing library,，

![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/inkroom/iepub/release.yml?label=CI)
[![Crates.io version](https://img.shields.io/crates/v/iepub.svg)](https://crates.io/crates/iepub)

## EPUB

Supports reading and generating EPUB e-books from files and memory

### Generation

- Can manually generate epub using the `EpubBook` struct
- (Recommended) Use `EpubBuilder` for quick generation

```rust
use iepub::prelude::EpubHtml;
use iepub::prelude::EpubBuilder;
use iepub::prelude::Direction;

EpubBuilder::default()
    .with_title("book name")
    .with_creator("author")
    .with_date("2024-03-14")
    .with_description("book description")
    .with_identifier("isbn")
    .with_publisher("publisher")
    .with_direction(Direction::RTL)
    .add_chapter(
        EpubHtml::default()
            .with_file_name("0.xml")
            .with_title("first titile")
            .with_data("<p>content</p>".to_string().as_bytes().to_vec()),
    )
    .add_chapter(
        EpubHtml::default()
            .with_file_name("1.xml")
            .with_title("second")
            .with_direction(crate::prelude::Direction::LTR)
            .with_data("<p>cccc</p>".to_string().as_bytes().to_vec()),
    )
    .add_assets("1.css", "p{color:red}".to_string().as_bytes().to_vec())
    .metadata("s", "d")
    .metadata("h", "m")
    .file("target/build.epub")
    .unwrap();

```

### 读取

```rust
use iepub::prelude::{reader::read_from_vec, reader::read_from_file, EpubHtml};
let mut data = Vec::new();// binary data of epub

let mut book = read_from_vec(data);
// Read from file
let mut bbook = read_from_file("absolute path to epub format file");

```

### Notes

- `iepub` uses `EpubHtml` to store chapter content, but EpubHtml#data will only actually store the content within the html>body nodes, other elements such as style sheets will be stored in other attributes
- Different readers have different compatibility with filenames, it is recommended to use the `.xhtml` suffix for files, for example `EpubHtml::default().with_file_name("1.xhtml")`


#### Custom Navigation

-  call `custome_nav(true)`, then call `add_nav()` to add navigation

#### Auto-generated Cover

Automatically generates a cover image with black background and white text displaying the book title

Need to enable feature `cover`, then call `auto_gen_cover(true)`, and also call `with_font(font)` to set the font file location.


## mobi

### Reading

```rust
use iepub::prelude::*;

let path = std::env::current_dir().unwrap().join("1.mobi");
let mut mobi = MobiReader::new(std::fs::File::open(path.to_str().unwrap()).unwrap()).unwrap();
let book = mobi.load().unwrap();
```

### Writing

```rust
let v = MobiBuilder::default()
            .with_title("书名")
            .with_creator("作者")
            .with_date("2024-03-14")
            .with_description("一本好书")
            .with_identifier("isbn")
            .with_publisher("行星出版社")
            .append_title(true)
            .custome_nav(true)
            .add_chapter(MobiHtml::new(1).with_title("标题").with_data("<p>锻炼</p>".as_bytes().to_vec()))
            // .file("builder.mobi")
            .mem()
            .unwrap();
```

#### Custom Navigation

- If you need custom navigation, call `custome_nav(true)`, then call `add_nav()` to add navigation
- To associate nav navigation and chapters, call `MobiNav#set_chap_id()` to indicate the pointing chapter; if it's like a table of contents at the beginning, point to the closest chapter

#### Images

- In mobi format, images do not have file paths. If you need to add images, first use the src attribute of the img tag in the chapter, give any filename as long as it's not duplicated, then when adding images also point to the same filename, and finally when writing it will add the images
- Additionally, the cover needs to be set by calling cover()


#### Titles

By default, a ‌title xml‌ segment will be added before the chapter's html fragment. If the chapter content already has a readable title, set `append_title(false)`

#### Auto-generated Cover

Automatically generates a cover image with black background and white text displaying the book title

Need to enable feature `cover`, then call `auto_gen_cover(true)`, and also call `with_font(font)` to set the font file location.

## Conversion

### mobi -> epub

```rust
use iepub::prelude::*;
let mut book = std::fs::File::open(std::path::PathBuf::from("example.mobi"))
            .map_err(|e| IError::Io(e))
            .and_then(|f| MobiReader::new(f))
            .and_then(|mut f| f.load())
            .unwrap();

let mut epub = mobi_to_epub(&mut book).unwrap();
epub.write("convert.epub").unwrap();
```

### epub -> mobi

```rust
use iepub::prelude::*;
let mut epub = EpubBook::default();

let mobi = epub_to_mobi(&mut epub).unwrap();
let mut v = std::io::Cursor::new(Vec::new());
MobiWriter::new(&mut v)
    .unwrap()
    .with_append_title(false)
    .write(&mobi)
    .unwrap();
```

## Command Line Tool

The [lib/src/cli](https://github.com/inkroom/iepub/releases) directory contains a command line tool that supports both mobi and epub formats, but different formats support different commands

Currently supported:
- Get metadata such as title, author
- Modify metadata
- Extract cover
- Extract all images
- Extract text from a chapter
- Get navigation
- Format conversion
- Ebook merging
- Text replacement
- Ebook slimming

Use `-h` to get usage instructions

Can be installed using `cargo install iepub`

## Cache

Enabling the **‌cache‌** feature can cache to files, suitable for crawler scenarios with retries
