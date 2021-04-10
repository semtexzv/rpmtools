#![feature(const_raw_ptr_deref)]

mod repolist;

use rpmsync::Syncer;
use rpmrepo::repomd::RepoMD;
use rpmrepo::primary::Package;
use rpmrepo::modules::Chunk;
use rpmrepo::updateinfo::{Update, Date};
use bindb::{Database, Table, FieldRef, Index, Indices, ROps, RwOps};

use anyhow::*;
use serde::{Serialize, Deserialize};
use itertools::Itertools;
use rayon::prelude::{ParallelBridge, ParallelIterator, IntoParallelRefIterator};
use rpmrepo::repomd::Type::Modules;
use uuid::Uuid;


pub struct Scanner {
    db: Database
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Repo {
    pub id: Uuid,
    pub url: String,
    pub basearch: Option<String>,
    pub releasever: Option<String>,
    pub revision: Option<i32>,
}

impl Table for Repo {
    const NAME: &'static str = "repo";
    const VERSION: u8 = 0;
    type Key = Uuid;

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
    pub id: Uuid,
    pub nevra: Nevra,
}

impl Table for Pkg {
    const NAME: &'static str = "package";
    const VERSION: u8 = 0;
    type Key = Uuid;


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
    pkg_id: Uuid,
    repo_id: Uuid,
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
    pub id: Uuid,
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
    type Key = Uuid;

    fn key() -> FieldRef<Self, Self::Key> {
        bindb::field_ref_of!(Advisory => id)
    }

    type Indices = (AdvisoryNameIdx, );
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
    pub pkg_id: Uuid,
    pub adv_id: Uuid,
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
    id: Uuid,
    name: String,
    repo_id: Uuid,
    arch: String,
}

impl Table for Module {
    const NAME: &'static str = "module";
    const VERSION: u8 = 0;
    type Key = Uuid;

    fn key() -> FieldRef<Self, Self::Key> {
        bindb::field_ref_of!(Module => id)
    }
}

#[derive(Debug, Clone, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct ModuleStream {
    id: Uuid,
    module_id: Uuid,
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
    type Key = Uuid;

    fn key() -> FieldRef<Self, Self::Key> {
        bindb::field_ref_of!(ModuleStream => id)
    }
}

