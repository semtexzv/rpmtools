use crate::prelude::*;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoMD {
    pub revision: String,
    pub data: Vec<RepoMDItem>,
}

impl RepoMD {
    pub fn find_item(&self, typ: Type) -> Option<&RepoMDItem> {
        self.data.iter().find(|it| it.typ == typ)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoMDItem {
    #[serde(rename = "type")]
    pub typ: Type,
    pub checksum: Checksum,
    pub location: Location,

    #[serde(rename = "open-checksum")]
    pub open_checksum: Option<Checksum>,
    pub timestamp: Option<f32>,
    pub size: Option<usize>,
    #[serde(rename = "open-size")]
    pub open_size: Option<usize>,

}

#[derive(Debug, Clone, Serialize, Deserialize, PartialOrd, PartialEq, Ord, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Type {
    Primary,
    PrimaryDb,

    Other,
    OtherDb,

    Filelists,
    FilelistsDb,

    Group,
    GroupGz,

    Modules,
    ProductId,
    #[serde(rename = "updateinfo")]
    UpdateInfo,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checksum {
    #[serde(rename = "type")]
    pub typ: String,
    #[serde(rename = "$value")]
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub href: String
}