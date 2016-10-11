use std::path::Path;
use ::flota::manager::{ClusterTestKey, TestResult};
use ::util::errors::*;

pub mod unqlite_backed;

pub trait TestResultStore {
    fn new(path: &Path) -> Self;
    fn find(&self, key: &ClusterTestKey) -> Option<TestResult>;
    fn set(&self, key: &ClusterTestKey, result: &TestResult) -> Result<()>;
}