#[derive(Debug, Clone, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct ModuleArtifact {
    pkg_id: Uuid,
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
        std::fs::create_dir_all("data").unwrap();
        Ok(Scanner {
            db: Database::open("data")
                .register::<Repo>()
                .register::<Pkg>()
                .register::<Advisory>()
                .register::<Module>()
                .register::<ModuleStream>()
                .register::<PkgAdvisory>()
                .register::<PkgRepo>()
            ,
        })
    }
    pub fn load_repolist(&mut self, rl: repolist::Repolist) -> Result<()> {
        println!("Pkg: {:?}", self.db.in_tx(|tx| tx.scan::<Pkg, _>(..).count()));

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
                        id: Uuid::new_v4(),
                    }
                }).collect::<Vec<_>>();

                println!("Repos: {:?}", repos);
                self.db.in_wtx(|tx| {
                    for mut r in repos {
                        if let Some(old) = tx.get_by::<Repo, RepoUrl>(&r.url) {
                            r.id = old.id;
                        }
                        tx.put(&r);
                    }
                });
            }
        }
        Ok(())
    }

    pub fn sync(&mut self) -> Result<()> {
        let repos = {
            self.db.in_tx(|tx| {
                println!("Package: {:?}", tx.scan::<Pkg, _>(..).count());
                println!("Advs: {:?}", tx.scan::<Advisory, _>(..).count());
                println!("Repos: {:?}", tx.scan::<Repo, _>(..).count());
                println!("PKG-Advs: {:?}", tx.scan::<PkgAdvisory, _>(..).count());
                println!("PKG-repos: {:?}", tx.scan::<PkgRepo, _>(..).count());
                println!("Modules: {:?}", tx.scan::<Module, _>(..).count());
                println!("Streams: {:?}", tx.scan::<ModuleStream, _>(..).count());
                tx.scan::<Repo, _>(..).collect::<Vec<_>>()
            })
        };

        repos.into_iter().par_bridge().for_each(|r| {
            match self.load_repo(&r) {
                Ok(o) => {}
                Err(e) => {
                    println!("Could not sync repo : {:?}", r.url);
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

impl rpmsync::MetadataTarget for RepoScanner {
    fn on_metadata(&mut self, syncer: &Syncer, md: RepoMD) {
        let old = self.db.in_tx(|tx| tx.get_by::<Repo, RepoUrl>(&self.repo.url));

        if old.as_ref().and_then(|r| r.revision) < Some(md.revision as _) {
            println!("{:?} is outdated, syncing", self.repo.url);

            if let Err(e) = syncer.sync_modules(&mut ModuleScanner { base: self, defaults: vec![] }, &md) {
                println!("Err :{:?}", e);
            }
            syncer.sync_packages_streaming(&mut PackageScanner { base: self, packages: vec![] }, &md);
            syncer.sync_updates_streaming(&mut UpdateScanner { base: self, advs: vec![] }, &md);

            self.repo.id = old.map(|v| v.id).unwrap_or_else(|| self.db.generate_id());
            self.repo.revision = Some(md.revision as _);

            let mut tx = self.db.wtx();
            tx.put(&self.repo);
            tx.commit();
        } else {
            println!("{:?} is up to date", self.repo.url);
        }
    }
}


pub struct PackageScanner<'a> {
    base: &'a mut RepoScanner,
    packages: Vec<Pkg>,
}

impl<'a> rpmsync::PackageTarget for PackageScanner<'a> {
    fn on_package(&mut self, p: Package) {
        let nevra = Nevra {
            name: p.name,
            epoch: p.version.epoch,
            ver: p.version.ver,
            rel: p.version.rel,
            arch: p.arch,
        };
        self.packages.push(Pkg {
            id: Uuid::new_v4(),
            nevra,
        });
    }

    fn done(&mut self) {
        let repo_id = self.base.repo.id;
        let pkgs = std::mem::replace(&mut self.packages, vec![]);
        self.base.db.in_wtx(|tx| {
            for mut pkg in pkgs {
                if let Some(old_pkg) = tx.get_by::<Pkg, PkgNevraIdx>(&pkg.nevra) {
                    pkg.id = old_pkg.id;
                }

                tx.put(&PkgRepo(PkgRepoId { pkg_id: pkg.id, repo_id }));
                tx.put(&pkg);
            }
        });
    }
}

pub struct UpdateScanner<'a> {
    base: &'a mut RepoScanner,
    advs: Vec<Advisory>,
}

impl<'a> rpmsync::UpdateTarget for UpdateScanner<'a> {
    fn on_update(&mut self, mut up: Update) {
        let mut adv = Advisory {
            id: Uuid::new_v4(),
            name: up.id.clone(),
            desc: up.description,
            summary: up.summary,
            r#type: up.typ,
            issued: up.issued.date,
            updated: up.updated.date,
        };
        self.advs.push(adv);

    }

    fn done(&mut self) {
        let repo_id = self.base.repo.id;
        let advisories = std::mem::replace(&mut self.advs, vec![]);
        self.base.db.in_wtx(|tx| {

            for mut adv in advisories {
                println!("adv: {:?}", adv);
                if let Some(old) = tx.get_by::<Advisory, AdvisoryNameIdx>(&adv.name) {
                    adv.id = old.id;
                }
                tx.put(&adv);
            }
        })
    }
}

pub struct ModuleScanner<'a> {
    base: &'a mut RepoScanner,
    defaults: Vec<String>,
}

impl rpmsync::ModuleTarget for ModuleScanner<'_> {
    fn on_module_chunk(&mut self, _chunk: Chunk) {
        match _chunk {
            Chunk::ModuleMd(md) => {
                let mut newmod = Module {
                    id: self.base.db.generate_id(),
                    repo_id: self.base.repo.id,
                    arch: md.arch,
                    name: md.name,
                };

                /*
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

                 */
            }
            Chunk::Defaults(def) => {}
        }
    }
}


fn main() -> Result<()> {
    env_logger::init();
    rayon::ThreadPoolBuilder::new().num_threads(4).build_global().unwrap();
    let mut scanner = Scanner::new()?;
    scanner.load_repolist(json::from_reader::<_, repolist::Repolist>(std::fs::File::open("./repolist.json")?)?)?;
    scanner.sync().unwrap();

    Ok(())
}
