use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use time;
use toml;
use ::consts::*;
use ::util::errors::*;
use ::util::ipv4::IPv4;
use ::util::url::Url;


#[derive(Debug, Clone, RustcEncodable)]
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

#[allow(dead_code)]
enum UnattendedInstallation {
    Kickstart(String),
    KickstartFile(PathBuf),
}

#[derive(Debug, Clone, RustcEncodable)]
pub enum Ingredient {
    /// distro + arch (+ unattended)
    OffTheShelf {
        /// Distro to use.
        /// All available distros can be seen with the command:
        /// "./PROGNAME templates show"
        distro: String,
    },
    /// iso (+ md5sum) + vmlinuz + initrd + arch (+ unattended)
    Custom {
        /// ISO uri.
        iso: Url,
        /// md5 checksum file uri to be checked against about iso.
        /// Note that if it contains several irrelevant target
        /// files' md5 and therefore in two columns style, try and
        /// parse it automatically. If the parsing fails, md5 checking
        /// will skipped with some warn message.
        /// DEFAULT: None
        iso_md5sum: Option<Url>,
        /// For the time being templates are supposed to be
        /// created with direction boot installation.
        vmlinuz: Url,
        /// For the time being templates are supposed to be
        /// created with direction boot installation.
        /// _no default value_
        initrd: Url,
    },
}

impl Ingredient {
    pub fn distinguish(tml: &toml::Value) -> Result<Self> {
        let distro = tml.lookup("distro")
            .map(|val| val.as_str().unwrap());
        let iso = tml.lookup("iso")
            .map(|val| Url::parse(val.as_str().unwrap()).unwrap());
        let iso_md5sum = tml.lookup("iso_md5sum")
            .map(|val| Url::parse(val.as_str().unwrap()).unwrap());
        let vmlinuz = tml.lookup("vmlinuz")
            .map(|val| Url::parse(val.as_str().unwrap()).unwrap());
        let initrd = tml.lookup("initrd")
            .map(|val| Url::parse(val.as_str().unwrap()).unwrap());
        match (distro, iso, iso_md5sum, vmlinuz, initrd) {
            (Some(_), Some(_), _, Some(_), Some(_)) => {
                Err("cannot tell which ingredient type you intend".into())
            }
            (Some(distro_), _, _, _, _) => {
                Ok(Ingredient::OffTheShelf { distro: distro_.to_owned() })
            }
            (None, Some(iso_), iso_md5sum_, Some(vmlinuz_), Some(initrd_)) => {
                Ok(Ingredient::Custom {
                    iso: iso_,
                    iso_md5sum: iso_md5sum_,
                    vmlinuz: vmlinuz_,
                    initrd: initrd_,
                })
            }
            _ => Err("insufficient configuration".into()),
        }
    }
}

#[derive(Debug, Clone, RustcEncodable)]
pub struct Template {
    /// Template name.
    pub name: String,
    /// Template architecture.
    pub arch: String,
    /// This is the enum which is defined beforehand.
    pub ingredient: Ingredient,
    pub ks: Option<String>,
    /// SSH Login user name for management use.
    pub mgmt_user: String,
    /// SSH private key path
    pub mgmt_user_ssh_private_key: PathBuf,
}

impl Template {
    pub fn from_toml(tml: &toml::Value) -> Template {
        let name = tml.lookup("name")
            .map(|val| val.as_str().unwrap())
            .unwrap();
        let arch = tml.lookup("arch")
            .map(|val| val.as_str().unwrap())
            .unwrap();
        let ingredient = match Ingredient::distinguish(tml) {
            Ok(ing) => ing,
            Err(_) => panic!("would not panic!"),
        };
        let ks = tml.lookup("ks")
            .map(|val| val.as_str().unwrap().to_owned());
        let mgmt_user = tml.lookup("mgmt_user")
            .map(|val| val.as_str().unwrap().to_owned())
            .unwrap_or(format!("admin_{}", *PROGNAME));
        let mgmt_user_ssh_private_key = tml.lookup("mgmt_user_ssh_private_key")
            .map(|val| PathBuf::from(val.as_str().unwrap()))
            .unwrap_or(PathBuf::from(format!("/home/{}/.ssh/id_rsa", mgmt_user)));
        Template {
            name: name.to_owned(),
            arch: arch.to_owned(),
            ingredient: ingredient,
            ks: ks,
            mgmt_user: mgmt_user,
            mgmt_user_ssh_private_key: mgmt_user_ssh_private_key,
        }
    }
}

