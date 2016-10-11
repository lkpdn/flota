use serde_json;
use std::path::Path;
use unqlite::{UnQLite, KV};
use ::flota::manager::{ClusterTestKey, TestResult};
use ::store::TestResultStore as TestResultStoreTrait;
use ::util::errors::*;

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
