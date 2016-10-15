use serde_json::value::{from_value, ToJson, Value};
use serde::de::Deserialize;
use serde::ser::Serialize;
use std::hash::{Hash, Hasher, SipHasher};
use std::path::PathBuf;
use std::str::FromStr;
use unqlite::{Cursor, KV, UnQLite};
use ::util::errors::*;

pub mod config;
pub mod entity;
pub mod manager;
pub mod test;

pub trait Storable : From<Vec<u8>> + ToJson + PartialEq {
    fn db_path() -> PathBuf;
    fn unqlite() -> UnQLite {
        UnQLite::create(Self::db_path().as_path().to_str().unwrap())
    }
    fn key(&self) -> Vec<u8>;
    fn save(&self) -> Result<()> {
        debug!("[{db}] now saving...\n\
                [{db}] - key: {key:?}\n\
                [{db}] - val: {val}",
               db = Self::db_path().display(),
               key = self.key(),
               val = format!("{}", self.to_json()));
        Self::unqlite().kv_store(
            self.key(),
            format!("{}", self.to_json())
        ).map_err(|e| format!("{}", e).into())
    }
    fn is_last_saved(&self) -> Result<bool> where Self: Sized {
        if let Some(last) = Self::last_saved() {
            if last.eq(self) {
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            Err("no saved record".into())
        }
    }
    fn last_saved() -> Option<Self> where Self: Sized {
        match Self::unqlite().last() {
            Some(last) => {
                Some(last.value().into())
            },
            None => None,
        }
    }
    fn find(key: Vec<u8>) -> Option<Self> where Self: Sized {
        Self::unqlite().kv_fetch(key).ok().map(|res| res.into())
    }
    fn get_all() -> Option<Vec<Self>> where Self: Sized {
        unimplemented!()
    }
}

// XXX: not scalable approach. I Will Say Goodbye to kv store.
pub trait HistoryStorable : Clone + From<Vec<u8>> + ToJson +
                            PartialEq + Deserialize + Serialize {
    fn db_path() -> PathBuf;
    fn unqlite() -> UnQLite {
        UnQLite::create(Self::db_path().as_path().to_str().unwrap())
    }
    fn key(&self) -> Vec<u8>;
    fn update(&self) -> Result<bool> {
        if let Some(mut record) = Self::find(self.key()) {
            let last = try!(record.last().ok_or("broken record found")).clone();
            if self.eq(&last) {
                info!("no change since last pinning");
                Ok(false)
            } else {
                let self_clone = self.clone();
                record.push(self_clone);
                Self::unqlite().kv_store(
                    self.key(),
                    format!("{}", record.to_json())
                )
                .map(|()| true)
                .map_err(|e| format!("{}", e).into())
            }
        } else {
            Self::unqlite().kv_store(
                self.key(),
                format!("{}", vec![&self].to_json())
            )
            .map(|()| true)
            .map_err(|e| format!("{}", e).into())
        }
    }
    fn find(key: Vec<u8>) -> Option<Vec<Self>> where Self: Sized {
        if let Ok(raw) = Self::unqlite().kv_fetch(key) {
            let buf = String::from_utf8(raw).unwrap();
            if let Ok(val) = Value::from_str(&buf) {
                match val.as_array() {
                    Some(vec) => {
                        Some(
                            vec.iter().map(|elem| {
                                from_value(elem.clone()).unwrap()
                            }).collect::<Vec<_>>()
                        )
                    },
                    // seems to be broken so ignore it.
                    // likely to be overwritten on some test later.
                    None => None,
                }
            } else { None }
        } else { None }
    }
    fn get_all() -> Option<Vec<Vec<Self>>> where Self: Sized {
        unimplemented!()
    }
}

pub fn hash<T: Hash>(t: &T) -> u64 {
    let mut s = SipHasher::new();
    t.hash(&mut s);
    s.finish()
}
