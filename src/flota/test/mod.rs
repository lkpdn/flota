use serde_json;
use std::path::PathBuf;
use ::exec::ExecResult;
use ::flota::config::cluster::Cluster;
use ::flota::config::cluster::host::Host;
use ::flota::manager::watch::WatchPointPerception;
use ::flota::Storable;

// this indicated a cause to run tests
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, RustcEncodable, Hash)]
pub enum Cause {
    FirstRun,
    WatchPoint {
        ident: WatchPointPerception,
    }
}

pub trait TestResult {
    fn set_cause(&mut self, cause: &Cause) where Self: Sized;
    fn push_result(&mut self, result: ExecResult) where Self: Sized;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, RustcEncodable, Hash)]
pub struct HostTestResult {
    pub host: Host,
    pub cause: Option<Cause>,
    pub results: Vec<ExecResult>,
}

impl TestResult for HostTestResult {
    fn set_cause(&mut self, cause: &Cause) where Self: Sized {
        self.cause = Some(cause.clone());
    }
    fn push_result(&mut self, result: ExecResult) where Self: Sized {
        self.results.push(result);
    }
}

impl HostTestResult {
    pub fn init(host: &Host) -> Self {
        HostTestResult {
            host: host.clone(),
            results: vec![],
            cause: None,
        }
    }
    fn set_cause(&mut self, cause: &Cause) where Self: Sized {
        self.cause = Some(cause.clone());
    }
}

impl From<Vec<u8>> for HostTestResult {
    fn from(v: Vec<u8>) -> Self {
        let buf = String::from_utf8(v).unwrap();
        serde_json::from_str(&buf).unwrap()
    }
}

impl Storable for HostTestResult {
    fn db_path() -> PathBuf {
        ::consts::CONFIG_HISTORY_DIR.join("host_test_results")
    }
    fn key(&self) -> Vec<u8> {
        match self.cause {
            Some(Cause::WatchPoint { ref ident }) => {
                let mut key: Vec<u8> = "watchpoint:".into();
                key.append(&mut ident.watchpoint_id.clone());
                key
            },
            // in case no watchpoint is set
            _ => {
                format!("host:{}", self.host.id())
                    .as_bytes().to_vec()
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, RustcEncodable, Hash)]
pub struct ClusterTestResult {
    pub cluster: Cluster,
    pub cause: Option<Cause>,
    pub results: Vec<ExecResult>,
}

impl TestResult for ClusterTestResult {
    fn set_cause(&mut self, cause: &Cause) where Self: Sized {
        self.cause = Some(cause.clone());
    }
    fn push_result(&mut self, result: ExecResult) where Self: Sized {
        self.results.push(result);
    }
}

impl ClusterTestResult {
    pub fn init(cluster: &Cluster) -> Self {
        ClusterTestResult {
            cluster: cluster.clone(),
            cause: None,
            results: vec![],
        }
    }
}

impl From<Vec<u8>> for ClusterTestResult {
    fn from(v: Vec<u8>) -> Self {
        let buf = String::from_utf8(v).unwrap();
        serde_json::from_str(&buf).unwrap()
    }
}

impl Storable for ClusterTestResult {
    fn db_path() -> PathBuf {
        ::consts::CONFIG_HISTORY_DIR.join("cluster_test_results")
    }
    fn key(&self) -> Vec<u8> {
        match self.cause {
            Some(Cause::WatchPoint { ref ident }) => {
                let mut key: Vec<u8> = "watchpoint:".into();
                key.append(&mut ident.watchpoint_id.clone());
                key
            },
            // in case no watchpoint is set
            _ => {
                format!("cluster:{}", self.cluster.id())
                    .as_bytes().to_vec()
            },
        }
    }
}
