## BinDB

- [Crates.io](https://crates.io/crates/bindb)
- [Documentation](https://docs.rs/bindb) 

Simple database built on [sled](https://docs.rs/sled). Uses [bincode](https://github.com/servo/bincode)
as an internal  format and provides:

* Automatic serialization/deserialization of values to/from bytes
* Table abstraction
* Range scans
* Full table scans

Goal is to use as simplest possible storage for transient cache-able data with some structure without using sqlite
(for a change. I seem to encounter a need for it in every project I do). 

While we do some amount of copying bytes, the library aims to be zero-clone in order to avoid calling unnecessary 
constructors and  drops.