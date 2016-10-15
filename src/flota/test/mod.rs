use serde_json;
use std::path::PathBuf;
use ::exec::ExecResult;
use ::flota::config::cluster::Cluster;
use ::flota::config::cluster::host::Host;
use ::flota::manager::watch::WatchPointPerception;
use ::flota::HistoryStorable;

// this indicated a cause to run tests
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, RustcEncodable, Hash)]
pub enum Cause {
    FirstRun,
    WatchPoint {
        ident: WatchPointPerception,
    }
}

pub trait TestResult {
    fn push_cause(&mut self, cause: &Cause) where Self: Sized;
    fn push_result(&mut self, result: ExecResult) where Self: Sized;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, RustcEncodable, Hash)]
pub struct HostTestResult {
    pub host: Host,
    pub causes: Vec<Cause>,
    pub results: Vec<ExecResult>,
}

impl TestResult for HostTestResult {
    fn push_cause(&mut self, cause: &Cause) where Self: Sized {
        self.causes.push(cause.clone());
    }
    fn push_result(&mut self, result: ExecResult) where Self: Sized {
        self.results.push(result);
    }
}

impl HostTestResult {
    pub fn init(host: &Host) -> Self {
        HostTestResult {
            host: host.clone(),
            causes: vec![],
            results: vec![],
        }
    }
}

impl From<Vec<u8>> for HostTestResult {
    fn from(v: Vec<u8>) -> Self {
        let buf = String::from_utf8(v).unwrap();
        serde_json::from_str(&buf).unwrap()
    }
}

impl HistoryStorable for HostTestResult {
    fn db_path() -> PathBuf {
        ::consts::DATA_DIR.join("host_test_results")
    }
    fn key(&self) -> Vec<u8> {
        format!("host:{}", self.host.id())
            .as_bytes().to_vec()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, RustcEncodable, Hash)]
pub struct ClusterTestResult {
    pub cluster: Cluster,
    pub causes: Vec<Cause>,
    pub results: Vec<ExecResult>,
}

impl TestResult for ClusterTestResult {
    fn push_cause(&mut self, cause: &Cause) where Self: Sized {
        self.causes.push(cause.clone());
    }
    fn push_result(&mut self, result: ExecResult) where Self: Sized {
        self.results.push(result);
    }
}

impl ClusterTestResult {
    pub fn init(cluster: &Cluster) -> Self {
        ClusterTestResult {
            cluster: cluster.clone(),
            causes: vec![],
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

impl HistoryStorable for ClusterTestResult {
    fn db_path() -> PathBuf {
        ::consts::DATA_DIR.join("cluster_test_results")
    }
    fn key(&self) -> Vec<u8> {
        format!("cluster:{}", self.cluster.id())
            .as_bytes().to_vec()
    }
}
