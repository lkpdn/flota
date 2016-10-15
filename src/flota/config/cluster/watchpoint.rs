use serde_json;
use std::mem;
use std::path::PathBuf;
use toml;

use ::flota::{hash, Storable};
use ::util::errors::*;
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
        ::consts::DATA_DIR.join("watchpoint")
    }
    fn key(&self) -> Vec<u8> {
        unsafe { mem::transmute::<u64, [u8; 8]>(hash(self)).to_vec() }
    }
}

impl WatchPoint {
    pub fn from_toml(tml: &toml::Value) -> Result<Self> {
        let ty = unfold!(tml, "type", String);
        // WatchPoint::Git
        if ty == "git" {
            if let Some(&toml::Value::Array(ref refs)) = tml.lookup("refs") {
                Ok(WatchPoint::Git {
                    uri: unfold!(tml, "uri", Url),
                    remote: unfold!(tml, "remote", String, optional,
                                    "origin".to_string()),
                    refs: refs.iter().map(|s| s.as_str().unwrap().to_owned())
                              .collect::<Vec<_>>(),
                    checkout_dir: unfold!(tml, "checkout_dir", PathBuf),
                })
            } else {
                Err("watchpoint type `git` requires `refs` array".into())
            }
        // WatchPoint::File
        } else if ty == "file" {
            Ok(WatchPoint::File {
                path: unfold!(tml, "path", PathBuf),
            })
        } else {
            Err(format!("unsupported watchpoint type: {}", ty).into())
        }
    }
}
