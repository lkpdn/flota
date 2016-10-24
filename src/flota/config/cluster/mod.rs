use rusted_cypher::graph::GraphClient;
use std::collections::HashSet;
use std::sync::Arc;
use toml;
use ::flota::{hash, Cypherable};
use ::util::errors::*;

use super::Exec;
use super::template::Template;

pub mod host;
use self::host::Host;

pub mod watchpoint;
use self::watchpoint::WatchPoint;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

impl Cypherable for Cluster {
    fn cypher_ident(&self) -> String {
        format!("Cluster {{ name: '{}' }}", self.name)
    }
}

impl Cluster {
    pub fn save(&self) -> Result<()> {
        // prepare and start transaction
        let graph = GraphClient::connect(::NEO4J_ENDPOINT).unwrap();
        let mut transaction = graph.cypher().transaction();
        transaction.add_statement("MATCH (n: TRANSACTION) RETURN n");
        let (mut transaction, _) = transaction.begin().unwrap();

        // save watchpoint
        for ref watchpoint in self.watchpoints.iter() {
            if let Err(e) = save_child_rel!(&mut transaction, self, watchpoint, "WATCH") {
                error!("{}", e);
                try!(transaction.rollback());
                return Err("failed to save Cluster".into());
            }
        }

        // save tests
        for ref pre_test in self.pre_tests.iter() {
            if let Err(e) = save_child_rel!(&mut transaction, self, pre_test, "EXEC") {
                error!("{}", e);
                try!(transaction.rollback());
                return Err("failed to save Cluster".into());
            }
        }
        for ref test in self.tests.iter() {
            if let Err(e) = save_child_rel!(&mut transaction, self, test, "EXEC") {
                error!("{}", e);
                try!(transaction.rollback());
                return Err("failed to save Cluster".into());
            }
        }
        for ref post_test in self.post_tests.iter() {
            if let Err(e) = save_child_rel!(&mut transaction, self, post_test, "EXEC") {
                error!("{}", e);
                try!(transaction.rollback());
                return Err("failed to save Cluster".into());
            }
        }

        // save hosts
        for ref host in self.hosts.iter() {
            if let Err(e) = save_child_rel!(&mut transaction, self, host, "DEFINE") {
                error!("{}", e);
                try!(transaction.rollback());
                return Err("failed to save Cluster".into());
            }
        }

        // commit transaction
        transaction.commit().map(|_| ()).map_err(|e| e.into())
    }
    pub fn is_first_run(&self) -> Result<bool> {
        // XXX: if first and last cluster run was aborted for some reason,
        // it will mistakenly conclude that it was ok and skip FirstRun until
        // some watchpoint triggers it. some mechanism to notice cluster
        // state has to be introduced.
        let graph = GraphClient::connect(::NEO4J_ENDPOINT).unwrap();
        graph.cypher().exec(
            format!("MATCH (self: {})-[:EXEC]->(t: Exec)<-[:IS_RESULT_OF]-(res: ExecResult)
                     RETURN res", self.cypher_ident()).as_ref()
        ).map(|r| r.rows().count() == 0).map_err(|e| e.into())
    }
    fn from_toml_inner(tml: &toml::Value, templates: &HashSet<Arc<Template>>) -> Result<Cluster> {
        let name = tml.lookup("name").map(|val| val.as_str().unwrap()).unwrap();
        let watchpoints = match tml.lookup("watchpoint") {
            Some(&toml::Value::Array(ref tml_watchpoints)) => {
                let mut watchpoints = Vec::new();
                for tml_watchpoint in tml_watchpoints {
                    watchpoints.push(
                        WatchPoint::from_toml(tml_watchpoint).unwrap()
                    )
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
    pub fn from_toml(tml: &toml::Value, templates: &HashSet<Arc<Template>>) -> Result<Cluster> {
        match Self::from_toml_inner(tml, templates) {
            Ok(cluster) => {
                cluster.save().unwrap();
                Ok(cluster)
            },
            Err(e) => Err(e),
        }
    }
    pub fn id(&self) -> u64 {
        hash(self)
    }
}
