use serde_json::value::ToJson;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use toml;
use ::flota::hash;
use ::util::errors::*;
use ::util::url::Url;

use super::Exec;
use super::template::Template;

pub mod host;
use self::host::Host;

pub mod watchpoint;
use self::watchpoint::WatchPoint;

#[derive(Debug, Clone, Serialize, Deserialize, RustcEncodable, PartialEq, Eq, Hash)]
pub struct Cluster {
    /// Cluster name arbitrarily chosen.
    pub name: String,
    /// Watchpoints. Empty array is okay, in that case only this
    /// cluster's config change triggers test reruns.
    pub watchpoints: Vec<WatchPoint>,
    /// Hosts which belong to this cluster. Note that
    /// these are set up in the same order.
    pub hosts: Vec<Host>,
    /// Additional execs after all the hosts have been
    /// provisioned in the standalone tasks. Note that
    /// these execs will be executed in the presice order.
    pub pre_tests: Vec<Exec>,
    /// Cluster tests sequence.
    pub tests: Vec<Exec>,
    /// Additional execs after the cluster tests.
    pub post_tests: Vec<Exec>,
    /// If true, poweroff all the hosts except ones
    /// spcified otherwise in host granularity.
    pub destroy_when_finished: bool,
    /// If false, completely erase all the hosts except
    /// ones specified otherwise in host granularity.
    pub persistent: bool,
}

impl Cluster {
    pub fn from_toml(tml: &toml::Value, templates: &HashSet<Arc<Template>>) -> Result<Cluster> {
        let name = tml.lookup("name").map(|val| val.as_str().unwrap()).unwrap();
        let watchpoints = match tml.lookup("watchpoint") {
            Some(&toml::Value::Array(ref tml_watchpoints)) => {
                let mut watchpoints = Vec::new();
                for tml_watchpoint in tml_watchpoints {
                    let ty = unfold!(tml_watchpoint, "type", String);
                    // WatchPoint::Git
                    if ty == "git" {
                        if let Some(&toml::Value::Array(ref refs)) = tml_watchpoint.lookup("refs") {
                            watchpoints.push(WatchPoint::Git {
                                uri: unfold!(tml_watchpoint, "uri", Url),
                                remote: unfold!(tml_watchpoint, "remote", String, optional, "origin".to_string()),
                                refs: refs.iter().map(|s| s.as_str().unwrap().to_owned())
                                    .collect::<Vec<_>>(),
                                checkout_dir: unfold!(tml_watchpoint, "checkout_dir", PathBuf),
                            });
                        } else {
                            return Err("watchpoint type git requires branches array".into())
                        }
                    // WatchPoint::File
                    } else if ty == "file" {
                        watchpoints.push(WatchPoint::File {
                            path: unfold!(tml_watchpoint, "path", PathBuf),
                        });
                    } else {
                        return Err(format!("unsupported watchpoint type: {}", ty).into())
                    }
                }
                watchpoints
            },
            _ => { vec![] }
        };
        let hosts = match tml.lookup("host") {
            Some(&toml::Value::Array(ref tml_hsts)) => {
                let mut hsts = Vec::new();
                for tml_hst in tml_hsts {
                    hsts.push(Host::from_toml(&tml_hst, templates).unwrap());
                }
                hsts
            },
            _ => {
                warn!("no hosts found in cluster: {}", name);
                return Err(format!("no hosts found in cluster: {}", name).as_str().into());
            }
        };
        let pre_tests = match tml.lookup("pre_tests") {
            Some(&toml::Value::Array(ref tml_execs)) => {
                let mut execs = Vec::new();
                for tml_exec in tml_execs {
                    execs.push(Exec::from_toml(&tml_exec).unwrap());
                }
                execs
            }
            _ => vec![],
        };
        let tests = match tml.lookup("tests") {
            Some(&toml::Value::Array(ref tml_execs)) => {
                let mut execs = Vec::new();
                for tml_exec in tml_execs {
                    execs.push(Exec::from_toml(&tml_exec).unwrap());
                }
                execs
            }
            _ => vec![],
        };
        let post_tests = match tml.lookup("post_tests") {
            Some(&toml::Value::Array(ref tml_execs)) => {
                let mut execs = Vec::new();
                for tml_exec in tml_execs {
                    execs.push(Exec::from_toml(&tml_exec).unwrap());
                }
                execs
            }
            _ => vec![],
        };
        let destroy_when_finished = tml.lookup("destroy_when_finished")
            .map(|val| val.as_bool().unwrap())
            .unwrap_or(true);
        let persistent = tml.lookup("persistent")
            .map(|val| val.as_bool().unwrap())
            .unwrap_or(true);
        Ok(Cluster {
            name: name.to_owned(),
            watchpoints: watchpoints,
            hosts: hosts,
            pre_tests: pre_tests,
            tests: tests,
            post_tests: post_tests,
            destroy_when_finished: destroy_when_finished,
            persistent: persistent,
        })
    }
    pub fn id(&self) -> u64 {
        hash(self)
    }
}
