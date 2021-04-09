use crate::prelude::*;


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "document", content = "data")]
pub enum Chunk {
    #[serde(rename = "modulemd")]
    ModuleMd(ModuleMDData),
    #[serde(rename = "modulemd-defaults")]
    Defaults(DefaultsData),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct License {
    #[serde(default)]
    pub module: Vec<String>,
    #[serde(default)]
    pub content: Vec<String>,
}

pub type Requires = HashMap<String, Vec<String>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Depencency {
    pub requires: Requires,
    #[serde(default)]
    pub buildrequires: Requires,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rpms<T> {
    pub rpms: T
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub description: Option<String>,
    pub rpms: Vec<String>,
}

pub type Profiles = HashMap<String, Profile>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    pub rationale: String,
    #[serde(rename = "ref")]
    pub reference: String,
    pub buildorder: Option<String>,
    #[serde(default)]
    pub arches: Vec<String>,
    #[serde(default)]
    pub multilib: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleMDData {
    pub name: String,
    pub stream: String,
    pub version: u64,

    pub context: String,
    pub arch: String,

    pub summary: Option<String>,
    pub description: Option<String>,
    pub license: License,

    #[serde(default)]
    pub dependencies: Vec<Depencency>,
    pub profiles: Option<Profiles>,
    pub api: Option<Rpms<Vec<String>>>,

    pub components: Option<Rpms<HashMap<String, Component>>>,

    pub artifacts: Option<Rpms<Vec<String>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultsData {
    pub module: String,
    pub stream: Option<String>,
    pub profiles: HashMap<String, Vec<String>>,
}