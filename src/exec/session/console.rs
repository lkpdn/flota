use ::util::errors::*;
use super::{Return, Session};

pub struct SessConsole {}

impl Session for SessConsole {
    #[allow(unused_variables)]
    fn exec(&self, command: String) -> Result<Return> {
        unimplemented!()
    }
}
