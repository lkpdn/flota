use ::util::errors::*;

pub mod ssh;
pub mod console;
pub mod local;

pub struct Return {
    pub stdout: String,
    pub stderr: String,
    pub status: i32,
}

pub trait Session {
    fn exec(&self, command: String) -> Result<Return>;
}
