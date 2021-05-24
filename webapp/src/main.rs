use bindb::{Database, ROps};
use cache::*;

fn main() {
    let db = Database::open("data.mdbx")
        .register::<Repo>()
        .register::<Pkg>()
        .register::<Advisory>()
        .register::<Module>()
        .register::<ModuleStream>()
        .register::<PkgAdvisory>()
        .register::<PkgRepo>()
        .register::<AdvisoryRepo>();

    db.in_tx(|tx| {
        println!("We have {:?} packages", tx.scan::<Pkg>().count());
    });

    println!("Hello, world!");
}
