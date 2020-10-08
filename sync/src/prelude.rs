pub(crate) use std::{
    sync::Arc,
    io::{Read, BufReader},
};
pub(crate) use serde::{Deserialize, de::DeserializeSeed};
use rpmrepo::repomd::Type;

#[derive(Debug)]
pub enum ErrorImpl {
    // TODO: https://github.com/algesten/ureq/issues/126
    Req(String, String),
    ReqCode(String, String, u16),
    Xml(xml::de::DeError),
    Yaml(syaml::Error),
    TypeNotFound(Type),
}

impl ErrorImpl {
    pub fn from_resp(url: &str, resp: &ureq::Response) -> Box<Self> {
        if let Some(err) = resp.synthetic_error() {
            // TODO: https://github.com/algesten/ureq/issues/126
            return Box::new(ErrorImpl::Req(url.to_string(), err.to_string()));
        } else {
            return Box::new(ErrorImpl::ReqCode(url.to_string(), resp.status_text().to_string(), resp.status()));
        }
    }
    pub fn boxed(self) -> Box<Self> {
        Box::new(self)
    }
}

pub type Error = Box<ErrorImpl>;
pub type Result<T, E = Error> = std::result::Result<T, E>;