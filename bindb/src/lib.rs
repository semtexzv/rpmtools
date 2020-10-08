#![feature(slice_fill)]
pub use sled;
use serde::{Serialize, de::DeserializeOwned};
use std::ops::{Deref, Range};
use bincode::Options;
use bincode::config::{BigEndian, LittleEndian, FixintEncoding};

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
}

/*
#[derive(Debug, Serialize, Deserialize)]
#[serde(bound(deserialize = "T: DeserializeOwned"))]
pub struct Versioned<T: Table> {
    version: u8,
    data: T,
}

impl<T: Table> Versioned<T> {
    pub fn new(t: T) -> Self {
        Self {
            version: T::VERSION,
            data: t,
        }
    }
}
*/

fn key_opts() -> bincode::config::WithOtherIntEncoding<bincode::config::WithOtherEndian<bincode::DefaultOptions, BigEndian>, FixintEncoding> {
    bincode::DefaultOptions::default().with_big_endian().with_fixint_encoding()
}

fn val_opts() -> bincode::config::WithOtherEndian<bincode::DefaultOptions, LittleEndian> {
    bincode::DefaultOptions::default().with_little_endian()
}

/// Trait denoting an engine capable of providing storage for tables
pub trait Database {
    /// Serializes the key and retrieves what was stored using this key
    fn tget<T: Table>(&self, k: &T::Key) -> Option<T>;

    // Performs a range scan over a part of the table. TODO: Use iterators, don't collect
    fn trange<T: Table>(&self, range: Range<&T::Key>) -> Vec<T>;

    /// Serializes the key and value using proper formats(Big endian for key, little endian for values)
    /// and safely stores this entry in the database
    fn tput<T: Table>(&self, k: &T::Key, v: &T) -> Option<T>;
    /// Performs a full table scan of a specified table
    fn tscan<T: Table>(&self) -> Vec<T>;
}

fn key_bytes_empty<T: Table>() -> Vec<u8> {
    key_opts().serialize(&(T::NAME, ())).unwrap()
}

fn key_bytes<T: Table>(k: &T::Key) -> Vec<u8> {
    return key_opts().serialize(&(T::NAME, k)).unwrap();
}

impl Database for sled::Tree {
    fn tget<T: Table>(&self, k: &T::Key) -> Option<T> {
        let key = key_bytes::<T>(k);

        let val = self.get(&key).unwrap();
        val.map(|val| {
            let res = val_opts().deserialize::<T>(val.deref()).unwrap();
            /*
            if res.version != T::VERSION {
                panic!("Invalid data version, migration needed")
            }
             */
            res
        })
    }

    fn trange<T: Table>(&self, range: Range<&T::Key>) -> Vec<T> {
        let start = key_bytes::<T>(range.start);
        let end = key_bytes::<T>(range.end);
        let range = Range {
            start: start.as_slice(),
            end: end.as_slice(),
        };


        println!("{:?}", range);

        self.range::<&[u8], _>(range).values().map(|a| {
            val_opts().deserialize(a.unwrap().as_ref()).unwrap()
        }).collect()
    }

    fn tput<T: Table>(&self, k: &T::Key, v: &T) -> Option<T> {
        let key = key_bytes::<T>(k);

        let value = val_opts().serialize(v).unwrap();

        self.insert(key.deref(), value.deref()).unwrap()
            .map(|v| val_opts().deserialize(v.deref()).unwrap())
    }

    fn tscan<T: Table>(&self) -> Vec<T> {
        let key = key_bytes_empty::<T>();

        self.scan_prefix(&key).values().map(|v| {
            val_opts().deserialize(v.unwrap().as_ref()).unwrap()
        }).collect()
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
    }

    std::fs::remove_dir_all("/tmp/db.test").unwrap();
    let db = sled::open("/tmp/db.test").unwrap();
    assert!(db.tput(&Key(0, 0), &Key(0, 0)).is_none());
    assert!(db.tput(&Key(0, 0), &Key(0, 0)).is_some());
    assert!(db.tput(&Key(1, 0), &Key(1, 0)).is_none());
    assert!(db.tput(&Key(2, 0), &Key(2, 0)).is_none());
    assert!(db.tput(&Key(4, 0), &Key(4, 0)).is_none());

    assert_eq!(Database::tget(db.deref(), &Key(2, 0)), Some(Key(2, 0)));

    let range = db.tscan::<Key>();
    assert_eq!(range.len(), 4);

    let range = db.trange::<Key>(&Key(0, 0)..&Key(2, 0));
    assert_eq!(range.len(), 2);
}

#[test]
fn test_order() {
    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
    struct Key(String, String);

    impl Table for Key {
        const NAME: &'static str = "key2";
        const VERSION: u8 = 0;
        type Key = Self;
    }
    let _ = std::fs::remove_dir_all("/tmp/db.test2");
    let db = sled::open("/tmp/db.test2").unwrap();

    let empty = "".to_string();
    let a = "a".to_string();
    let b = "b".to_string();
    let ab = "ab".to_string();
    let abc = "abc".to_string();

    assert!(db.tput(&Key(empty.clone(), empty.clone()), &Key(empty.clone(), empty.clone())).is_none());
    assert!(db.tput(&Key(empty.clone(), empty.clone()), &Key(empty.clone(), empty.clone())).is_some());

    assert!(db.tput(&Key(a.clone(), empty.clone()), &Key(a.clone(), empty.clone())).is_none());
    assert!(db.tput(&Key(b.clone(), empty.clone()), &Key(b.clone(), empty.clone())).is_none());
    assert!(db.tput(&Key(b.clone(), abc.clone()), &Key(b.clone(), abc.clone())).is_none());
    assert!(db.tput(&Key(ab.clone(), empty.clone()), &Key(ab.clone(), empty.clone())).is_none());
    assert!(db.tput(&Key(ab.clone(), abc.clone()), &Key(ab.clone(), abc.clone())).is_none());

    let all = db.tscan::<Key>();
    panic!("{:?}", all);
}