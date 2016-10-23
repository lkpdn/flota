use ::flota::manager::watch::WatchPointPerception;

// this indicated a cause to run tests
#[derive(Debug)]
pub enum Cause {
    FirstRun,
    WatchPoint {
        ident: WatchPointPerception,
    }
}
