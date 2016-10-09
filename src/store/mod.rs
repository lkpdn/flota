use std::path::Path;
use ::flota::config::Config;
use ::flota::manager::{ClusterTestKey, TestResult};
use ::util::errors::*;

pub mod unqlite_backed;

pub trait ConfigStore {
    fn new(path: &Path) -> Self;
    fn update(&self, config: &Config) -> Result<bool>;
    fn current(&self) -> Option<Config>;
}

pub trait TestResultStore {
    fn new(path: &Path) -> Self;
    fn find(&self, key: &ClusterTestKey) -> Option<TestResult>;
    fn set(&self, key: &ClusterTestKey, result: &TestResult) -> Result<()>;
}
