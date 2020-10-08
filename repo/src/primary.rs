use crate::prelude::*;
use crate::repomd::{Checksum, Location};

#[derive(Debug, Serialize, Deserialize)]
pub struct Primary {

    #[serde(rename = "packages")]
    pub package_count: usize,
    #[serde(rename = "package")]
    pub packages: Vec<Package>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Package {
    #[serde(rename = "type")]
    pub typ: String,

    pub name: String,
    pub arch: String,
    pub version: PackageVersion,
    pub checksum: Option<Checksum>,
    pub summary: String,
    pub description: String,
    // packager
    pub packager: Option<String>,
    pub url: Option<String>,
    pub time: PackageTime,
    pub size: PackageSize,
    pub location: Location,
    // TODO: Extensible
    pub format: Option<Format>,


}

#[derive(Debug, Serialize, Deserialize, PartialOrd, Ord, Eq, PartialEq)]
pub struct PackageVersion {
    // TODO: should be usize ?
    pub epoch: String,
    pub ver: String,
    pub rel: String,

}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageTime {
    pub file: usize,
    pub build: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageSize {
    pub package: usize,
    pub archive: usize,
    pub installed: usize,
}


#[derive(Debug, Serialize, Deserialize)]
pub struct Format {
    #[serde(rename = "sourcerpm")]
    pub source: String
}


#[test]
fn test_parse_primary() {
    let data = include_str!("../../testdata/yarm-primary.xml");
    let primary = xml::de::from_str::<Primary>(data).unwrap();
    assert_eq!(primary.packages.len(), 51);

}