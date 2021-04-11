use bindb::{index, table, Table, FieldRef, Index};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Repo {
    pub id: Uuid,
    pub url: String,
    pub basearch: Option<String>,
    pub releasever: Option<String>,
    pub revision: Option<i32>,
}

table!(Repo => id(Uuid), RepoUrl);
index!(RepoUrl, Repo => url(String));

#[derive(Debug, Clone, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct Nevra {
    pub name: String,
    pub epoch: u32,
    pub ver: String,
    pub rel: String,
    pub arch: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Pkg {
    pub id: Uuid,
    pub nevra: Nevra,
}

table!(Pkg => nevra(Nevra), PkgNevraIdx);
index!(PkgNevraIdx, Pkg => nevra(Nevra));

#[derive(Debug, Clone, Copy, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct PkgRepoId {
    pub pkg_id: Uuid,
    pub repo_id: Uuid,
}

#[derive(Debug, Clone, Copy, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct PkgRepo(pub PkgRepoId);
table!(PkgRepo => 0(PkgRepoId));

#[derive(Debug, Clone, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct Advisory {
    pub id: Uuid,
    pub r#type: String,
    pub name: String,
    pub summary: Option<String>,
    pub desc: Option<String>,
    pub issued: String,
    pub updated: String,
}

table!(Advisory => id(Uuid), AdvisoryNameIdx);
index!(AdvisoryNameIdx, Advisory => name(String));

#[derive(Debug, Clone, Copy, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct AdvisoryRepoId {
    pub adv_id: Uuid,
    pub repo_id: Uuid,
}

#[derive(Debug, Clone, Copy, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct AdvisoryRepo(pub AdvisoryRepoId);
table!(AdvisoryRepo => 0(AdvisoryRepoId));

#[derive(Debug, Clone, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct PkgAdvisoryId {
    pub pkg_id: Uuid,
    pub adv_id: Uuid,
    pub stream_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct PkgAdvisory(pub PkgAdvisoryId);
table!(PkgAdvisory => 0(PkgAdvisoryId));

#[derive(Debug, Clone, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct ModuleAttrs {
    pub repo_id: Uuid,
    pub name: String,
    pub arch: String,
}

#[derive(Debug, Clone, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct Module {
    pub id: Uuid,
    pub attrs: ModuleAttrs,
}

table!(Module => id(Uuid), ModuleAttrsIdx);
index!(ModuleAttrsIdx, Module => attrs(ModuleAttrs));

#[derive(Debug, Clone, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct ModuleStream {
    pub id: Uuid,
    pub attrs: StreamAttrs,
    pub default: bool,
}

#[derive(Debug, Clone, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct StreamAttrs {
    pub module_id: Uuid,
    pub name: String,
    pub version: u64,
    pub context: String,
}

table!(ModuleStream => id(Uuid), StreamAttrsIdx);
index!(StreamAttrsIdx, ModuleStream => attrs(StreamAttrs));