#[derive(Debug, Clone, RustcEncodable)]
pub struct Exec {
    /// Hostname on which this Exec will be executed.
    pub hostname: String,
    /// For the time being this has to be an ssh user
    /// to login and execute this Exec. Whether it has
    /// to be a password-less sudoer depends.
    pub user: String,
    /// For the time being this is supposed to be directly
    /// executed on the guest side.
    pub command: String,
    /// Optionally you can set an expected stdout.
    pub expect_stdout: Option<String>,
    /// Optionally you can set an expected exit code.
    pub expect_status: Option<i64>,
    /// If either expect_stdout or expect_status set,
    /// and if this is set true, all the following
    /// executions would be skipped on an unexpecte result.
    pub abort_on_failure: bool,
}

impl Exec {
    pub fn from_toml(tml: &toml::Value) -> Exec {
        let hostname = tml.lookup("hostname")
            .map(|val| val.as_str().unwrap())
            .unwrap();
        let user = tml.lookup("user")
            .map(|val| val.as_str().unwrap())
            .unwrap();
        let command = tml.lookup("command")
            .map(|val| val.as_str().unwrap())
            .unwrap();
        let expect_stdout = tml.lookup("expect_stdout")
            .map(|val| val.as_str().unwrap().to_string());
        let expect_status = tml.lookup("expect_status")
            .map(|val| val.as_integer().unwrap());
        let abort_on_failure = tml.lookup("abort_on_failure")
            .map(|val| val.as_bool().unwrap())
            .unwrap();
        Exec {
            hostname: hostname.to_owned(),
            user: user.to_owned(),
            command: command.to_owned(),
            expect_stdout: expect_stdout,
            expect_status: expect_status,
            abort_on_failure: abort_on_failure,
        }
    }
}

#[derive(Debug, Clone, RustcEncodable)]
pub struct HostInterface {
    /// Network interface dev name on guest side.
    pub dev: String,
    /// For the time being only ipv4 supported.
    pub ip: IPv4,
}

impl HostInterface {
    pub fn from_toml(tml: &toml::Value) -> HostInterface {
        let dev = tml.lookup("dev")
            .map(|val| val.as_str().unwrap())
            .unwrap();
        let ip = tml.lookup("ip")
            .map(|val| IPv4::from_cidr_notation(val.as_str().unwrap()).unwrap())
            .unwrap();
        HostInterface {
            dev: dev.to_owned(),
            ip: ip,
        }
    }
}

#[derive(Debug, Clone, RustcEncodable)]
pub struct Host {
    /// Hostname.
    pub hostname: String,
    /// Exact template name to be based on.
    pub template: String,
    /// Network interfaces.
    pub interfaces: Vec<HostInterface>,
    /// Additional setups before standalone tests.
    pub solo_pre_tests: Option<Vec<Exec>>,
    /// Standalone tests.
    pub solo_tests: Option<Vec<Exec>>,
    /// Additional execs after standalone tests.
    pub solo_post_tests: Option<Vec<Exec>>,
    /// If true, poweroff after the cluster it belongs
    /// to has finished all tasks.
    /// DEFAULT: true
    pub destroy_when_finished: bool,
    /// If false, completely erase this host after the
    /// cluster it belongs to has finidhed all tasks.
    /// DEFAULT: true
    pub persistent: bool,
    /// To override Template's definition for SSH Login
    /// user name for management use.
    pub mgmt_user: Option<String>,
    /// SSH private key path
    pub mgmt_user_ssh_private_key: Option<PathBuf>,
}

