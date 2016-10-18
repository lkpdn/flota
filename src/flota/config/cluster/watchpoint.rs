use std::path::PathBuf;
use toml;

use ::flota::Cypherable;
use ::util::errors::*;
use ::util::url::Url;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
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

impl Cypherable for WatchPoint {
    fn cypher_ident(&self) -> String {
        match *self {
            WatchPoint::Git { ref uri, ref remote, ref refs, ref checkout_dir } => {
                format!("WatchPoint {{ type: 'Git',
                                       uri: '{}',
                                       remote: '{}',
                                       refs: '{}',
                                       checkout_dir: '{}' }}",
                        uri.as_str(),
                        remote.as_str(),
                        refs.join(", "),
                        checkout_dir.to_str().unwrap())
            },
            WatchPoint::File { ref path } => {
                format!("WatchPoint {{ type: 'File',
                                       path: '{}' }}",
                        path.to_str().unwrap())
            }
        }
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
