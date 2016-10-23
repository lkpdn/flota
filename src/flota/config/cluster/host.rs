use rusted_cypher::graph::GraphClient;
use std::collections::HashSet;
use std::sync::Arc;
use toml;
use ::flota::{hash, Cypherable};
use ::flota::config::Exec;
use ::flota::config::template::Template;
use ::util::errors::*;
use ::util::ipv4::IPv4;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

impl Cypherable for Host {
    fn cypher_ident(&self) -> String {
        format!("Host {{ hostname: '{hostname}',
                         interfaces: '{interfaces:?}',
                         destroy_when_finished: '{destroy_when_finished}',
                         persistent: '{persistent}' }}",
               hostname = self.hostname,
               interfaces = self.interfaces,
               destroy_when_finished = self.destroy_when_finished,
               persistent = self.persistent)
    }
}

impl Host {
    fn save(&self) -> Result<()> {
        // prepare and start transaction
        let graph = GraphClient::connect(::NEO4J_ENDPOINT).unwrap();
        let mut transaction = graph.cypher().transaction();
        transaction.add_statement("MATCH (n: TRANSACTION) RETURN n");
        let (mut transaction, _) = transaction.begin().unwrap();

        // save template
        if let Err(e) = save_child_rel!(&mut transaction, self, self.template, "BACKED_BY") {
            error!("{}", e);
            try!(transaction.rollback());
            return Err("failed to save Host".into());
        }

        // save tests
        for ref solo_pre_test in self.solo_pre_tests.iter() {
            if let Err(e) = save_child_rel!(&mut transaction, self, solo_pre_test, "EXEC") {
                error!("{}", e);
                try!(transaction.rollback());
                return Err("failed to save Host".into());
            }
        }
        for ref solo_test in self.solo_tests.iter() {
            if let Err(e) = save_child_rel!(&mut transaction, self, solo_test, "EXEC") {
                error!("{}", e);
                try!(transaction.rollback());
                return Err("failed to save Host".into());
            }
        }
        for ref solo_post_test in self.solo_post_tests.iter() {
            if let Err(e) = save_child_rel!(&mut transaction, self, solo_post_test, "EXEC") {
                error!("{}", e);
                try!(transaction.rollback());
                return Err("failed to save Host".into());
            }
        }

        // commit transaction
        transaction.commit().map(|_| ()).map_err(|e| e.into())
    }

    fn from_toml_inner(tml: &toml::Value, templates: &HashSet<Arc<Template>>) -> Result<Host> {
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
    pub fn from_toml(tml: &toml::Value, templates: &HashSet<Arc<Template>>) -> Result<Host> {
        match Self::from_toml_inner(tml, templates) {
            Ok(host) => {
                host.save().unwrap();
                Ok(host)
            },
            Err(e) => Err(e),
        }
    }
    pub fn id(&self) -> u64 {
        hash(self)
    }
}