impl Host {
    pub fn from_toml(tml: &toml::Value) -> Host {
        let hostname = tml.lookup("hostname")
            .map(|val| val.as_str().unwrap())
            .unwrap();
        let template = tml.lookup("template")
            .map(|val| val.as_str().unwrap())
            .unwrap();
        let interfaces = match tml.lookup("interface") {
            Some(&toml::Value::Array(ref tml_ifs)) => {
                let mut ifs = Vec::new();
                for tml_if in tml_ifs {
                    ifs.push(HostInterface::from_toml(&tml_if));
                }
                ifs
            }
            _ => {
                panic!("interface definition not found. host: {}", hostname);
            }
        };
        let solo_pre_tests = match tml.lookup("solo_pre_tests") {
            Some(&toml::Value::Array(ref tml_execs)) => {
                let mut execs = Vec::new();
                for tml_exec in tml_execs {
                    execs.push(Exec::from_toml(&tml_exec));
                }
                Some(execs)
            }
            _ => None,
        };
        let solo_tests = match tml.lookup("solo_tests") {
            Some(&toml::Value::Array(ref tml_execs)) => {
                let mut execs = Vec::new();
                for tml_exec in tml_execs {
                    execs.push(Exec::from_toml(&tml_exec));
                }
                Some(execs)
            }
            _ => None,
        };
        let solo_post_tests = match tml.lookup("solo_post_tests") {
            Some(&toml::Value::Array(ref tml_execs)) => {
                let mut execs = Vec::new();
                for tml_exec in tml_execs {
                    execs.push(Exec::from_toml(&tml_exec));
                }
                Some(execs)
            }
            _ => None,
        };
        let destroy_when_finished = tml.lookup("destroy_when_finished")
            .map(|val| val.as_bool().unwrap())
            .unwrap_or(true);
        let persistent = tml.lookup("persistent")
            .map(|val| val.as_bool().unwrap())
            .unwrap_or(true);
        let mgmt_user = tml.lookup("mgmt_user")
            .map(|val| val.as_str().unwrap().to_owned());
        let mgmt_user_ssh_private_key = tml.lookup("mgmt_user_ssh_private_key")
            .map(|val| PathBuf::from(val.as_str().unwrap()));
        Host {
            hostname: hostname.to_owned(),
            template: template.to_owned(),
            interfaces: interfaces,
            solo_pre_tests: solo_pre_tests,
            solo_tests: solo_tests,
            solo_post_tests: solo_post_tests,
            destroy_when_finished: destroy_when_finished,
            persistent: persistent,
            mgmt_user: mgmt_user,
            mgmt_user_ssh_private_key: mgmt_user_ssh_private_key,
        }
    }
}

#[derive(Debug, Clone, RustcEncodable)]
pub struct Cluster {
    /// Cluster name arbitrarily chosen.
    pub name: String,
    /// Hosts which belong to this cluster. Note that
    /// these are set up in the same order.
    pub hosts: Vec<Host>,
    /// Additional execs after all the hosts have been
    /// provisioned in the standalone tasks. Note that
    /// these execs will be executed in the presice order.
    pub pre_tests: Vec<Exec>,
    /// Cluster tests sequence.
    pub tests: Vec<Exec>,
    /// Additional execs after the cluster tests.
    pub post_tests: Vec<Exec>,
    /// If true, poweroff all the hosts except ones
    /// spcified otherwise in host granularity.
    pub destroy_when_finished: bool,
    /// If false, completely erase all the hosts except
    /// ones specified otherwise in host granularity.
    pub persistent: bool,
}

impl Cluster {
    pub fn from_toml(tml: &toml::Value) -> Option<Cluster> {
        let name = tml.lookup("name").map(|val| val.as_str().unwrap()).unwrap();
        let hosts = match tml.lookup("host") {
            Some(&toml::Value::Array(ref tml_hsts)) => {
                let mut hsts = Vec::new();
                for tml_hst in tml_hsts {
                    hsts.push(Host::from_toml(&tml_hst));
                }
                hsts
            }
            _ => {
                warn!("no hosts found in cluster: {}, so skip it.", name);
                return None;
            }
        };
        let pre_tests = match tml.lookup("pre_tests") {
            Some(&toml::Value::Array(ref tml_execs)) => {
                let mut execs = Vec::new();
                for tml_exec in tml_execs {
                    execs.push(Exec::from_toml(&tml_exec));
                }
                execs
            }
            _ => vec![],
        };
        let tests = match tml.lookup("tests") {
            Some(&toml::Value::Array(ref tml_execs)) => {
                let mut execs = Vec::new();
                for tml_exec in tml_execs {
                    execs.push(Exec::from_toml(&tml_exec));
                }
                execs
            }
            _ => vec![],
        };
        let post_tests = match tml.lookup("post_tests") {
            Some(&toml::Value::Array(ref tml_execs)) => {
                let mut execs = Vec::new();
                for tml_exec in tml_execs {
                    execs.push(Exec::from_toml(&tml_exec));
                }
                execs
            }
            _ => vec![],
        };
        let destroy_when_finished = tml.lookup("destroy_when_finished")
            .map(|val| val.as_bool().unwrap())
            .unwrap_or(true);
        let persistent = tml.lookup("persistent")
            .map(|val| val.as_bool().unwrap())
            .unwrap_or(true);
        Some(Cluster {
            name: name.to_owned(),
            hosts: hosts,
            pre_tests: pre_tests,
            tests: tests,
            post_tests: post_tests,
            destroy_when_finished: destroy_when_finished,
            persistent: persistent,
        })
    }
}

