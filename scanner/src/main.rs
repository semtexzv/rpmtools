#![feature(const_raw_ptr_deref)]

mod repolist;
mod data;

use rpmsync::Syncer;
use rpmrepo::repomd::RepoMD;
use rpmrepo::primary::Package;
use rpmrepo::modules::Chunk;
use rpmrepo::updateinfo::{Update};
use anyhow::*;
use itertools::Itertools;
use rayon::prelude::{ParallelBridge, ParallelIterator};
use uuid::Uuid;
use std::collections::HashMap;
use data::*;
use bindb::{Database, RwOps, ROps};


pub struct Scanner {
    db: Database
}


impl Scanner {
    pub fn new() -> Result<Self> {
        Ok(Scanner {
            db: Database::open("data.mdbx")
                .register::<Repo>()
                .register::<Pkg>()
                .register::<Advisory>()
                .register::<Module>()
                .register::<ModuleStream>()
                .register::<PkgAdvisory>()
                .register::<PkgRepo>()
                .register::<AdvisoryRepo>()
            ,
        })
    }
    pub fn load_repolist(&mut self, rl: repolist::Repolist) -> Result<()> {
        for (_p, prod) in rl.iter().flat_map(|p| &p.products) {
            for (_label, cs) in &prod.content_sets {
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

                self.db.in_wtx(|tx| {
                    for mut r in repos {
                        tx.put_by::<RepoUrl>(&mut r);
                    }
                });
            }
        }
        Ok(())
    }

    pub fn sync(&mut self) -> Result<()> {
        let repos = {
            self.db.in_tx(|tx| {
                println!("Package: {:?}", tx.scan::<Pkg>().count());
                println!("Advs: {:?}", tx.scan::<Advisory>().count());
                println!("Repos: {:?}", tx.scan::<Repo>().count());
                println!("PKG-Advs: {:?}", tx.scan::<PkgAdvisory>().count());
                println!("PKG-repos: {:?}", tx.scan::<PkgRepo>().count());
                println!("adv-repos: {:?}", tx.scan::<AdvisoryRepo>().count());
                println!("Modules: {:?}", tx.scan::<Module>().count());
                println!("Streams: {:?}", tx.scan::<ModuleStream>().count());
                for m in tx.scan::<ModuleStream>() {
                    println!("streams: {:?}", m);
                }
                tx.scan::<Repo>().collect::<Vec<_>>()
            })
        };

        repos.into_iter().par_bridge().for_each(|r| {
            let mut db = self.db.clone();
            match self.load_repo(&r) {
                Ok(_) => {}
                Err(e) => {
                    println!("Could not sync repo : {:?} : {}", r.url, e);
                    db.in_wtx(|w| w.delete::<Repo>(&r.id));
                }
            }
        });

        Ok(())
    }

    pub fn load_repo(&self, repo: &Repo) -> Result<()> {
        let syncer = rpmsync::Syncer::new(rpmsync::default_certs(), 32, &format!("{}/", repo.url));
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
        let old = self.db.in_tx(|tx| tx.get_by::<RepoUrl>(&self.repo.url));

        if old.as_ref().and_then(|r| r.revision) < Some(md.revision as _) {
            println!("{:?} is outdated, syncing", self.repo.url);

            syncer.sync_packages_streaming(&mut PackageScanner { base: self, packages: vec![] }, &md);
            syncer.sync_updates_streaming(&mut UpdateScanner { base: self, advs: vec![] }, &md);
            syncer.sync_modules(&mut ModuleScanner { base: self, defaults: HashMap::new(), module_ids: HashMap::new() }, &md);

            self.repo.id = old.map(|v| v.id).unwrap_or_else(|| self.db.generate_id());
            self.repo.revision = Some(md.revision as _);

            let repo = self.repo.clone();
            self.db.in_wtx(|tx| {
                tx.put(&repo);
            });
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
                tx.put_by::<PkgNevraIdx>(&mut pkg);
                tx.put(&PkgRepo(PkgRepoId { pkg_id: pkg.id, repo_id }));
            }
        });
    }
}

pub struct UpdateScanner<'a> {
    base: &'a mut RepoScanner,
    advs: Vec<(Advisory, Vec<(Pkg, Option<rpmrepo::updateinfo::Module>)>)>,
}

