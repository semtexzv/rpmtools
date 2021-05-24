

pub(crate) mod prelude;

pub mod repomd;
pub mod primary;
pub mod updateinfo;
pub mod modules;

pub use repomd::*;
pub use primary::*;
pub use modules::*;
pub use updateinfo::*;
/*
#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub enum VerFragment<T: Borrow<str>> {
    Text(T),
    Num(usize),
}

impl<T: Borrow<str>> Serialize for VerFragment<T> {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error> where
        S: Serializer {
        if serializer.is_human_readable() {
            match self {
                Self::Text(t) => serializer.serialize_str(t.borrow()),
                Self::Num(n) => serializer.serialize_u64(*n as _),
            }
        } else {
            panic!()
        }
    }
}

impl<'de> Deserialize<'de> for VerFragment<String> {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error> where
        D: Deserializer<'de> {
        struct Vis {}
        impl<'de> Visitor<'de> for Vis {
            type Value = VerFragment<String>;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                write!(formatter, "Text, num or bytes")
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> where
                E: Error, {
                Ok(VerFragment::Num(v as usize))
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> where
                E: Error, {
                Ok(VerFragment::Num(v as usize))
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> where
                E: Error, {
                Ok(VerFragment::Text(v.to_string()))
            }
        }
        deserializer.deserialize_any(Vis {})
    }
}

impl<T> VerFragment<T>
    where T: Borrow<str>
{
    pub fn map_text<O: Borrow<str>, F: FnOnce(T) -> O>(self, f: F) -> VerFragment<O> {
        match self {
            VerFragment::Text(t) => VerFragment::Text(f(t)),
            VerFragment::Num(n) => VerFragment::Num(n)
        }
    }
}

pub fn parse_ver(ver: &str) -> Result<Vec<VerFragment<&str>>, ()> {
    let text = map(take_while1(is_alphabetic), |t| {
        VerFragment::Text(unsafe { std::str::from_utf8_unchecked(t) })
    });

    let num = map(take_while1(is_digit), |t| {
        VerFragment::Num(unsafe { std::str::from_utf8_unchecked(t) }.parse().unwrap())
    });

    let other = take_while(|c: u8| !c.is_ascii_alphanumeric());
    let other1 = take_while(|c: u8| !c.is_ascii_alphanumeric());

    let fragment = delimited(other, alt((text, num)), other1);

    if ver.is_ascii() {
        let p: IResult<_, _> = complete(many1(fragment))(ver.as_bytes());
        if let Ok((rest, data)) = p {
            return Ok(data);
        } else {
            return Err(());
        }
    } else {
        panic!("Not ascii version");
    }
}

#[derive(Debug, Clone)]
pub struct PackedVersion(pub Vec<VerFragment<String>>);

impl Serialize for PackedVersion {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error> where
        S: Serializer {
        let mut bytes = vec![];
        for s in &self.0 {
            match s {
                VerFragment::Num(n) => {
                    bytes.push(0xFF);
                    bytes.extend_from_slice(&n.to_be_bytes());
                }
                VerFragment::Text(t) => {
                    bytes.push(0xFE);
                    bytes.extend_from_slice(t.as_bytes());
                }
            }
        }
        serializer.serialize_bytes(&bytes)
    }
}

impl<'de> Deserialize<'de> for PackedVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error> where
        D: Deserializer<'de> {
        struct Vis {}
        impl<'de> Visitor<'de> for Vis {
            type Value = PackedVersion;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                write!(formatter, "bytes")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E> where
                E: Error, {
                let num = map(preceded(tag([0xFF as u8]), be_u64), |v| {
                    VerFragment::Num(v as _)
                });
                let txt = map(preceded(tag([0xFE as u8]), take_while1(is_alphanumeric)), |v| {
                    VerFragment::Text(std::str::from_utf8(v).unwrap().to_string())
                });
                let items: IResult<_, _> = many1(alt((num, txt)))(v);
                return Ok(PackedVersion(items.unwrap().1));
            }
        }
        deserializer.deserialize_any(Vis {})
    }
}

#[test]
fn test_ver_parse() {
    let ver = PackedVersion(parse_ver("00.1ab.0f11").unwrap().into_iter().map(|m| m.map_text(ToString::to_string)).collect());
    let data = serde_json::to_string(&ver).unwrap();
    let back: PackedVersion = serde_json::from_str(&data).unwrap();
    panic!("{}, {:?}", data, back);
}*/