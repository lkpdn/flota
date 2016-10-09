use serde_json;
use std::path::Path;
use time;
use unqlite::{UnQLite, KV, Cursor};
use ::flota::config::Config;
use ::flota::manager::{ClusterTestKey, TestResult};
use ::store::ConfigStore as ConfigStoreTrait;
use ::store::TestResultStore as TestResultStoreTrait;
use ::util::errors::*;

pub struct ConfigStore {
    inner: UnQLite,
}

impl ConfigStoreTrait for ConfigStore {
    fn new(path: &Path) -> Self {
        let unqlite = UnQLite::create(path.to_str().unwrap());
        ConfigStore {
            inner: unqlite,
        }
    }
    fn update(&self, config: &Config) -> Result<bool> {
        if let Some(last) = self.current() {
            if last.eq(config) {
                return Ok(false)
            }
        }
        let now = time::now();
        let serialized = serde_json::to_string(&config).unwrap();
        // key: timestamp like "2012-02-22T14:53:18Z" (RFC3339)
        // value: json representation of the whole configuration
        self.inner.kv_store(format!("{}", &now.rfc3339()), serialized)
                  .map(|_| true)
                  .map_err(|e| format!("{}", e).into())
    }
    fn current(&self) -> Option<Config> {
        match self.inner.last() {
            Some(last) => {
                Some(last.value().into())
            },
            None => None
        }
    }
}

pub struct TestResultStore {
    inner: UnQLite,
}

impl TestResultStoreTrait for TestResultStore {
    fn new(path: &Path) -> Self {
        let unqlite = UnQLite::create(path.to_str().unwrap());
        TestResultStore {
            inner: unqlite,
        }
    }
    fn find(&self, key: &ClusterTestKey) -> Option<TestResult> {
        let ser_key = serde_json::to_string(&key).unwrap();
        self.inner.kv_fetch(ser_key).ok().map(|res| res.into())
    }
    fn set(&self, key: &ClusterTestKey, result: &TestResult) -> Result<()> {
        let ser_key = serde_json::to_string(&key).unwrap();
        let ser_result = serde_json::to_string(&result).unwrap();
        self.inner.kv_store(ser_key, ser_result)
                  .map(|_| ())
                  .map_err(|e| format!("{}", e).into())
    }
}
