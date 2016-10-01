use ::util::errors::*;
use super::{Return, Session};

pub struct SessLocal {}

impl Session for SessLocal {
    #[allow(unused_variables)]
    fn exec(&self, command: String) -> Result<Return> {
        unimplemented!()
    }
}
