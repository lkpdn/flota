use std::any::Any;
use std::fmt;
use ::flota::config;
use ::util::errors::*;

pub mod ssh;
pub mod console;
pub mod local;

pub trait Session {
    fn exec(&self, command: &str) -> Result<Return>;
}

pub trait SessionSeed : SessionSeedBoxer + fmt::Debug {
    fn spawn(&self) -> Result<Box<Session>>;
    fn seed_type(&self) -> SeedType;
    fn as_mut_any(&mut self) -> &mut Any;
}

// XXX: type uniqueness not guaranteed
pub type SessionSeeds = Vec<Box<SessionSeed>>;

impl Clone for Box<SessionSeed> {
    fn clone(&self) -> Box<SessionSeed> {
        self.box_seed()
    }
}

pub trait SessionSeedBoxer {
    fn box_seed(&self) -> Box<SessionSeed>;
}

impl<T> SessionSeedBoxer for T where T: 'static + SessionSeed + Clone {
    fn box_seed(&self) -> Box<SessionSeed> {
        Box::new(self.clone())
    }
}

pub fn try_spawn(seeds: SessionSeeds, prio: Vec<SeedType>) -> Result<Box<Session>> {
    for cand in prio.iter() {
        if let Some(seed) = seeds.iter().find(|s| s.seed_type() == *cand) {
            match seed.spawn() {
                Ok(session) => { return Ok(session) },
                Err(e) => { error!("{}", e) }
            }
        }
    }
    Err("failed to spawn session".into())
}

#[derive(PartialEq)]
pub enum SeedType {
    Ssh,
    Console,
    Local,
}

impl SeedType {
    pub fn from_exec_type(exec_type: &config::ExecType) -> Option<SeedType> {
        match *exec_type {
            config::ExecType::Console => { Some(SeedType::Console) },
            config::ExecType::Ssh{..} => { Some(SeedType::Ssh) },
            config::ExecType::Local => { Some(SeedType::Local) },
        }
    }
}

#[derive(Debug)]
pub struct Return {
    pub stdout: String,
    pub stderr: String,
    pub status: i32,
}
