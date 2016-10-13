use serde_json::value::ToJson;
use std::collections::HashSet;
use std::sync::Arc;
use toml;
use ::flota::hash;
use ::flota::config::Exec;
use ::flota::config::template::Template;
use ::util::errors::*;
use ::util::ipv4::IPv4;

#[derive(Debug, Clone, Serialize, Deserialize, RustcEncodable, PartialEq, Eq, Hash)]
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

#[derive(Debug, Clone, Serialize, Deserialize, RustcEncodable, PartialEq, Eq, Hash)]
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
    pub fn id(&self) -> u64 {
        hash(self)
    }
}
