#![feature(associated_type_defaults)]
#![feature(generic_associated_types)]
#![feature(trace_macros)]

use serde::{Serialize, de::DeserializeOwned};
use std::path::Path;
use std::collections::{HashMap};
use heed::{RoTxn, RwTxn};
use heed::types::{SerdeBincode, SerdeJson, CowSlice, DecodeIgnore};

type KeyType<T> = SerdeBincode<<T as Table>::Key>;
type ValType<T> = SerdeJson<T>;


/// Types which should be stored.
pub trait Table: Serialize + DeserializeOwned {
    /// Name of the table. This should be unique within database
    const NAME: &'static str;

    /// Primary key of the table. If this type is sane, then it should have same ordering
    /// in rust as it does in its bincode serialized form. Simply - Fields sorted in-order
    /// numeric values sorted naturally, and strings lexicographically.
    type Key: PartialOrd + Serialize + DeserializeOwned;

    fn get(&self) -> &Self::Key;
    fn get_mut(&mut self) -> &mut Self::Key;

    type Indices: Indices<Self> = ();
}



#[macro_export]
macro_rules! table {
    ($name:ty $(=> $p:tt)+ ($type:ty) $(,$idx:ty)*) => {
        impl Table for $name {
            const NAME: &'static str = stringify!($name);
            type Key = $type;
            type Indices = ($($idx,)*);
            fn get(&self) -> &Self::Key {
                &self.$($p).*
            }
            fn get_mut(&mut self) -> &mut Self::Key {
                &mut self.$($p).*
            }
        }
    };
}

pub trait Index {
    type Table: Table;
    /// Name of the table. This should be unique within database
    const NAME: &'static str;
    type Key: PartialOrd + Serialize + DeserializeOwned;
    type KeyRef<'a>: PartialOrd + Serialize;

    fn get<'a>(t: &'a Self::Table) -> Self::KeyRef<'a>;
}

#[macro_export]
macro_rules! index {
    ($name:ident, $src:ty, $(unique,)? $($($p:ident).+ : $type:ty),+) => {
        pub struct $name {}
        impl Index for $name {
            type Table = $src;
            const NAME: &'static str = concat!(stringify!($name) $($(, "_", stringify!($p))*)+);
            type Key = ( $($type),+ );
            type KeyRef<'a> = ( $(&'a $type),+ );

            fn get<'a>(t : &'a Self::Table) -> Self::KeyRef<'a> {
                ( $( &t.$($p).+),+ )
            }
        }
    };
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
    for_tuples!(where #(Tuple: Index<Table=T>)*);

    fn on_register(mut db: Database) -> Database {
        for_tuples!( #( db = db.register_idx::<Tuple>();)* );
        db
    }

    #[inline(always)]
    fn on_insert<'a>(db: &Database, tx: &mut RwTxn<'a, 'a>, t: &T) {
        for_tuples!( #(
            let db_inner = db.index_db_ref::<Tuple>();
            db_inner.put(tx, &Tuple::get(&t), Tuple::Table::get(&t)).unwrap();
        )*);
    }

    fn on_delete<'a>(db: &Database, tx: &mut RwTxn<'a, 'a, ()>, t: &T) {
        for_tuples!( #(
            let inner_db = db.index_db_ref::<Tuple>();
            inner_db.delete(tx, &Tuple::get(&t)).unwrap();
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
        unsafe {
            std::fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .write(true)
                .open(&f)
                .unwrap();

            let db = heed::EnvOpenOptions::new()
                .max_dbs(256)
                .max_readers(32)
                .map_size(1024 * 1024 * 1024 * 1024)
                .flag(heed::flags::Flags::MdbNoSubDir)
                .open(f)
                .unwrap();

            Database {
                tree: db,
                dbs: HashMap::new(),
            }
        }
    }

    pub fn register<T: Table>(mut self) -> Self {
        let db = self.tree.create_database(Some(T::NAME)).unwrap();
        self.dbs.insert(T::NAME.to_string(), db);
        T::Indices::on_register(self)
    }

    pub fn register_idx<I: Index>(mut self) -> Self {
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

    pub fn wtx(&self) -> Wtx<'_> {
        Wtx {
            db: self,
            tx: self.tree.write_txn().unwrap(),
        }
    }

    pub fn in_wtx<R, F: FnOnce(&mut Wtx) -> R>(&self, f: F) -> R {
        let mut tx = self.wtx();
        let res = f(&mut tx);
        tx.commit();
        return res;
    }
}


