use crate::prelude;
use serde::de::{DeserializeSeed, Visitor, MapAccess, SeqAccess};
use serde::{Deserializer, Deserialize};
use std::fmt;


/// A helper struct that runs a specific DeserializeSeed on a struct field
pub struct SeedField<T> {
    names: &'static [&'static str],
    inner: T,
}

impl<T> SeedField<T> {
    pub fn new(field: &'static [&'static str], seed: T) -> Self {
        Self {
            names: field,
            inner: seed,
        }
    }
}

impl<'de, T: DeserializeSeed<'de>> DeserializeSeed<'de> for SeedField<T> {
    type Value = T::Value;


    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error> where D: Deserializer<'de> {
        deserializer.deserialize_struct("T", self.names, self)
    }
}

impl<'de, T: DeserializeSeed<'de>> Visitor<'de> for SeedField<T> {
    type Value = T::Value;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("Expected map")
    }


    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        use serde::de::Error;
        while let Some(k) = map.next_key::<String>()? {
            if k == self.names[0] {
                return map.next_value_seed(self.inner);
            } else {
                map.next_value::<serde::de::IgnoredAny>()?;
            }
        }
        return Err(A::Error::missing_field(self.names[0]));
    }
}

/// Should we continue deserializing more items, or end with the response of [R]
pub enum IterState<R> {
    Continue,
    Break(R),
}

/// A helper struct that runs a specific action on each item of sequence
pub struct ItemAction<'a, T, R> {
    pub(crate) action: &'a mut dyn FnMut(T) -> IterState<R>,
}

impl<'a, T, R> ItemAction<'a, T, R> {
    pub fn new(f: &'a mut dyn FnMut(T) -> IterState<R>) -> ItemAction<'a, T, R> {
        Self {
            action: f
        }
    }
}

impl<'a, T: Deserialize<'a>, R> DeserializeSeed<'a> for ItemAction<'a, T, R> {
    type Value = Option<R>;

    fn deserialize<D: Deserializer<'a>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'v, T: Deserialize<'v>, R> Visitor<'v> for ItemAction<'v, T, R> {
    type Value = Option<R>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("A sequence of items")
    }

    fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
        where S: SeqAccess<'v>,
    {
        while let Some(elem) = seq.next_element()? {
            if let IterState::Break(res) = (self.action)(elem) {
                return Ok(Some(res));
            }
        }

        Ok(None)
    }
}
