#![feature(associated_type_defaults)]
#![feature(trace_macros)]

mod query;

pub use sled;
pub use field_ref::{FieldRef, field_ref_of};
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use std::ops::{Deref, Range};
use bincode::Options;
use bincode::config::{LittleEndian};
use std::marker::PhantomData;
use std::path::Path;
use sled::IVec;


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
    fn on_insert(db: &Database, t: &T);
}

#[impl_trait_for_tuples::impl_for_tuples(6)]
#[tuple_types_no_default_trait_bound]
impl<T> Indices<T> for Tuple
    where T: Table,
{
    fn on_insert(db: &Database, t: &T) {
        for_tuples!( #( db.update_idx::<T, Tuple>(&t);)*);
    }
    for_tuples!(where #(Tuple: Index<T>)*);
}

#[derive(Clone)]
pub struct Database {
    tree: sled::Db
}

impl Database {
    pub fn open(f: impl AsRef<Path>) -> Self {
        Database {
            tree: sled::open(f.as_ref()).unwrap()
        }
    }
    pub fn from_tree(tree: sled::Db) -> Self {
        Database {
            tree
        }
    }

    pub fn generate_id(&self) -> u64 {
        self.tree.generate_id().unwrap()
    }
}


fn val_opts() -> bincode::config::WithOtherEndian<bincode::DefaultOptions, LittleEndian> {
    bincode::DefaultOptions::default().with_little_endian()
}

fn key_bytes_empty<T: Table>() -> Vec<u8> {
    bytekey::serialize(&(T::NAME, T::VERSION, ())).unwrap()
}


fn index_key_bytes<T: Table, I: Index<T>>(k: &I::Key) -> Vec<u8> {
    return bytekey::serialize(&(I::NAME, T::VERSION, k)).unwrap();
}

fn key_bytes<T: Table>(k: &T::Key) -> Vec<u8> {
    return bytekey::serialize::<(String, u8, &T::Key)>(&(T::NAME.to_string(), T::VERSION, k)).unwrap();
}

fn from_key_bytes<T: Table>(k: &[u8]) -> T::Key {
    let res = bytekey::deserialize::<(String, u8, T::Key)>(k).unwrap();
    res.2
}

impl Database {
    pub fn find_by<T: Table, I: Index<T>>(&self, k: &I::Key) -> Option<T> {
        let key = index_key_bytes::<T, I>(k);
        let val = self.tree.get(&key).unwrap();

        // Index stores primary key of table
        let table_key = val.map(|val| from_key_bytes::<T>(val.deref()));

        table_key.and_then(|key| {
            self.get(&key)
        })
    }

    pub fn update_idx<T: Table, I: Index<T>>(&self, v: &T) {
        let ik = I::key().get(&v);
        let key = index_key_bytes::<T, I>(ik);

        let vk = T::key().get(&v);
        let val = key_bytes::<T>(vk);
        self.tree.insert(key, val);
    }


    pub fn get<T: Table>(&self, k: &T::Key) -> Option<T> {
        let key = key_bytes::<T>(k);

        let val = self.tree.get(&key).unwrap();
        val.map(|val| {
            let res = val_opts().deserialize::<T>(val.deref()).unwrap();
            res
        })
    }

    pub fn range<T: Table>(&self, range: Range<&T::Key>) -> Iter<T> {
        let start = key_bytes::<T>(range.start);
        let end = key_bytes::<T>(range.end);
        let range = Range {
            start: start.as_slice(),
            end: end.as_slice(),
        };

        return Iter {
            iter: self.tree.range::<&[u8], _>(range),
            _m: PhantomData,
        };
    }
    pub fn put<T: Table>(&self, v: &T) -> Option<T> {
        let k = T::key().get(v);
        let key = key_bytes::<T>(&k);

        let value = val_opts().serialize(v).unwrap();

        T::Indices::on_insert(self, v);

        self.tree.insert(key.deref(), value.deref()).unwrap()
            .map(|v| val_opts().deserialize(v.deref()).unwrap())
    }

    pub fn delete_by<T: Table>(&self, k: &T::Key) {
        self.tree.remove(key_bytes::<T>(k));
    }

    pub fn delete_all<T: Table>(&self, items: impl Iterator<Item=T>) {
        for i in items {
            self.tree.remove(key_bytes::<T>(T::key().get(&i)));
        }
    }

    pub fn scan<T: Table>(&self) -> Iter<T> {
        let key = key_bytes_empty::<T>();

        return Iter {
            iter: self.tree.scan_prefix(&key),
            _m: PhantomData,
        };
    }
}

pub struct Iter<T> {
    iter: sled::Iter,
    _m: PhantomData<T>,
}

impl<T: Table> Iterator for Iter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.iter.next();
        return item.map(|item| {
            val_opts().deserialize(item.unwrap().1.as_ref()).unwrap()
        });
    }
}

#[test]
fn test_simple() {
    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
    struct Key(usize, usize);

    impl Table for Key {
        const NAME: &'static str = "key";
        const VERSION: u8 = 0;
        type Key = Self;

        fn key(&self) -> Self::Key {
            Self(self.0, self.1)
        }
    }

    let _ = std::fs::remove_dir_all("/tmp/db.test");
    let db = Database::open("/tmp/db.test");

    assert!(db.put(&Key(0, 0)).is_none());
    assert!(db.put(&Key(0, 0)).is_some());
    assert!(db.put(&Key(1, 0)).is_none());
    assert!(db.put(&Key(2, 0)).is_none());
    assert!(db.put(&Key(4, 0)).is_none());

    assert_eq!(db.get(&Key(2, 0)), Some(Key(2, 0)));

    let range = db.scan::<Key>();
    assert_eq!(range.count(), 4);

    let range = db.range::<Key>(&Key(0, 0)..&Key(2, 0));
    assert_eq!(range.count(), 2);
}

#[test]
fn test_order() {
    #[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
    struct Key(String, String);

    impl Table for Key {
        const NAME: &'static str = "key2";
        const VERSION: u8 = 0;
        type Key = Self;

        fn key(&self) -> Self::Key {
            self.clone()
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