mod repolist;

use rpmsync::Syncer;
use rpmrepo::repomd::RepoMD;
use rpmrepo::primary::Package;
use rpmrepo::modules::Chunk;
use rpmrepo::updateinfo::Update;

use anyhow::*;
use rpmrepo::modules::Chunk::Defaults;
use serde::{Serialize, Deserialize};
use bindb::{DBOps, Table};
use itertools::Itertools;
use std::collections::BTreeSet;
use log::*;

pub type Db = r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>;

pub struct Scanner {
    db: bindb::sled::Tree,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repo {
    content_set: String,
    url: String,
    basearch: Option<String>,
    releasever: Option<String>,
    revision: Option<i64>
}

impl bindb::Table for Repo {
    const NAME: &'static str = "repo";
    const VERSION: u8 = 0;
    type Key = String;

    fn key(&self) -> Self::Key {
        self.url.clone()
    }
}

impl Scanner {
    pub fn new() -> Result<Self> {
        Ok(Scanner {
            db: bindb::sled::open("./data.db")?.open_tree("/")?
        })
    }
    pub fn load_repolist(&mut self, rl: repolist::Repolist) -> Result<()> {
        for (p, prod) in rl.iter().flat_map(|p| &p.products) {
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
                    Repo  {
                        content_set: label.clone(),
                        url,
                        basearch: arch.cloned(),
                        releasever: rv.cloned(),
                        revision: None
                    }
                }).collect::<Vec<_>>();

                for r in &repos {
                    println!("Loading repo {:?}", r);
                    self.load_repo(r)?;
                }
            }
        }
        Ok(())
    }

    pub fn load_repo(&mut self, repo: &Repo) -> Result<()> {
        let syncer = rpmsync::Syncer::new(rpmsync::default_certs(), &format!("{}/", repo.url));
        let old: Option<Repo> = self.db.tget( &repo.url);
        println!("Old: {:?}", old);
        let mut scanner = RepoScanner {
            db: self.db.clone(),
            repo: repo.clone()
        };
        Ok(syncer.sync_md(&mut scanner)?)
    }
}

pub struct RepoScanner {
    db: bindb::sled::Tree,
    repo: Repo
}

impl rpmsync::SyncTarget for RepoScanner {
    fn on_metadata(&mut self, syncer: &Syncer, md: RepoMD) {
        if self.repo.revision < Some(md.revision as _) {
            println!("{:?} is outdated, syncing", self.repo.url);
            syncer.sync_primary_streaming(self, &md);
            self.repo.revision = Some(md.revision as _);
            self.db.tput(&self.repo);
        }
        println!("{:?}", md);
        todo!()
    }

    fn on_package(&mut self, p: Package) {
        println!("Package: {:?}", p);
        //todo!()
    }

    fn on_update(&mut self, up: Update) {
        todo!()
    }

    fn on_module_chunk(&mut self, chunk: Chunk) {
        todo!()
    }
}


fn main() -> Result<()> {
    env_logger::init();
    let mut scanner = Scanner::new()?;
    scanner.load_repolist(json::from_reader::<_, repolist::Repolist>(std::fs::File::open("./repolist.json")?)?)?;
    println!("Hello, world!");
    Ok(())
}
