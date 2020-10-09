pub(crate) use std::{
    sync::Arc,
    io::{Read, BufReader},
};
pub(crate) use serde::{Deserialize, de::DeserializeSeed};
use rpmrepo::repomd::Type;
use std::time::Duration;
use std::fmt::Debug;
use ureq::Response;
use retry::OperationResult;

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

pub fn backoff() -> impl Iterator<Item=Duration> {
    (4..=8).map(|t| {
        // 1600, 3200, 6400, 12800, 25600
        // Total is around 45secs, should be okay for disconnections and whatnot
        Duration::from_millis(u64::pow(2, t) * 100)
    })
}


fn unwrap_inner<E: Debug>(e: retry::Error<E>) -> E {
    match e {
        retry::Error::Operation { error, .. } => error,
        e => panic!("Invalid error {:?}", e)
    }
}

pub fn retry_call<F: FnMut() -> Response>(mut call: F) -> Result<Response, Box<ErrorImpl>> {
    retry::retry(backoff(), || {
        let resp = call();
        if !resp.ok() {
            let err = ErrorImpl::from_resp("", &resp);
            return if resp.server_error() {
                OperationResult::Err(err)
            } else {
                OperationResult::Retry(err)
            };
        }
        OperationResult::Ok(resp)
    }).map_err(unwrap_inner)
}