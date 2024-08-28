#[allow(unused)]
#[allow(dead_code)]

extern crate iepub_derive;
mod common;
mod epub;
mod mobi;

pub mod prelude {
    pub use crate::common::IError;
    pub use crate::common::IResult;

    pub use crate::epub::builder::EpubBuilder;
    pub use crate::epub::common::LinkRel;
    pub use crate::epub::core::EpubAssets;
    pub use crate::epub::core::EpubBook;
    pub use crate::epub::core::EpubHtml;
    pub use crate::epub::core::EpubLink;
    pub use crate::epub::core::EpubMetaData;
    pub use crate::epub::core::EpubNav;
    pub use crate::epub::core::EpubWriter;
    pub use crate::epub::reader::read_from_file;
    pub use crate::epub::reader::read_from_vec;

    pub mod appender {
        pub use crate::epub::appender::write_metadata;
    }

    pub use crate::mobi::core::MobiBook;
    pub use crate::mobi::core::MobiNav;
    pub use crate::mobi::core::MobiHtml;
    pub use crate::mobi::reader::MobiReader;

    pub mod check {
        pub use crate::epub::reader::is_epub;
        pub use crate::mobi::reader::is_mobi;
    }
    

}