#[derive(Debug, Clone, RustcEncodable)]
pub struct Config {
    pub setting: Setting,
    pub templates: Vec<Template>,
    pub clusters: Vec<Cluster>,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            setting: Setting::default(),
            templates: Vec::new(),
            clusters: Vec::new(),
        }
    }
}

impl Config {
    pub fn from_toml_file(path: &Path) -> Result<Config> {
        let mut file = File::open(path.to_str().unwrap())
            .expect(format!("Cannot open toml file: {}", path.display()).as_ref());
        let mut buf = String::new();
        file.read_to_string(&mut buf)
            .expect(format!("Cannot read toml file: {}", path.display()).as_ref());
        let tml: toml::Value = buf.parse().unwrap();
        Config::from_toml(&tml)
    }
    pub fn from_toml(tml: &toml::Value) -> Result<Config> {
        let setting = if let Some(v) = tml.lookup("setting") {
            Setting::from_toml(&v)
        } else {
            Setting::default()
        };
        let templates = match tml.lookup("template") {
            Some(&toml::Value::Array(ref tml_tmpls)) => {
                let mut tmpls = Vec::new();
                for tml_tmpl in tml_tmpls {
                    tmpls.push(Template::from_toml(&tml_tmpl));
                }
                tmpls
            }
            _ => vec![],
        };
        let clusters = match tml.lookup("cluster") {
            Some(&toml::Value::Array(ref tml_cltrs)) => {
                let mut cltrs = Vec::new();
                for tml_cltr in tml_cltrs {
                    match Cluster::from_toml(&tml_cltr) {
                        Some(cltr) => cltrs.push(cltr),
                        None => {}
                    }
                }
                cltrs
            }
            _ => vec![],
        };
        // XXX: validate here
        Ok(Config {
            setting: setting,
            templates: templates,
            clusters: clusters,
        })
    }
    pub fn as_toml(&self) -> toml::Value {
        toml::encode(self)
    }
    pub fn differ_from(&self, config: &Config) -> bool {
        self.as_toml() == config.as_toml()
    }
    pub fn snapshot(&self) -> Result<()> {
        let save_to = ::consts::CONFIG_HISTORY_DIR.join(format!(
          "config-{}", time::now().to_timespec().sec).as_str());
        if let Ok(mut f) = File::create(save_to.to_str().unwrap()) {
            write!(&mut f, "{}", toml::encode_str(self)).map_err(|e| e.into())
        } else {
            Err("cannot take snapshot of config".into())
        }
    }
    pub fn latest_saved_config() -> Option<Config> {
        if let Some(entries) = fs::read_dir(
            ::consts::CONFIG_HISTORY_DIR.to_str().unwrap()
        ).ok() {
            let mut dentries = entries.map(|v| v.unwrap())
                                      .collect::<Vec<fs::DirEntry>>();
            dentries.sort_by(|d1, d2| {
                    let m1 = d1.metadata().unwrap().modified().unwrap();
                    let m2 = d2.metadata().unwrap().modified().unwrap();
                    m1.cmp(&m2)
            });
            if let Some(entry) = dentries.first() {
                Some(Self::from_toml_file(::consts::CONFIG_HISTORY_DIR.join(
                            entry.file_name()).as_path()).unwrap())
            } else {
                None
            }
        } else {
            None
        }
    }
}
