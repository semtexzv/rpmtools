use crate::prelude::*;

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateInfo {
    #[serde(default)]
    #[serde(rename = "update")]
    pub updates: Vec<Update>
}

#[derive(Debug, Clone, Deserialize)]
pub struct Update {
    #[serde(rename = "type")]
    pub typ: String,

    pub from: String,
    pub status: String,
    pub id: String,
    pub title: String,

    pub summary: Option<String>,
    pub rights: Option<String>,
    pub description: Option<String>,
    pub release: Option<String>,
    pub solution: Option<String>,

    #[serde(default)]
    pub severity: Option<String>,

    pub issued: Date,
    pub updated: Date,

    pub references: Vec<Reference>,
    pub pkglist: Vec<PkgList>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Date {
    pub date: String
}

#[derive(Debug, Clone, Deserialize)]
pub struct Reference {
    pub href: Option<String>,
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub typ: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PkgList {
    pub collection: Vec<Collection>
}

#[derive(Debug, Clone, Deserialize)]
pub struct Collection {
    pub name: String,
    pub module: Option<Module>,
    pub package: Vec<Package>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Module {
    pub name: String,
    pub stream: String,

    pub arch: String,
    pub version: String,
    pub context: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Package {
    pub name: String,
    pub epoch: String,
    pub version: String,
    pub release: String,
    pub arch: String,
}
