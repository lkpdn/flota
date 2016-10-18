use ::flota::manager::watch::WatchPointPerception;

// this indicated a cause to run tests
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Cause {
    FirstRun,
    WatchPoint {
        ident: WatchPointPerception,
    }
}
