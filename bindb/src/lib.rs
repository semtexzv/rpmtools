#![feature(associated_type_defaults)]
#![feature(trace_macros)]

mod query;

pub use sled;
pub use field_ref::{FieldRef, field_ref_of};
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use std::ops::{Deref, Range, RangeBounds};
use bincode::Options;
use bincode::config::{LittleEndian};
use std::marker::PhantomData;
use std::path::Path;
use sled::IVec;
use std::collections::{BTreeMap, HashMap};
use heed::{RoTxn, RwTxn};


type KeyType<T: Table> = heed::types::SerdeBincode<T::Key>;
type ValType<T: Table> = heed::types::SerdeJson<T>;


/// Types which should be stored.
pub trait Table: Serialize + DeserializeOwned {
    /// Name of the table. This should be unique within database
    const NAME: &'static str;
    /// Version of the schema - This is here for future support for migrations of
    const VERSION: u8;

    /// Primary key of the table. If this type is sane, then it should have same ordering
    /// in rust as it does in its bincode serialized form. Simply - Fields sorted in-order
    /// numeric values sorted naturally, and strings lexicographically.
    type Key: PartialOrd + Serialize + DeserializeOwned;

    fn key() -> FieldRef<Self, Self::Key>;
    type Indices: Indices<Self> = ();
}

pub trait Index<T: Table> {
    /// Name of the table. This should be unique within database
    const NAME: &'static str;
    type Key: PartialOrd + Serialize + DeserializeOwned;
    fn key() -> FieldRef<T, Self::Key>;
}


pub trait Indices<T> {
    fn on_register(db: Database) -> Database;
    fn on_insert<'a>(db: &Database, tx: &mut RwTxn<'a, 'a>, t: &T);
    fn on_delete<'a>(db: &Database, tx: &mut RwTxn<'a, 'a>, t: &T);
}

#[impl_trait_for_tuples::impl_for_tuples(6)]
#[tuple_types_no_default_trait_bound]
impl<T> Indices<T> for Tuple
where T: Table,
{

    fn on_register(mut db: Database) -> Database {
        for_tuples!( #( db = db.register_idx::<T, Tuple>();)* );
        db
    }

    for_tuples!(where #(Tuple: Index<T>)*);

    #[inline(always)]
    fn on_insert<'a>(db: &Database, tx: &mut RwTxn<'a, 'a>, t: &T) {
        for_tuples!( #(
            let db_inner = db.index_db::<T, Tuple>();
            db_inner.put(tx, Tuple::key().get(&t), T::key().get(&t)).unwrap();
        )*);
    }

    fn on_delete<'a>(db: &Database, tx: &mut RwTxn<'a, 'a, ()>, t: &T) {
        for_tuples!( #(
            let inner_db = db.index_db::<T, Tuple>();
            inner_db.delete(tx, Tuple::key().get(&t)).unwrap();
        )*);
    }
}

#[derive(Clone)]
pub struct Database {
    tree: heed::Env,
    dbs: HashMap<String, heed::UntypedDatabase>,
}

impl Database {
    pub fn open(f: impl AsRef<Path>) -> Self {
        let db = heed::EnvOpenOptions::new()
            .max_dbs(256)
            .max_readers(8)
            .map_size(1024 * 1024 * 1024  * 1024)
            .open(f)
            .unwrap();

        Database {
            tree: db,
            dbs: HashMap::new(),
        }
    }

    pub fn register<T: Table>(mut self) -> Self {
        let db = self.tree.create_database(Some(T::NAME)).unwrap();
        self.dbs.insert(T::NAME.to_string(), db);
        T::Indices::on_register(self)
    }

    pub fn register_idx<T: Table, I: Index<T>>(mut self) -> Self {
        let db = self.tree.create_database(Some(I::NAME)).unwrap();
        self.dbs.insert(I::NAME.to_string(), db);
        self
    }

    pub fn tx(&self) -> Tx<'_> {
        Tx {
            db: self,
            tx: self.tree.read_txn().unwrap(),
        }
    }

    pub fn in_tx<R, F: FnOnce(&Tx) -> R>(&self, f: F) -> R {
        let tx = self.tx();
        let res = f(&tx);
        tx.commit();
        return res;
    }

    pub fn wtx(&mut self) -> Wtx<'_> {
        Wtx {
            db: self,
            tx: self.tree.write_txn().unwrap(),
        }
    }

    pub fn in_wtx<R, F: FnOnce(&mut Wtx) -> R>(&mut self, f: F) -> R {
        let mut tx = self.wtx();
        let res = f(&mut tx);
        tx.commit();
        return res;
    }
}


impl Database {
    pub fn typed_db<T: Table>(&self) -> heed::Database<heed::types::SerdeBincode<T::Key>, heed::types::SerdeJson<T>> {
        self.dbs.get(T::NAME).expect("Not registered").remap_types()
    }
    pub fn index_db<T: Table, I: Index<T>>(&self) -> heed::Database<heed::types::SerdeBincode<I::Key>, heed::types::SerdeBincode<T::Key>> {
        self.dbs.get(I::NAME).expect("index not registered").remap_types()
    }
    pub fn generate_id(&self) -> uuid::Uuid {
        uuid::Uuid::new_v4()
    }
}

pub struct Iter<'a, T: Table> {
    i: heed::RoRange<'a, KeyType<T>, ValType<T>>
}

