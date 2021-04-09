#![feature(const_raw_ptr_deref)]

mod repolist;

use rpmsync::Syncer;
use rpmrepo::repomd::RepoMD;
use rpmrepo::primary::Package;
use rpmrepo::modules::Chunk;
use rpmrepo::updateinfo::{Update, Date};
use bindb::{Database, Table, FieldRef, Index, Indices};

use anyhow::*;
use serde::{Serialize, Deserialize};
use itertools::Itertools;
use rayon::prelude::{ParallelBridge, ParallelIterator};
use rpmrepo::repomd::Type::Modules;


pub struct Scanner {
    db: Database
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Repo {
    pub id: u64,
    pub url: String,
    pub basearch: Option<String>,
    pub releasever: Option<String>,
    pub revision: Option<i32>,
}

impl Table for Repo {
    const NAME: &'static str = "repo";
    const VERSION: u8 = 0;
    type Key = u64;

    fn key() -> FieldRef<Self, Self::Key> {
        bindb::field_ref_of!(Repo => id)
    }

    type Indices = (RepoUrl, );
}

pub struct RepoUrl {}

impl Index<Repo> for RepoUrl {
    const NAME: &'static str = "repo_url";
    type Key = String;

    fn key() -> FieldRef<Repo, Self::Key> {
        bindb::field_ref_of!(Repo => url)
    }
}

#[derive(Debug, Clone, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct Nevra {
    pub name: String,
    pub epoch: String,
    pub ver: String,
    pub rel: String,
    pub arch: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Pkg {
    pub id: u64,
    pub nevra: Nevra,
}

impl Table for Pkg {
    const NAME: &'static str = "package";
    const VERSION: u8 = 0;
    type Key = u64;


    fn key() -> FieldRef<Self, Self::Key> {
        bindb::field_ref_of!(Pkg => id)
    }

    type Indices = (PkgNevraIdx, );
}

pub struct PkgNevraIdx {}

impl Index<Pkg> for PkgNevraIdx {
    const NAME: &'static str = "package_nevra";
    type Key = Nevra;

    fn key() -> FieldRef<Pkg, Self::Key> {
        bindb::field_ref_of!(Pkg => nevra)
    }
}

#[derive(Debug, Clone, Copy, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct PkgRepoId {
    pkg_id: u64,
    repo_id: u64,
}

#[derive(Debug, Clone, Copy, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct PkgRepo(PkgRepoId);

impl Table for PkgRepo {
    const NAME: &'static str = "pkg_repo";
    const VERSION: u8 = 0;
    type Key = PkgRepoId;

    fn key() -> FieldRef<Self, Self::Key> {
        bindb::field_ref_of!(PkgRepo => 0)
    }
}

#[derive(Debug, Clone, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct Advisory {
    pub id: u64,
    pub r#type: String,
    pub name: String,
    pub summary: Option<String>,
    pub desc: Option<String>,
    pub issued: String,
    pub updated: String,
}

impl Table for Advisory {
    const NAME: &'static str = "advisory";
    const VERSION: u8 = 0;
    type Key = u64;

    fn key() -> FieldRef<Self, Self::Key> {
        bindb::field_ref_of!(Advisory => id)
    }
}


pub struct AdvisoryNameIdx {}

impl Index<Advisory> for AdvisoryNameIdx {
    const NAME: &'static str = "advisory_name";
    type Key = String;

    fn key() -> FieldRef<Advisory, Self::Key> {
        bindb::field_ref_of!(Advisory => name)
    }
}

#[derive(Debug, Clone, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct PkgAdvisoryId {
    pub pkg_id: u64,
    pub adv_id: u64,
}

#[derive(Debug, Clone, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct PkgAdvisory(PkgAdvisoryId);

impl Table for PkgAdvisory {
    const NAME: &'static str = "package_advisory";
    const VERSION: u8 = 0;
    type Key = PkgAdvisoryId;

    fn key() -> FieldRef<Self, Self::Key> {
        bindb::field_ref_of!(PkgAdvisory => 0)
    }
}


#[derive(Debug, Clone, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct Module {
    id: u64,
    name: String,
    repo_id: u64,
    arch: String,
}

impl Table for Module {
    const NAME: &'static str = "module";
    const VERSION: u8 = 0;
    type Key = u64;

