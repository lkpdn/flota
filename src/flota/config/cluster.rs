use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use toml;
use ::util::errors::*;
use ::util::ipv4::IPv4;
use ::util::url::Url;

use super::Exec;
use super::template::Template;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct HostInterface {
    /// Network interface dev name on guest side.
    pub dev: String,
    /// For the time being only ipv4 supported.
    pub ip: IPv4,
}

impl HostInterface {
    pub fn from_toml(tml: &toml::Value) -> Result<HostInterface> {
        let dev = unfold!(tml, "dev", String);
        let ip = unfold!(tml, "ip", IPv4);
        Ok(HostInterface {
            dev: dev,
            ip: ip,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Host {
    /// Hostname.
    pub hostname: String,
    /// Network interfaces.
    pub interfaces: Vec<HostInterface>,
    /// Additional setups before standalone tests.
    pub solo_pre_tests: Vec<Exec>,
    /// Standalone tests.
    pub solo_tests: Vec<Exec>,
    /// Additional execs after standalone tests.
    pub solo_post_tests: Vec<Exec>,
    /// If true, poweroff after the cluster it belongs
    /// to has finished all tasks.
    /// DEFAULT: true
    pub destroy_when_finished: bool,
    /// If false, completely erase this host after the
    /// cluster it belongs to has finidhed all tasks.
    /// DEFAULT: true
    pub persistent: bool,
    /// Arc for corresponding template
    pub template: Arc<Template>,
}

impl Host {
    pub fn from_toml(tml: &toml::Value, templates: &HashSet<Arc<Template>>) -> Result<Host> {
        let hostname = unfold!(tml, "hostname", String);
        // XXX: we have a reference to wholly cloned template Arc Vec
        //      until all but one survivor chosen will drop when returning from this func.
        let template = templates.iter()
            .find(|&i| i.name == unfold!(tml, "template", String))
            .unwrap();
        let interfaces = match tml.lookup("interface") {
            Some(&toml::Value::Array(ref tml_ifs)) => {
                let mut ifs = Vec::new();
                for tml_if in tml_ifs {
                    ifs.push(HostInterface::from_toml(&tml_if).unwrap());
                }
                ifs
            }
            _ => {
                panic!("interface definition not found for host: {}", hostname);
            }
        };
        let solo_pre_tests = match tml.lookup("solo_pre_tests") {
            Some(&toml::Value::Array(ref tml_execs)) => {
                let mut execs = Vec::new();
                for tml_exec in tml_execs {
                    execs.push(Exec::from_toml(&tml_exec).unwrap());
                }
                execs
            }
            _ => vec![]
        };
        let solo_tests = match tml.lookup("solo_tests") {
            Some(&toml::Value::Array(ref tml_execs)) => {
                let mut execs = Vec::new();
                for tml_exec in tml_execs {
                    execs.push(Exec::from_toml(&tml_exec).unwrap());
                }
                execs
            }
            _ => vec![]
        };
        let solo_post_tests = match tml.lookup("solo_post_tests") {
            Some(&toml::Value::Array(ref tml_execs)) => {
                let mut execs = Vec::new();
                for tml_exec in tml_execs {
                    execs.push(Exec::from_toml(&tml_exec).unwrap());
                }
                execs
            }
            _ => vec![]
        };
        let destroy_when_finished = tml.lookup("destroy_when_finished")
            .map(|val| val.as_bool().unwrap())
            .unwrap_or(true);
        let persistent = unfold!(tml, "persistent", bool, optional, true);
        Ok(Host {
            hostname: hostname.to_owned(),
            interfaces: interfaces,
            solo_pre_tests: solo_pre_tests,
            solo_tests: solo_tests,
            solo_post_tests: solo_post_tests,
            destroy_when_finished: destroy_when_finished,
            persistent: persistent,
            template: template.clone(),
        })
    }
}

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
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
}