impl<'a, T: Table + 'static> Iterator for Iter<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.i.next().map(|v| v.unwrap().1)
    }
}

pub trait ROps {
    fn _ro_tx(&self) -> (&Database, &RoTxn);

    fn get_by<T: Table, I: Index<T>>(&self, ikey: &I::Key) -> Option<T> {
        let (db, tx) = self._ro_tx();
        let idb = db.index_db::<T, I>();
        let tdb = db.typed_db::<T>();
        if let Some(pkey) = idb.get(tx, ikey).unwrap() {
            tdb.get(tx, &pkey).unwrap()
        } else {
            None
        }
    }

    fn get<T: Table>(&self, k: &T::Key) -> Option<T> {
        let (db, tx) = self._ro_tx();
        let db = db.typed_db::<T>();
        let res = db.get(&tx, k).unwrap();
        res
    }

    fn scan<T: Table + 'static, B: RangeBounds<T::Key>>(&self, range: B) -> Iter<T> {
        let (db, tx) = self._ro_tx();

        let db = db.typed_db::<T>();
        let r = db.range(&tx, &range).unwrap();

        Iter {
            i: r
        }
    }
}

pub trait RwOps<'a> {
    fn _rw_tx(&mut self) -> (&Database, &mut RwTxn<'a, 'a>);

    #[inline(always)]
    fn put<T: Table>(&mut self, v: &T) {
        let (dd, mut tx) = self._rw_tx();
        let db = dd.typed_db::<T>();
        db.put(&mut tx, &T::key().get(&v), &v);
        T::Indices::on_insert(&dd, &mut tx, &v);
    }

    fn delete<T: Table>(&mut self, k: &T::Key) {
        let (db, mut tx) = self._rw_tx();
        let typed = db.typed_db::<T>();

        if let Some(item) = typed.get(&tx, k).unwrap() {
            // If the entry was stored, first update indices and only after that delete the entry
            T::Indices::on_delete(&db, &mut tx, &item);
        }
        typed.delete(&mut tx, k).unwrap();
    }
}

pub struct Tx<'a> {
    db: &'a Database,
    tx: RoTxn<'a>,
}

impl<'a> Tx<'a> {
    pub fn commit(mut self) {
        self.tx.commit();
    }
}

impl<'a> ROps for Tx<'a> {
    fn _ro_tx(&self) -> (&Database, &RoTxn) {
        (&self.db, &self.tx)
    }
}

pub struct Wtx<'a> {
    db: &'a Database,
    tx: RwTxn<'a, 'a>,
}

impl<'a> Wtx<'a> {
    pub fn commit(mut self) {
        self.tx.commit();
    }
}


impl<'a> ROps for Wtx<'a> {
    fn _ro_tx(&self) -> (&Database, &RoTxn) {
        (&self.db, &self.tx)
    }
}

impl<'a> RwOps<'a> for Wtx<'a> {
    fn _rw_tx(&mut self) -> (&Database, &mut RwTxn<'a, 'a>) {
        (&self.db, &mut self.tx)
    }
}

#[test]
fn test_simple() {
    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
    struct Key(usize, usize);

    impl Table for Key {
        const NAME: &'static str = "key";
        const VERSION: u8 = 0;
        type Key = usize;

        fn key() -> FieldRef<Self, Self::Key> {
            field_ref_of!(Self => 0)
        }
    }

    let _ = std::fs::remove_dir_all("./tmp/db/");
    std::fs::create_dir_all("./tmp/db/");
    let mut db = Database::open("./tmp/db/").register::<Key>();
    {
        let mut db = db.wtx();
        db.put(&Key(0, 0));
        db.put(&Key(0, 0));
        db.put(&Key(1, 0));
        db.put(&Key(2, 0));
        db.put(&Key(4, 0));
        db.commit();
    }
    let db = db.tx();
    assert_eq!(db.get(&2), Some(Key(2, 0)));

    let range = db.scan::<Key, _>(..);
    assert_eq!(range.count(), 4);

    let range = db.scan::<Key, _>(&0..&2);
    assert_eq!(range.count(), 2);
}

/*
#[test]
fn test_order() {
    #[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
    struct Key(String, String);

    impl Table for Key {
        const NAME: &'static str = "key2";
        const VERSION: u8 = 0;
        type Key = String;

        fn key() -> FieldRef<Self, Self::Key> {
            field_ref_of!(Self => 0)
        }
    }
    let _ = std::fs::remove_dir_all("/tmp/db.test2");
    let db = Database::open("/tmp/db.test2");

    let empty = "".to_string();
    let a = "a".to_string();
    let b = "b".to_string();
    let ab = "ab".to_string();
    let abc = "abc".to_string();

    assert!(db.put(&Key(empty.clone(), empty.clone())).is_none());
    assert!(db.put(&Key(empty.clone(), empty.clone())).is_some());

    assert!(db.put(&Key(a.clone(), empty.clone())).is_none());
    assert!(db.put(&Key(b.clone(), empty.clone())).is_none());
    assert!(db.put(&Key(b.clone(), abc.clone())).is_none());
    assert!(db.put(&Key(ab.clone(), empty.clone())).is_none());
    assert!(db.put(&Key(a.clone(), abc.clone())).is_none());

    let all = db.scan::<Key>().collect::<Vec<_>>();
    let mut sorted = all.clone();
    sorted.sort();
    assert_eq!(all, sorted);
    //panic!("{:?}", all);
}

 */