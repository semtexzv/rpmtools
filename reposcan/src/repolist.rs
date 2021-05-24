use std::collections::BTreeMap;
use serde_with::{serde_as,  OneOrMany};
use serde::{Deserialize,};

pub type Repolist = Vec<RepoEntry>;

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct RepoEntry {
    pub products: BTreeMap<String, Product>
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct Product {
    pub content_sets: BTreeMap<String, ContentSet>
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct ContentSet {
    pub name: String,
    #[serde_as(as = "OneOrMany<_>")]
    pub baseurl: Vec<String>,
    #[serde_as(as = "OneOrMany<_>")]
    pub basearch: Vec<String>,
    #[serde_as(as = "OneOrMany<_>")]
    pub releasever: Vec<String>,
}