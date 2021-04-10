#![allow(unused_imports)]

mod prelude;
mod magic;

use crate::prelude::*;
use crate::magic::IterState;
use rpmrepo::{
    repomd::{RepoMD, Type},
    primary::{Primary, Package},
    updateinfo::Update,
    modules::Chunk,
};
use retry::OperationResult;
use rustls::ClientConfig;
use std::time::Duration;

const PACKAGE_PATH: &[&str] = &["package"];
const UPDATE_PATH: &[&str] = &["update"];
const BUFFER_SIZE: usize = 1024 * 1024;

pub struct Syncer {
    base: String,
    agent: ureq::Agent,
}

impl Syncer {
    pub fn new(cfg: rustls::ClientConfig, url: &str) -> Self {
        let mut base = url.to_string();
        if !base.ends_with('/') {
            base.push('/');
        }

        let cert_config = Arc::new(cfg);

        let agent = ureq::AgentBuilder::new()
            .tls_config(cert_config)
            .max_idle_connections(8)
            .max_idle_connections_per_host(2).timeout_connect(Duration::from_secs(1));

        Self {
            base,
            agent: agent.build(),
        }
    }

    pub fn sync_md(&self, target: &mut dyn MetadataTarget) -> Result<()> {
        let url = format!("{}repodata/repomd.xml", &self.base);

        let resp = retry_call(|| {
            self.agent.get(&url)
                .call()
        })?;

        let (reader, _format) = niffler::get_reader(Box::new(resp.into_reader()))?;
        let md: RepoMD = xml::de::from_reader(BufReader::new(reader))?;
        target.on_metadata(self, md);

        Ok(())
    }

    pub fn sync_packages_streaming(&self, target: &mut dyn PackageTarget, md: &RepoMD) -> Result<()> {
        println!("Downloading primary");
        let mut action = |p| {
            target.on_package(p);
            IterState::Continue
        };
        let action = crate::magic::ItemAction::<Package, ()>::new(&mut action);
        let seed = crate::magic::SeedField::new(PACKAGE_PATH, action);

        if let None = self.sync_xml_streaming(md, Type::Primary, seed)? {
            eprintln!("Missing primary")
        }
        target.done();
        Ok(())
    }

    pub fn sync_updates_streaming(&self, target: &mut dyn UpdateTarget, md: &RepoMD) -> Result<()> {
        println!("Downloading updateinfo");
        let mut action = |p| {
            target.on_update(p);
            IterState::Continue
        };
        let action = crate::magic::ItemAction::<Update, ()>::new(&mut action);
        let seed = crate::magic::SeedField::new(UPDATE_PATH, action);

        if let None = self.sync_xml_streaming(md, Type::UpdateInfo, seed)? {
            eprintln!("Missing updateinfo")
        }

        Ok(())
    }

    pub fn sync_modules(&self, target: &mut dyn ModuleTarget, md: &RepoMD) -> Result<()> {
        println!("Downloading modules");
        let data = if let Some(data) = md.find_item(Type::Modules) {
            data
        } else { return Err(ErrorImpl::TypeNotFound(Type::Modules).boxed()); };

        let url = format!("{}{}", &self.base, data.location.href);

        let resp = retry_call(|| {
            self.agent.get(&url)
                .call()
        })?;

        let mut data = String::new();
        let (mut reader, _format) = niffler::get_reader(Box::new(resp.into_reader()))?;

        reader.read_to_string(&mut data)?;
        let modules: Vec<Chunk> = syaml::from_str_multidoc(&data)?;
        for m in modules {
            target.on_module_chunk(m);
        }

        Ok(())
    }


    fn sync_xml_streaming<'a, T: DeserializeSeed<'a>>(&self, md: &RepoMD, typ: Type, seed: T) -> Result<Option<T::Value>> {
        use xml::de::Deserializer;

        let data = if let Some(data) = md.find_item(typ.clone()) {
            data
        } else {
            return Err(ErrorImpl::TypeNotFound(typ).boxed());
        };

        let url = format!("{}{}", &self.base, data.location.href);
        println!("Call");
        let resp = retry_call(|| {
            self.agent.get(&url)
                .call()
        })?;
        println!("Got resp");

        let (decomp, _format) = niffler::get_reader(Box::new(resp.into_reader()))?;
        let reader = BufReader::with_capacity(BUFFER_SIZE, decomp);
        let mut de = Deserializer::from_reader(reader);

        Ok(Some(DeserializeSeed::deserialize(seed, &mut de).map_err(ErrorImpl::Xml)?))
    }
}

pub trait MetadataTarget {
    fn on_metadata(&mut self, syncer: &Syncer, md: RepoMD);
}

pub trait PackageTarget {
    fn on_package(&mut self, p: Package);
    fn done(&mut self);
}

pub trait UpdateTarget {
    fn on_update(&mut self, up: Update);
    fn done(&mut self);
}

pub trait ModuleTarget {
    fn on_module_chunk(&mut self, chunk: Chunk);
}

pub fn default_certs() -> ClientConfig {
    let mut cert = rustls::ClientConfig::default();
    for f in std::fs::read_dir("/etc/ssl/certs/").unwrap() {
        let f = f.unwrap();
        if f.path().extension().and_then(|s| s.to_str()) == Some("crt") {
            if let Ok(cert_file) = std::fs::File::open(f.path()) {
                cert.root_store.add_pem_file(&mut BufReader::new(cert_file)).unwrap();
            }
        }
    }
    return cert;
}

#[test]
fn test_sync() {
    struct DummyTarget {
        last_rev: usize
    }
    impl SyncTarget for DummyTarget {
        fn on_metadata(&mut self, syncer: &Syncer, md: RepoMD) {
            println!("Metadata {:?}", md);
            if self.last_rev < md.revision {
                syncer.sync_packages_streaming(self, &md).unwrap();
                if let Err(err) = syncer.sync_updates_streaming(self, &md) {
                    if let ErrorImpl::TypeNotFound(typ) = *err {
                        println!("Did not find : {:?}", typ);
                    }
                };

                if let Err(err) = syncer.sync_modules(self, &md) {
                    if let ErrorImpl::TypeNotFound(typ) = *err {
                        println!("Did not find : {:?}", typ);
                    }
                };
            }
        }

        fn on_package(&mut self, p: Package) {
            println!("Downloaded package  {:?}", p);
        }

        fn on_update(&mut self, up: Update) {
            println!("Downloaded update {:?}", up);
        }

        fn on_module_chunk(&mut self, chunk: Chunk) {
            println!("Downloaded module chunk {:?}", chunk);
        }
    }

    let mut cert = default_certs();

    let syncer = Syncer::new(cert, "https://dl.yarnpkg.com/rpm/");
    syncer.sync_md(&mut DummyTarget {
        last_rev: 0
    }).unwrap()
}