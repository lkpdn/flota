use std::path::Path;
use super::Config;
use ::util::errors::*;

pub mod unqlite_backed;

pub trait ConfigStore {
    fn new(path: &Path) -> Self;
    fn update(&self, config: &Config) -> Result<bool>;
    fn current(&self) -> Option<Config>;
}
