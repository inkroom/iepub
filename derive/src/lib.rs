extern crate proc_macro;

use proc_macro::TokenStream;

///
///
/// 对于Option<String>类型的结构体成员，生成相关方法，支持多级嵌套
///
///
/// ```compile_fail
/// use iepub_derive::option_string_method;
/// // 访问成员 self.info.k
/// option_string_method!("", info,k);
/// // 访问成员 self.k
/// option_string_method!("", k);
/// ```
///
#[proc_macro]
pub fn option_string_method(input: TokenStream) -> TokenStream {
    let s = input.to_string();
    let mut v: Vec<&str> = s.split(',').collect();
    let doc = v.remove(0);
    let m = r#"
    #[doc = {doc}]
    pub fn set_{method}<T:  AsRef<str>>(&mut self, {method}: T) {
        if let Some( c) = &mut self.{prefix}{method} {
            c.clear();
            c.push_str({method}.as_ref());
        } else {
            self.{prefix}{method} = Some(String::from({method}.as_ref()));
        }
    }
    #[doc = {doc}]
    pub fn with_{method}<T:  AsRef<str>>(mut self, {method}: T) ->Self {
        self.set_{method}({method}.as_ref());
        self
    }
    #[doc = {doc}]
    pub fn {method}(&self) -> Option<&str> {
        self.{prefix}{method}.as_ref().map(|x|x.as_str())
    }"#
    .replace("{doc}", doc);
    if v.len() == 2 {
        let r = m
            .replace("{prefix}", format!("{}.", v[0].trim()).as_str().trim())
            .replace("{method}", v[1].trim());
        return r.parse().unwrap();
    } else if v.len() == 1 {
        return m
            .replace("{prefix}", "")
            .replace("{method}", v[0].trim())
            .parse()
            .unwrap();
    }
    "".parse().unwrap()
}
