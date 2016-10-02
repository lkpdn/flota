use std::any::Any;
use ::util::errors::*;
use super::{Return, SeedType, Session, SessionSeed};

#[derive(Debug, Clone)]
pub struct SessLocal {}
#[derive(Debug, Clone)]
pub struct SessSeedLocal {}

impl SessionSeed for SessSeedLocal {
    fn spawn(&self) -> Result<Box<Session>> {
        unimplemented!()
    }
    fn seed_type(&self) -> SeedType {
        SeedType::Local
    }
    fn as_mut_any(&mut self) -> &mut Any {
        self
    }
}

impl Session for SessLocal {
    #[allow(unused_variables)]
    fn exec(&self, command: &str) -> Result<Return> {
        unimplemented!()
    }
}
