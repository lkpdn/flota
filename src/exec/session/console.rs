use std::any::Any;
use ::exec::Output;
use ::exec::session::{SeedType, Session, SessionSeed};
use ::util::errors::*;

#[derive(Debug, Clone)]
pub struct SessConsole {}
#[derive(Debug, Clone)]
pub struct SessSeedConsole {}

impl SessionSeed for SessSeedConsole {
    fn spawn(&self) -> Result<Box<Session>> {
        unimplemented!()
    }
    fn seed_type(&self) -> SeedType {
        SeedType::Console
    }
    fn as_mut_any(&mut self) -> &mut Any {
        self
    }
}

impl Session for SessConsole {
    #[allow(unused_variables)]
    fn exec(&self, command: &str) -> Result<Output> {
        unimplemented!()
    }
}