impl<'a> rpmsync::UpdateTarget for UpdateScanner<'a> {
    fn on_update(&mut self, up: Update) {
        let adv = Advisory {
            id: Uuid::new_v4(),
            name: up.id.clone(),
            desc: up.description,
            summary: up.summary,
            r#type: up.typ,
            issued: up.issued.date,
            updated: up.updated.date,
        };

        let mut pkgs = vec![];
        let collections = up.pkglist
            .into_iter()
            .flat_map(|i| {
                i.collection.into_iter()
            });

        for mut col in collections {
            let m = col.module.take();
            for p in col.package {
                let p = Pkg {
                    id: Uuid::new_v4(),
                    nevra: Nevra {
                        name: p.name,
                        epoch: p.epoch,
                        ver: p.version,
                        rel: p.release,
                        arch: p.arch,
                    },
                };
                pkgs.push((p, m.clone()));
            }
        }
        self.advs.push((adv, pkgs));
    }

    fn done(&mut self) {
        let repo_id = self.base.repo.id;
        let advisories = std::mem::replace(&mut self.advs, vec![]);
        self.base.db.in_wtx(|tx| {
            for (mut adv, pkgs) in advisories {
                tx.put_by::<AdvisoryNameIdx>(&mut adv);
                tx.put(&AdvisoryRepo(AdvisoryRepoId { adv_id: adv.id, repo_id }));

                for (mut pkg, module) in pkgs {
                    let stream_id = if let Some(mod_data) = module {
                        let mut newmod = Module {
                            id: Uuid::new_v4(),
                            attrs: ModuleAttrs {
                                name: mod_data.name,
                                arch: mod_data.arch,
                                repo_id,
                            },
                        };

                        tx.put_by::<ModuleAttrsIdx>(&mut newmod);

                        let mut stream = ModuleStream {
                            id: Uuid::new_v4(),
                            attrs: StreamAttrs {
                                module_id: newmod.id,
                                name: mod_data.stream,
                                version: mod_data.version,
                                context: mod_data.context,
                            },
                            // TODO: Update default from proper location
                            default: false,
                        };
                        tx.put_by::<StreamAttrsIdx>(&mut stream);
                        Some(stream.id)
                    } else { None };

                    tx.put_by::<PkgNevraIdx>(&mut pkg);
                    tx.put(&PkgRepo(PkgRepoId { pkg_id: pkg.id, repo_id }));
                    tx.put(&PkgAdvisory(PkgAdvisoryId { pkg_id: pkg.id, adv_id: adv.id, stream_id }));
                }
            }
        })
    }
}

#[allow(dead_code)]
pub struct ModuleScanner<'a> {
    base: &'a mut RepoScanner,
    module_ids: HashMap<String, Uuid>,
    defaults: HashMap<String, String>,
}

impl rpmsync::ModuleTarget for ModuleScanner<'_> {
    fn on_module_chunk(&mut self, _chunk: Chunk) {
        match _chunk {
            Chunk::ModuleMd(_md) => {
                let mut module = Module {
                    id: Uuid::new_v4(),
                    attrs: ModuleAttrs {
                        name: _md.name,
                        repo_id: self.base.repo.id,
                        arch: _md.arch,
                    },
                };

                let mut stream = ModuleStream {
                    id: Uuid::new_v4(),
                    attrs: StreamAttrs {
                        name: _md.stream,
                        context: _md.context,
                        version: _md.version,
                        module_id: Uuid::new_v4(),
                    },
                    default: false,
                };

                self.base.db.in_wtx(|tx| {
                    tx.put_by::<ModuleAttrsIdx>(&mut module);
                    stream.attrs.module_id = module.id;
                    tx.put_by_with::<StreamAttrsIdx, _>(&mut stream, |old, new| {
                        new.default = old.default
                    });
                });
                self.module_ids.insert(module.attrs.name.clone(), module.id);
            }
            Chunk::Defaults(_def) => {
                if let Some(default) = _def.stream {
                    self.defaults.insert(_def.module, default);
                }
            }
        }
    }

    fn done(&mut self) {
        let module_ids = std::mem::replace(&mut self.module_ids, HashMap::new());
        for (module, s) in std::mem::replace(&mut self.defaults, HashMap::new()) {
            self.base.db.in_wtx(|tx| {
                let streams = tx.scan().filter(|stream: &ModuleStream| {
                    stream.attrs.module_id == *module_ids.get(&module).unwrap()
                }).map(|stream| stream.clone()).collect::<Vec<_>>();

                for mut stream in streams {
                    stream.default = stream.attrs.name == *s;
                    tx.put(&stream);
                }
            });
        }
    }
}


fn main() -> Result<()> {
    env_logger::init();
    rayon::ThreadPoolBuilder::new().num_threads(32).build_global().unwrap();
    let mut scanner = Scanner::new()?;
    scanner.load_repolist(json::from_reader::<_, repolist::Repolist>(std::fs::File::open("./repolist.json")?)?)?;
    scanner.sync().unwrap();

    Ok(())
}
