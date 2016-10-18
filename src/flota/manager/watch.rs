use git2::{Direction, ErrorCode, Repository};
use std::path::Path;
use ::flota::Cypherable;
use ::flota::config::cluster::watchpoint::WatchPoint;
use ::util::md5sum::calc_md5;
use ::util::url::Url;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum WatchPointPerceptionValue {
    Git {
        ref_commit_ids: Vec<(String, Vec<u8>)>,
    },
    File {
        checksum: Vec<u8>,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct WatchPointPerception {
    pub value: WatchPointPerceptionValue,
}

impl Cypherable for WatchPointPerception {
    fn cypher_ident(&self) -> String {
        format!("WatchPointPerception {{ value: '{:?}' }}",
                self.value)
    }
}

impl WatchPointPerception {
    pub fn new(watchpoint: &WatchPoint) -> Self {
        let perception = Self::perceive(watchpoint);
        WatchPointPerception {
            value: perception,
        }
    }
    fn perceive_git(uri: &Url, remote: &str, refs: &Vec<String>,
                    checkout_dir: &Path) -> WatchPointPerceptionValue {
        let url = uri.as_str();
        let repo = match Repository::clone(url, checkout_dir) {
            Ok(repo) => { repo },
            Err(ref e) if e.code() == ErrorCode::Exists => {
                // XXX: re-clone if it's broken
                Repository::open(checkout_dir).expect(
                    format!("failed to open {:?}", checkout_dir).as_str())
            },
            Err(e) => panic!("failed to clone: {}", e),
        };
        let mut rem = repo.find_remote(remote).unwrap();
        rem.connect(Direction::Fetch).expect(format!(
                "failed to connect to {}", remote).as_str());
        let ref_commit_ids = rem
            .list()
            .unwrap()
            .iter()
            .map(|head| (head.name().to_owned(), head.oid().as_bytes().to_vec()))
            .filter(|r1| {
                if &refs[..] == &[ "*" ] {
                    true
                } else {
                    refs.iter().find(|r2| **r2 == r1.0).is_some()
                }
            })
            .collect::<Vec<_>>();
        WatchPointPerceptionValue::Git {
            ref_commit_ids: ref_commit_ids,
        }
    }
    fn perceive_file(path: &Path) -> WatchPointPerceptionValue {
        WatchPointPerceptionValue::File {
            checksum: calc_md5(path).unwrap().as_bytes().to_vec(),
        }
    }
    pub fn perceive(watchpoint: &WatchPoint) -> WatchPointPerceptionValue {
        match *watchpoint {
            WatchPoint::Git {
                ref uri,
                ref remote,
                ref refs,
                ref checkout_dir,
            } => {
                Self::perceive_git(uri, remote, refs, checkout_dir)
            },
            WatchPoint::File {
                ref path,
            } => {
                Self::perceive_file(path)
            }
        }
    }
}
