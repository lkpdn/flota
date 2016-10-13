use serde_json;
use std::mem;
use std::path::PathBuf;

use ::flota::{hash, Storable};
use ::util::url::Url;

#[derive(Debug, Clone, Serialize, Deserialize, RustcEncodable, PartialEq, Eq, Hash)]
pub enum WatchPoint {
    Git {
        uri: Url,
        remote: String,
        refs: Vec<String>,
        checkout_dir: PathBuf,
    },
    File {
        path: PathBuf,
    }
}

impl From<Vec<u8>> for WatchPoint {
    fn from(v: Vec<u8>) -> Self {
        let buf = String::from_utf8(v).unwrap();
        serde_json::from_str(&buf).unwrap()
    }
}

impl Storable for WatchPoint {
    fn db_path() -> PathBuf {
        ::consts::CONFIG_HISTORY_DIR.join("watchpoint")
    }
    fn key(&self) -> Vec<u8> {
        unsafe { mem::transmute::<u64, [u8; 8]>(hash(self)).to_vec() }
    }
}
