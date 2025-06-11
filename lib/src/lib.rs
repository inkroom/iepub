#[allow(unused)]
#[allow(dead_code)]
extern crate iepub_derive;
mod adapter;
mod common;
mod cover;
mod epub;
mod mobi;
pub mod path;

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
    pub use crate::epub::reader::read_from_file;
    pub use crate::epub::reader::read_from_vec;
    pub use crate::epub::writer::EpubWriter;

    pub mod appender {
        pub use crate::epub::appender::write_metadata;
    }

    pub use crate::mobi::builder::MobiBuilder;
    pub use crate::mobi::core::MobiBook;
    pub use crate::mobi::core::MobiHtml;
    pub use crate::mobi::core::MobiNav;
    pub use crate::mobi::reader::MobiReader;
    pub use crate::mobi::writer::MobiWriter;

    pub mod check {
        pub use crate::epub::reader::is_epub;
        pub use crate::mobi::reader::is_mobi;
    }

    pub mod adapter {
        pub use crate::adapter::core::concat::add_into_epub;
        pub use crate::adapter::core::epub_to_mobi;
        pub use crate::adapter::core::mobi_to_epub;
    }
}
#[cfg(test)]
mod tests {

    #[test]
    fn test_req() {
        use ureq::{Agent, Proxy};
        // Configure a SOCKS proxy.
        // let proxy = Proxy::new("socks5://user:password@cool.proxy:9090").;
        let agent: Agent = Agent::config_builder()
        .proxy(Some(Proxy::new("http://192.168.31.239:7890").unwrap()))
        
        .build().into();


        // This is proxied.
        let mut resp = agent.get("https://ifconfig.me/ip").call().unwrap();
        
        println!("{}", resp.body_mut().read_to_string().unwrap());



        let v =minreq::get("https://ifconfig.me/ip").send();
        println!("mint req={}",v.unwrap().as_str().unwrap());
let v =minreq::get("https://ifconfig.me/ip").with_proxy(minreq::Proxy::new("http://192.168.31.239:7890").unwrap()).send();

        println!("mint req={}",v.unwrap().as_str().unwrap());


    }
}
