use serde_json;
use std::collections::HashSet;
use std::convert::AsRef;
use std::fs::File;
use std::io::prelude::*;
use std::mem;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use time;
use toml;
use ::util::errors::*;
use ::flota::Storable;

macro_rules! unfold {
    ( $toml:ident, $key:expr, $ty:tt, optional, $default:expr ) => {{
        match stringify!($ty) {
            "bool"|"IPv4"|"i32"|"PathBuf"|"String"|"Url" => {
                unfold!($toml, $key, $ty, optional)
                    .or(Some($default)).unwrap()
            },
            _ => panic!("unsupported type")
        }
    }};
    ( $toml:ident, $key:expr, $ty:tt ) => {{
        match stringify!($ty) {
            "bool"| "IPv4"|"i32"|"PathBuf"|"String"|"Url" => {
                unfold!($toml, $key, $ty, optional)
                     .ok_or(format!("`{}` must be specified", $key).as_str()).unwrap()
            },
            _ => panic!("unsupported type")
        }
    }};
    ( $toml:ident, $key:expr, String, optional ) => {{
        $toml.lookup($key)
             .map(|val| val.as_str().unwrap().to_string())
    }};
    ( $toml:ident, $key:expr, IPv4, optional ) => {{
        $toml.lookup($key)
             .map(|val| {
                 IPv4::from_cidr_notation(val.as_str().unwrap())
                     .unwrap()
             })
    }};
    ( $toml:ident, $key:expr, i32, optional ) => {{
        $toml.lookup($key)
             .map(|val| val.as_integer().unwrap() as i32)
    }};
    ( $toml:ident, $key:expr, PathBuf, optional ) => {{
        $toml.lookup($key)
             .map(|val| PathBuf::from(val.as_str().unwrap()))
    }};
    ( $toml:ident, $key:expr, Url, optional ) => {{
        $toml.lookup($key)
             .map(|val| {
                 Url::parse(val.as_str().unwrap()).unwrap()
             })
    }};
    ( $toml:ident, $key:expr, bool, optional ) => {{
        $toml.lookup($key)
             .map(|val| val.as_bool().unwrap())
    }};
}

#[derive(Debug, Clone, Serialize, Deserialize, RustcEncodable, PartialEq, Eq, Hash)]
// XXX: local/remote choices might probably be sufficient
pub enum ExecType {
    Console,
    Local,
    Ssh,
}

#[derive(Debug, Clone, Serialize, Deserialize, RustcEncodable, PartialEq, Eq, Hash)]
pub struct Exec {
    /// of enum ExecType
    pub exec_type: ExecType,
    /// Hostname on which this Exec will be executed.
    pub host: Option<String>,
    /// For the time being this is supposed to be directly
    /// executed on the guest side.
    pub command: String,
    /// Optionally you can set an expected stdout.
    pub expect_stdout: Option<String>,
    /// Optionally you can set an expected stderr.
    pub expect_stderr: Option<String>,
    /// Optionally you can set an expected exit code.
    pub expect_status: Option<i32>,
    /// If either expect_stdout or expect_status set,
    /// and if this is set true, all the following
    /// executions would be skipped on an unexpecte result.
    pub abort_on_failure: bool,
}

impl Exec {
    pub fn from_toml(tml: &toml::Value) -> Result<Exec> {
        let exec_type = unfold!(tml, "type", String);
        let command = unfold!(tml, "command", String);
        let expect_stdout = unfold!(tml, "stdout", String, optional);
        let expect_stderr = unfold!(tml, "stderr", String, optional);
        let expect_status = unfold!(tml, "status", i32, optional);
        let abort_on_failure = unfold!(tml, "abort_on_failure", bool, optional, false);
        match &*exec_type {
            "console" => Ok(Exec {
                exec_type: ExecType::Console,
                host: unfold!(tml, "host", String, optional),
                command: command,
                expect_stdout: expect_stdout,
                expect_stderr: expect_stderr,
                expect_status: expect_status,
                abort_on_failure: abort_on_failure
            }),
            "local" => Ok(Exec {
                exec_type: ExecType::Console,
                host: Some("localhost".to_string()),
                command: command,
                expect_stdout: expect_stdout,
                expect_stderr: expect_stderr,
                expect_status: expect_status,
                abort_on_failure: abort_on_failure
            }),
            "ssh" => Ok(Exec {
                exec_type: ExecType::Ssh,
                host: unfold!(tml, "host", String, optional),
                command: command,
                expect_stdout: expect_stdout,
                expect_stderr: expect_stderr,
                expect_status: expect_status,
                abort_on_failure: abort_on_failure,
            }),
            _ => Err("failed to build exec".into())
        }
    }
}

pub mod setting;
pub mod template;
pub mod cluster;

use self::setting::Setting;
use self::template::Template;
use self::cluster::Cluster;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, RustcEncodable)]
pub struct Config {
    pub setting: Arc<Setting>,
    pub templates: HashSet<Arc<Template>>,
    pub clusters: HashSet<Arc<Cluster>>,
    pub created_at: i64,
}

// XXX: TryFrom
impl From<Vec<u8>> for Config {
    fn from(v: Vec<u8>) -> Self {
        let buf = String::from_utf8(v).unwrap();
        serde_json::from_str(&buf).unwrap()
    }
}

impl Storable for Config {
    fn db_path() -> PathBuf {
        ::consts::DATA_DIR.join("config")
    }
    fn key(&self) -> Vec<u8> {
        unsafe { mem::transmute::<i64, [u8; 8]>(self.created_at).to_vec() }
    }
}

impl Config {
    pub fn from_toml_file(path: &Path) -> Result<Config> {
        let mut file = File::open(path.to_str().unwrap())
            .expect(format!("Cannot open toml file: {}", path.display()).as_ref());
        let mut buf = String::new();
        file.read_to_string(&mut buf)
            .expect(format!("Cannot read toml file: {}", path.display()).as_ref());
        match buf.parse() {
            Ok(tml) => {
                Config::from_toml(&tml)
            },
            Err(err) => {
                for e in err.iter() {
                    error!("{}", e);
                }
                Err("invalid toml".into())
            }
        }
    }
    pub fn from_toml(tml: &toml::Value) -> Result<Config> {
        // global setting
        let setting = if let Some(val) = tml.lookup("setting") {
            Arc::new(Setting::from_toml(&val))
        } else {
            // if blank, default setting applied.
            Arc::new(Setting::default())
        };

        // templates
        let templates = if let Some(&toml::Value::Array(ref vals)) = tml.lookup("template") {
            vals.iter().map(|val| {
                // XXX: should we just overlook broken config?
                Arc::new(Template::from_toml(&val, &setting).unwrap())
            }).collect::<HashSet<_>>()
        } else {
            // there must be at least one template definition
            return Err("no template found".into())
        };

        // clusters
        let clusters = if let Some(&toml::Value::Array(ref vals)) = tml.lookup("cluster") {
            vals.iter().map(|val| {
                // XXX: should we just overlook broken config?
                Arc::new(Cluster::from_toml(&val, &templates.clone()).unwrap())
            }).collect::<HashSet<_>>()
        } else {
            // there must be at least one cluster definition
            return Err("no cluster found".into())
        };

        Ok(Config {
            setting: setting,
            templates: templates,
            clusters: clusters,
            created_at: time::get_time().sec,
        })
    }
}
