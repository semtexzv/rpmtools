#![allow(unused_import)]
mod prelude;
mod magic;

use crate::prelude::*;
use crate::magic::IterState;
use rpmrepo::{
    repomd::{RepoMD, Type},
    primary::{Primary, Package},
    updateinfo::Update,
    modules::Chunk
};

const PACKAGE_PATH: &[&str] = &["package"];
const UPDATE_PATH: &[&str] = &["update"];

pub struct Syncer {
    base: String,
    cert_config: Arc<rustls::ClientConfig>,
}

impl Syncer {
    pub fn new(cfg: rustls::ClientConfig, url: &str) -> Self {
        let mut base = url.to_string();
        if !base.ends_with('/') {
            base.push('/');
        }

        Self {
            base,
            cert_config: Arc::new(cfg),
        }
    }

    pub fn sync_md(&self, target: &mut dyn SyncTarget) {
        let resp = ureq::get(&format!("{}repodata/repomd.xml", &self.base))
            .set_tls_config(self.cert_config.clone())
            .call();

        if !resp.ok() {
            panic!("Error getting repo: {:?}", resp.synthetic_error())
        }
        let reader = resp.into_reader();
        let md: RepoMD = xml::de::from_reader(BufReader::new(reader)).unwrap();
        target.on_metadata(self, md);
    }

    pub fn sync_primary_streaming(&self, target: &mut dyn SyncTarget, md: &RepoMD) {
        println!("Downloading primary");
        let mut action = |p| {
            target.on_package(p);
            IterState::Continue
        };
        let action = crate::magic::ItemAction::<Package, ()>::new(&mut action);
        let seed = crate::magic::SeedField::new(PACKAGE_PATH, action);

        self.sync_xml_streaming(md, Type::Primary, seed).unwrap();
    }

    pub fn sync_updateinfo_streaming(&self, target: &mut dyn SyncTarget, md: &RepoMD) {
        println!("Downloading updateinfo");
        let mut action = |p| {
            target.on_update(p);
            IterState::Continue
        };
        let action = crate::magic::ItemAction::<Update, ()>::new(&mut action);
        let seed = crate::magic::SeedField::new(UPDATE_PATH, action);

        if let None = self.sync_xml_streaming(md, Type::UpdateInfo, seed) {
            eprintln!("Missing updateinfo")
        }
    }

    pub fn sync_modules(&self, target: &mut dyn SyncTarget, md: &RepoMD) {
        println!("Downloading modules");
        let data = if let Some(data) = md.find_item(Type::Modules) {
            data
        } else { return; };

        let resp = ureq::get(&format!("{}{}", &self.base, data.location.href))
            .set_tls_config(self.cert_config.clone())
            .call();

        if !resp.ok() {
            panic!("Error getting modules")
        }

        let mut data = String::new();
        resp.into_reader().read_to_string(&mut data).unwrap();

        let modules: Vec<Chunk> = syaml::from_str_multidoc(&data).unwrap();
        for m in modules {
            target.on_module_chunk(m);
        }
    }


    fn sync_xml_streaming<'a, T: DeserializeSeed<'a>>(&self, md: &RepoMD, typ: Type, seed: T) -> Option<T::Value> {
        use xml::de::Deserializer;

        let data = if let Some(data) = md.find_item(typ.clone()) {
            data
        } else { return None; };

        let url = format!("{}{}", &self.base, data.location.href);
        println!("URL: {:?}", url);
        let resp = ureq::get(&url)
            .set_tls_config(self.cert_config.clone())
            .call();

        if !resp.ok() {
            panic!("Error getting {:?}", typ)
        }

        let (decomp, format) = niffler::get_reader(Box::new(resp.into_reader())).unwrap();
        let mut de = Deserializer::from_reader(BufReader::new(decomp));

        Some(DeserializeSeed::deserialize(seed, &mut de).unwrap())
    }

}

pub trait SyncTarget {
    fn on_metadata(&mut self, syncer: &Syncer, md: RepoMD);

    fn on_package(&mut self, p: Package);
    fn on_update(&mut self, up: Update);
    fn on_module_chunk(&mut self, chunk: Chunk);
}

#[test]
fn test_sync() {
    struct DummyTarget {
        last_rev: usize
    }
    impl SyncTarget for DummyTarget {
        fn on_metadata(&mut self, syncer: &Syncer, md: RepoMD) {
            println!("{:?}", md);
            if self.last_rev < md.revision {
                syncer.sync_primary_streaming(self, &md);
                syncer.sync_updateinfo_streaming(self, &md);
                syncer.sync_modules(self, &md);
            }
        }

        fn on_package(&mut self, p: Package) {
            println!("Downloaded  package  {:?}", p);
        }

        fn on_update(&mut self, up: Update) {
            println!("Downloaded update {:?}", up);
        }

        fn on_module_chunk(&mut self, chunk: Chunk) {
            println!("Downloaded module chunk {:?}", chunk);
        }
    }

    let mut cert = rustls::ClientConfig::default();
    let cert_file = std::fs::File::open("/etc/ssl/certs/ca-bundle.crt").unwrap();
    cert.root_store.add_pem_file(&mut BufReader::new(cert_file)).unwrap();
    let syncer = Syncer::new(cert, "https://dl.yarnpkg.com/rpm/");
    syncer.sync_md(&mut DummyTarget {
        last_rev: 0
    })
}