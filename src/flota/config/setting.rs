use toml;
use ::consts::*;
use ::util::ipv4::IPv4;

#[derive(Debug, Clone, RustcEncodable, PartialEq, Eq, Hash)]
pub struct Setting {
    /// Hypervisor uri to connect.
    /// _no default value_
    pub hypervisor: String,
    /// For the time being only file-based disk images are
    /// created and the root directory where all of those
    /// will reside is defined with this one field.
    /// _no default value_
    pub pool_root: String,
    /// Default network.
    pub default_network: IPv4,
    pub default_storage_pool_name: String,
    /// If true, no actions are taken to destroy and erase
    /// things that have been constructed when the programme
    /// exits. Otherwise all those will be erased unless
    /// specified not to be done so in a more fine-grained
    /// settings s.t. Cluster or Host settings.
    /// DEFAULT: true
    pub persistent: bool,
    /// If true, and also if run in daemon-mode, delete
    /// unused templates once detected.
    /// DEFAULT: true
    pub delete_unused_template: bool,
    /// If true, run in daemon mode
    /// DEFAULT: false
    pub daemonized: bool,
}

impl Default for Setting {
    fn default() -> Setting {
        Setting {
            hypervisor: "qemu:///system".to_string(),
            pool_root: "/tmp".to_string(),
            default_network: IPv4::from_cidr_notation("203.0.113.0/24").unwrap(),
            default_storage_pool_name: format!("_{}", *PROGNAME),
            persistent: true,
            delete_unused_template: true,
            daemonized: false,
        }
    }
}

impl Setting {
    pub fn from_toml(tml: &toml::Value) -> Setting {
        let mut setting = Setting::default();
        if let Some(val) = tml.lookup("hypervisor") {
            setting.hypervisor = val.as_str().unwrap().to_owned();
        }
        if let Some(val) = tml.lookup("pool_root") {
            setting.pool_root = val.as_str().unwrap().to_owned();
        }
        if let Some(val) = tml.lookup("persistent") {
            setting.persistent = val.as_bool().unwrap();
        }
        if let Some(val) = tml.lookup("delete_unused_template") {
            setting.delete_unused_template = val.as_bool().unwrap();
        }
        if let Some(val) = tml.lookup("daemonized") {
            setting.daemonized = val.as_bool().unwrap();
        }
        setting
    }
}
