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
    ReqCode(String, usize),
    Xml(xml::de::DeError),
    Yaml(syaml::Error),
    TypeNotFound(Type),
}

impl ErrorImpl {
    pub fn boxed(self) -> Box<Self> {
        Box::new(self)
    }
}

pub type Error = Box<ErrorImpl>;
pub type Result<T, E = Error> = std::result::Result<T, E>;