    fn key() -> FieldRef<Self, Self::Key> {
        bindb::field_ref_of!(Module => id)
    }
}

#[derive(Debug, Clone, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct ModuleStream {
    id: u64,
    module_id: u64,
    name: String,
    version: u64,
    context: String,
    default: bool,
    artifacts: Vec<ModuleArtifact>,
    profiles: Vec<ModuleProfile>,
}

impl Table for ModuleStream {
    const NAME: &'static str = "module_stream";
    const VERSION: u8 = 0;
    type Key = u64;

    fn key() -> FieldRef<Self, Self::Key> {
        bindb::field_ref_of!(ModuleStream => id)
    }
}
#[derive(Debug, Clone, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct ModuleArtifact {
    pkg_id: u64,
}

#[derive(Debug, Clone, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct ModuleProfile {
    name: String,
    default: bool,
    artifacts: Vec<ModuleProfileArtifact>,
}

#[derive(Debug, Clone, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct ModuleProfileArtifact {
    name: String,
}

impl Scanner {
    pub fn new() -> Result<Self> {
        Ok(Scanner {
            db: Database::open("data"),
        })
    }
    pub fn load_repolist(&mut self, rl: repolist::Repolist) -> Result<()> {
        println!("Pkg: {:?}", self.db.scan::<Pkg>().count());

        for (_p, prod) in rl.iter().flat_map(|p| &p.products) {
            for (label, cs) in &prod.content_sets {
                let urls = cs.baseurl.iter().cloned();
                let urls = urls.cartesian_product(cs.basearch.iter().map(Some).chain(None));
                let urls = urls.cartesian_product(cs.releasever.iter().map(Some).chain(None));
                let repos = urls.map(|((mut url, arch), rv)| {
                    if let Some(arch) = arch {
                        url = url.replace("$basearch", arch);
                    }
                    if let Some(rv) = rv {
                        url = url.replace("$releasever", rv);
                    }
                    Repo {
                        url,
                        basearch: arch.map(ToString::to_string),
                        releasever: rv.map(ToString::to_string),
                        revision: None,
                        id: 0,
                    }
                }).collect::<Vec<_>>();

                for mut r in repos {
                    if self.db.find_by::<Repo, RepoUrl>(&r.url).is_none() {
                        r.id = self.db.generate_id();
                        self.db.put(&r);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn sync(&self) -> Result<()> {
        println!("Package: {:?}", self.db.scan::<Pkg>().count());
        println!("Advs: {:?}", self.db.scan::<Advisory>().count());
        println!("Repos: {:?}", self.db.scan::<Repo>().count());
        println!("PKG-Advs: {:?}", self.db.scan::<PkgAdvisory>().count());
        println!("PKG-repos: {:?}", self.db.scan::<PkgRepo>().count());
        println!("Modules: {:?}", self.db.scan::<Module>().count());
        println!("Streams: {:?}", self.db.scan::<ModuleStream>().count());


        self.db.scan::<Repo>().par_bridge().for_each(|r| {
            match self.load_repo(&r) {
                Ok(o) => {}
                Err(e) => {
                    println!("Could not sync repo : {:?}", r.url);
                    self.db.delete_by::<Repo>(&r.id);
                }
            }
        });
        Ok(())
    }

    pub fn load_repo(&self, repo: &Repo) -> Result<()> {
        let syncer = rpmsync::Syncer::new(rpmsync::default_certs(), &format!("{}/", repo.url));
        let mut scanner = RepoScanner {
            repo: repo.clone(),
            db: self.db.clone(),
        };
        Ok(syncer.sync_md(&mut scanner)?)
    }
}

pub struct RepoScanner {
    repo: Repo,
    db: Database,
}

impl RepoScanner {}

impl rpmsync::MetadataTarget for RepoScanner {
    fn on_metadata(&mut self, syncer: &Syncer, md: RepoMD) {
        let old = self.db
            .find_by::<Repo, RepoUrl>(&self.repo.url);

        if old.as_ref().and_then(|r| r.revision) < Some(md.revision as _) {
            println!("{:?} is outdated, syncing", self.repo.url);

            if let Err(e) = syncer.sync_modules(&mut ModuleSyncer { base: self, defaults: vec![] }, &md) {
                println!("Err :{:?}", e);
            }
            syncer.sync_primary_streaming(self, &md);
            syncer.sync_updateinfo_streaming(self, &md);

            self.repo.id = old.map(|v| v.id).unwrap_or_else(|| self.db.generate_id());
            self.repo.revision = Some(md.revision as _);

            self.db.put(&self.repo);
        } else {
            println!("{:?} is up to date", self.repo.url);
        }
    }
}

impl rpmsync::PackageTarget for RepoScanner {
    fn on_package(&mut self, p: Package) {
        let nevra = Nevra {
            name: p.name,
            epoch: p.version.epoch,
            ver: p.version.ver,
            rel: p.version.rel,
            arch: p.arch,
        };
        let pkg = self.db.find_by::<Pkg, PkgNevraIdx>(&nevra);
        let pkg = if let Some(pkg) = pkg {
            pkg
        } else {
            Pkg {
                id: self.db.generate_id(),
                nevra,
            }
        };
        let repo_id = self.repo.id;
        self.db.put(&PkgRepo(PkgRepoId { pkg_id: pkg.id, repo_id }));
        self.db.put(&pkg);
    }
}

impl rpmsync::UpdateTarget for RepoScanner {
    fn on_update(&mut self, mut up: Update) {
        let mut adv = Advisory {
            id: self.db.generate_id(),
            name: up.title.clone(),
            desc: up.description,
            summary: up.summary,
            r#type: up.typ,
            issued: up.issued.date,
            updated: up.updated.date,
        };

        if let Some(old) = self.db.find_by::<Advisory, AdvisoryNameIdx>(&up.id) {
            adv.id = old.id;
        }

        self.db.put(&adv);

        for mut pkg in up.pkglist.drain(..) {
            for mut pkg in pkg.collection.drain(..) {
                for pkg in pkg.package.drain(..) {
                    let nevra = Nevra {
                        name: pkg.name,
                        epoch: pkg.epoch,
                        ver: pkg.version,
                        rel: pkg.release,
                        arch: pkg.arch,
                    };

                    let pkg = self.db.find_by::<Pkg, PkgNevraIdx>(&nevra)
                        .unwrap_or_else(|| {
                            Pkg {
                                id: self.db.generate_id(),
                                nevra,
                            }
                        });

                    self.db.put(&pkg);
                    self.db.put(&PkgRepo(PkgRepoId { pkg_id: pkg.id, repo_id: self.repo.id }));
                    self.db.put(&PkgAdvisory(PkgAdvisoryId { pkg_id: pkg.id, adv_id: adv.id }));
                }
            }
        }
    }
}

pub struct ModuleSyncer<'a> {
    base: &'a mut RepoScanner,
    defaults: Vec<String>,
}

impl rpmsync::ModuleTarget for ModuleSyncer<'_> {
    fn on_module_chunk(&mut self, _chunk: Chunk) {
        println!("Module: {:?}", _chunk);
        match _chunk {
            Chunk::ModuleMd(md) => {
                let mut newmod = Module {
                    id: self.base.db.generate_id(),
                    repo_id: self.base.repo.id,
                    arch: md.arch,
                    name: md.name,
                };

                if let Some(id) = self.base.db
                    .query::<Module, _>(|m| {
                        m.repo_id == newmod.repo_id && m.arch == newmod.arch && m.name == newmod.name
                    })
                    .map(|m| m.id).next() {
                    newmod.id = id
                };

                let stream = ModuleStream {
                    id: self.base.db.generate_id(),
                    module_id: newmod.id,
                    name: md.stream,
                    version: md.version,
                    context: md.context,
                    default: false,
                    artifacts: vec![],
                    profiles: vec![],
                };
                self.base.db.put(&newmod);
                self.base.db.put(&stream);
            }
            Chunk::Defaults(def) => {}
        }
    }
}


fn main() -> Result<()> {
    env_logger::init();
    rayon::ThreadPoolBuilder::new().num_threads(16).build_global().unwrap();
    let mut scanner = Scanner::new()?;
    scanner.load_repolist(json::from_reader::<_, repolist::Repolist>(std::fs::File::open("./repolist.json")?)?)?;
    scanner.sync().unwrap();

    Ok(())
}