impl Database {
    pub fn untyped_db<T : Table>(&self) -> heed::Database<DecodeIgnore, DecodeIgnore> {
        self.dbs.get(T::NAME).expect("Table not registered").remap_types()
    }
    pub fn typed_db<T: Table>(&self) -> heed::Database<SerdeBincode<T::Key>, SerdeJson<T>> {
        self.dbs.get(T::NAME).expect("Table not registered").remap_types()
    }
    pub fn index_db<I: Index>(&self) -> heed::Database<SerdeBincode<I::Key>, SerdeBincode<<I::Table as Table>::Key>> {
        self.dbs.get(I::NAME).expect("Index not registered").remap_types()
    }
    pub fn index_db_ref<'a, I : Index>(&self) -> heed::Database<SerdeBincode<I::KeyRef<'a>>, SerdeBincode<<I::Table as Table>::Key>> {
        self.dbs.get(I::NAME).expect("Index not registered").remap_types()
    }

    pub fn generate_id(&self) -> uuid::Uuid {
        uuid::Uuid::new_v4()
    }
}

pub struct Iter<'a, T: Table> {
    i: heed::RoRange<'a, KeyType<T>, ValType<T>>,
}

impl<'a, T: Table + 'static> Iterator for Iter<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.i.next().map(|v| v.unwrap().1)
    }
}

pub trait ROps {
    fn _ro_tx(&self) -> (&Database, &RoTxn);

    /// Index lookup
    fn get_by<'a, I: Index>(&self, ikey: I::KeyRef<'a>) -> Option<I::Table> {
        let (db, tx) = self._ro_tx();
        let idb = db.index_db_ref::<'a, I>();
        let tdb = db.typed_db::<I::Table>();

        if let Some(pkey) = idb.get(tx, &ikey).unwrap() {
            tdb.get(tx, &pkey).unwrap()
        } else {
            None
        }
    }

    /// Perform a primary key lookup
    fn get<T: Table>(&self, k: &T::Key) -> Option<T> {
        let (db, tx) = self._ro_tx();
        let db = db.typed_db::<T>();

        let res = db.get(&tx, k).unwrap();
        res
    }

    /// Perform a full table scan
    fn scan<T: Table + 'static>(&self) -> Iter<T> {
        let (db, tx) = self._ro_tx();

        let db = db.typed_db::<T>();
        let r = db.range(&tx, &(..)).unwrap();

        Iter {
            i: r
        }
    }
}

pub trait RwOps<'a>: ROps {
    fn _rw_tx(&mut self) -> (&Database, &mut RwTxn<'a, 'a>);

    fn put<T: Table>(&mut self, v: &T) {
        let (dd, mut tx) = self._rw_tx();
        let db = dd.typed_db::<T>();
        db.put(&mut tx, &T::get(&v), &v).unwrap();
        T::Indices::on_insert(&dd, &mut tx, &v);
    }

    /// Find and entry based on the index, if found, overwrite it and modify object id
    fn put_by<I: Index>(&mut self, v: &mut I::Table)
        where <<I as Index>::Table as Table>::Key: Clone
    {
        self.put_by_with::<I, _>(v, |old, v| {
            *I::Table::get_mut(v) = I::Table::get(&old).clone();
        })
    }

    /// Overwrite old entry using an index as key,
    fn put_by_with<I, F>(&mut self, v: &mut I::Table, patch: F)
        where I: Index, F: FnOnce(&I::Table, &mut I::Table)
    {
        if let Some(old) = self.get_by::<I>(I::get(v)) {
            patch(&old, v);
        }
        self.put(v)
    }

    fn delete<T: Table>(&mut self, k: &T::Key) {
        let (db, mut tx) = self._rw_tx();
        let typed = db.typed_db::<T>();

        if let Some(item) = typed.get(&tx, k).unwrap() {
            // If the entry was stored, first update index table and only after that delete the entry
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
    pub fn commit(self) {
        self.tx.commit().unwrap();
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
    pub fn commit(self) {
        self.tx.commit().unwrap();
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
    use serde::{Serialize, Deserialize};
    use crate::{ROps, RwOps};

    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
    struct Item(usize, usize);
    table!(Item => 0(usize));

    let mut db = Database::open("/tmp/db").register::<Item>();
    {
        let mut db = db.wtx();
        db.put(&Item(0, 0));
        db.put(&Item(1, 0));
        db.put(&Item(2, 0));
        db.put(&Item(4, 0));
        db.put(&Item(0, 0));
        db.commit();
    }
    let mut db = db.wtx();
    assert_eq!(db.get(&2), Some(Item(2, 0)));

    let range = db.scan::<Item>();
    assert_eq!(range.count(), 4);

    db.delete::<Item>(&0);
    assert_eq!(db.scan::<Item>().count(), 3);
}
