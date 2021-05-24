pub(crate) use std::{
    sync::Arc,
    io::{Read, BufReader},
};
pub(crate) use serde::{Deserialize, de::DeserializeSeed};
use rpmrepo::repomd::Type;
use std::time::Duration;
use std::fmt::Debug;
use ureq::{Response, ErrorKind};
use retry::OperationResult;
use retry::Error::Operation;
use std::error::Error;

#[derive(Debug, thiserror::Error)]
pub enum ErrorImpl {
    #[error("Requesting a resource")]
    // TODO: https://github.com/algesten/ureq/issues/126
    Req(String, String, u16),
    #[error("IO")]
    Io(#[from] std::io::Error),
    #[error("Parsing xml")]
    Xml(#[from] xml::de::DeError),
    #[error("Parsing yaml")]
    Yaml(#[from] syaml::Error),
    #[error("Stream compression")]
    Niffler(#[from] niffler::Error),
    #[error("HTTP: {0:?}")]
    Ureq(#[from] ureq::Error),
    #[error("{0:?} not found in repo metadata")]
    TypeNotFound(Type),
}

impl From<xml::de::DeError> for Box<ErrorImpl> {
    fn from(e: xml::de::DeError) -> Self {
        Box::new(ErrorImpl::Xml(e))
    }
}

impl From<niffler::Error> for Box<ErrorImpl> {
    fn from(e: niffler::Error) -> Self {
        Box::new(ErrorImpl::Niffler(e))
    }
}

impl From<std::io::Error> for Box<ErrorImpl> {
    fn from(e: std::io::Error) -> Self {
        Box::new(ErrorImpl::Io(e))
    }
}


impl From<syaml::Error> for Box<ErrorImpl> {
    fn from(e: syaml::Error) -> Self {
        Box::new(ErrorImpl::Yaml(e))
    }
}

impl ErrorImpl {
    /*
    pub fn from_resp(url: &str, resp: &ureq::Response) -> Box<Self> {
        let err = resp.synthetic_error().as_ref()
            .map(|e| e.to_string())
            .unwrap_or_else(|| resp.status_text().to_string());

        // TODO: https://github.com/algesten/ureq/issues/126
        return Box::new(ErrorImpl::Req(url.to_string(), err, resp.status()));
    }

     */
    pub fn boxed(self) -> Box<Self> {
        Box::new(self)
    }
}

pub type Result<T, E = Box<ErrorImpl>> = std::result::Result<T, E>;

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


pub fn retry_call<F: FnMut() -> Result<Response, ureq::Error>>(mut call: F) -> Result<Response, ErrorImpl> {
    retry::retry(backoff(), || {
        let resp = call();
        return match resp {
            Ok(resp) => {
                OperationResult::Ok(resp)
            }
            Err(err) if err.kind() == ErrorKind::Dns => {
                OperationResult::Retry(ErrorImpl::Ureq(err))
            }
            Err(ureq::Error::Transport(tp)) => {
                if let Some(src) = tp.source() {
                    if let Some(io) = src.downcast_ref::<std::io::Error>() {
                        if io.kind() == std::io::ErrorKind::TimedOut {
                            println!("Request timed out, retrying");
                            return OperationResult::Retry(ErrorImpl::Ureq(ureq::Error::Transport(tp)));
                        }
                    }
                }
                return OperationResult::Err(ErrorImpl::Ureq(ureq::Error::Transport(tp)));
            }
            Err(err) => {
                OperationResult::Err(ErrorImpl::Ureq(err))
            }
        };
    }).map_err(unwrap_inner)
}