use serde_json::value::ToJson;
use std::hash::{Hash, Hasher, SipHasher};
use std::path::PathBuf;
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

pub fn hash<T: Hash>(t: &T) -> u64 {
    let mut s = SipHasher::new();
    t.hash(&mut s);
    s.finish()
}
