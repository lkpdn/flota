#![feature(concat_idents,
           custom_attribute,
           inclusive_range_syntax,
           rustc_macro)]
#![recursion_limit = "1024"]
extern crate ansi_term;
extern crate bit_vec;
extern crate crypto;
extern crate difference;
extern crate env_logger;
#[macro_use]
extern crate error_chain;
extern crate getopts;
extern crate git2;
extern crate libc;
#[macro_use]
extern crate log;
extern crate hyper;
#[macro_use]
extern crate lazy_static;
extern crate nix;
extern crate notify;
#[macro_use]
extern crate quick_error;
extern crate rustc_serialize;
extern crate ssh2;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;
extern crate term;
extern crate time;
extern crate toml;
extern crate unqlite;
extern crate url;
extern crate uuid;
extern crate xml;

use getopts::Options;
use std::env;
use std::fs;
use std::os::unix::io::AsRawFd;
use std::sync::{Arc, Mutex};
use nix::sys::signal;
use nix::unistd::{close, dup2, fork, ForkResult, getppid, sleep};
use std::path::Path;
use std::process;
use std::process::Command;

pub mod consts;
use consts::*;

pub mod exec;

pub mod libvirt;

#[macro_use]
pub mod util;
use util::errors::*;
use util::notify::config_hup;

pub mod flota;
use flota::config::*;
use flota::config::template::Ingredient;
use flota::manager::Manager;
use flota::template::Template;

#[macro_use]
pub mod virt;
use virt::conn::Conn;
use virt::ResourceBlend;
use virt::storage::pool::StoragePool;

pub mod distro;
use distro::Distros;

pub mod store;
use store::ConfigStore;

fn print_usage(opts: Options) {
    let brief = format!("Usage: {}", *PROGNAME);
    print!("{}", opts.usage(&brief));
}

static mut CONFIG_RELOAD: bool = false;
static mut SIGTERM_RECVED: bool = false;
lazy_static! {
    static ref WAIT_FOR: Mutex<Vec<i32>> = Mutex::new(vec![]);
}

#[allow(unused_variables)]
fn daemonize() -> Result<()> {
    if getppid() == 1 {
        return Err("already daemonized".into());
    }
    match fork().expect("fork failed") {
        ForkResult::Parent { child } => {
            process::exit(0);
        }
        ForkResult::Child => {}
    }
    if unsafe { libc::setsid() } < 0 {
        error!("setsid failed.");
        process::exit(1);
    }
    match (fs::OpenOptions::new().read(true).write(true).open("/dev/null"),
           fs::OpenOptions::new().read(true).write(true).create(true).open(LOGFILE.as_os_str()),
           fs::OpenOptions::new().read(true).write(true).create(true).open(LOGERROR.as_os_str())) {
        (Ok(n), Ok(f), Ok(e)) => {
            let n_raw = n.as_raw_fd();
            let f_raw = f.as_raw_fd();
            let e_raw = e.as_raw_fd();
            dup2(n_raw, libc::STDIN_FILENO).expect("dup2 failed");
            dup2(f_raw, libc::STDOUT_FILENO).expect("dup2 failed");
            dup2(e_raw, libc::STDERR_FILENO).expect("dup2 failed");
            if n_raw > 2 {
                close(n_raw).expect("cannot close fd opened for /dev/null");
            }
        },
        _ => panic!("failed to daemonize. dup2 failed.")
    }
    Ok(())
}

extern "C" fn config_reload(_: i32) {
    unsafe { CONFIG_RELOAD = true };
}

extern "C" fn sigterm_received(_: i32) {
    unsafe { SIGTERM_RECVED = true };
}

fn verify_env() -> Result<()> {
    // selinux disabled?
    match Command::new("getenforce")
        .output()
        .expect("failed to execute getenforce")
        .stdout {
            ref s if String::from_utf8(s.clone()).unwrap() == "Disabled\n" => {}
            _ => {
                panic!("selinux must be disabled.");
        }
    }

    // data dir exists?
    match DATA_DIR.metadata() {
        Ok(attr) => {
            if ! attr.is_dir() {
                panic!("data dir ({}) does not exists.", DATA_DIR.to_str().unwrap());
            }
        },
        Err(e) => {
            panic!("{}", e);
        }
    }

    // XXX: if dnsmasq running on hosts, check its config and make sure "bind-interfaces" uncommented.
    // XXX: cli utils availability check
    Ok(())
}

fn main() {
    env_logger::init().unwrap();

    // parse options
    let args: Vec<String> = env::args().collect();
    let mut opts = Options::new();
    opts.optflag("h", "help", "print this help menu");
    opts.optopt("c", "config", format!(
            "config toml file (DEFAULT: /etc/flota.toml)").as_str(), "FILE");
    opts.optflag("", "clean", "remove all templates and clusters/hosts.");
    opts.optflag("", "reset", "reset all.");
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(e) => {
            println!("{}", e.to_string());
            print_usage(opts);
            return;
        }
    };
    if matches.opt_present("h") {
        print_usage(opts);
        return;
    }
    let config_path = match matches.opt_str("c") {
        Some(c) => { c },
        None => { "/etc/flota.toml".to_string() }
    };
    if matches.opt_present("reset") {
        println!("would reset");
        return;
    }
    if matches.opt_present("clean") {
        println!("would clean");
        return;
    }

    // verify environment
    verify_env().unwrap();

    let config_store = store::unqlite_backed::ConfigStore::new(
        ::consts::CONFIG_HISTORY_DIR.join("hoge").as_path()
    );

    // outermost loop
    'init: loop {
        if unsafe { SIGTERM_RECVED } { break }
        // read toml
        match Config::from_toml_file(Path::new(&config_path)) {
            Ok(config) => {
                debug!("{:#?}", config);
                config_store.update(&config).unwrap();

                // set up main connection
                let conn = Conn::new(&config.setting.hypervisor);

                // set up default storage pool + default network
                let default_storage_pool = StoragePool::ensure(&conn,
                                                               &config.setting.default_storage_pool_name,
                                                               &config.setting.pool_root)
                    .expect("cannot make sure default storage exists and is active");
                let default_nw_br_ip = &config.setting.default_network.nth_sibling(1);
                let default_network =
                    virt::network::Network::ensure_default(&conn, &default_nw_br_ip, true);
                let mut default_resources = ResourceBlend::new(&conn);
                default_resources.put_network(&default_network);
                default_resources.put_pool(&default_storage_pool);

                if config.setting.daemonized {
                    // if it's already daemonized, returns Err.
                    // in other words, changing the "daemonized" config value
                    // to true is one-way trip.
                    match daemonize() {
                        Ok(_) => {
                            let hup_action = signal::SigAction::new(
                                signal::SigHandler::Handler(config_reload),
                                signal::SaFlags::empty(),
                                signal::SigSet::empty());
                            let term_action = signal::SigAction::new(
                                signal::SigHandler::Handler(sigterm_received),
                                signal::SaFlags::empty(),
                                signal::SigSet::empty());
                            unsafe {
                                signal::sigaction(signal::SIGHUP, &hup_action)
                                    .expect("sigaction for SIGHUP failed");
                                signal::sigaction(signal::SIGTERM, &term_action)
                                    .expect("sigaction for SIGTERM failed");
                                WAIT_FOR.lock().unwrap().push(config_hup(Path::new(&config_path))
                                    .expect("failed to setup config_hup"));
                            }
                        },
                        Err(_) => {}
                    }
                }

                // staying in this inner loop
                'cycle: loop {
                    // construct templates.
                    let mut templates = Vec::new();

                    for ref template in &config.templates {
                        let distro = match &template.ingredient {
                            &Ingredient::OffTheShelf {
                                ref distro
                            } => {
                                Distros::search(&distro, &template.arch)
                            },
                            // XXX: linux is not the only choice
                            &Ingredient::Custom {
                                ref iso,
                                ref iso_md5sum,
                                ref vmlinuz,
                                ref initrd,
                            } => {
                                Distros::custom(iso, iso_md5sum, vmlinuz, initrd)
                            }
                        };
                        match Template::new(&default_resources, template, distro) {
                            Ok(t) => {
                                templates.push(Arc::new(t));
                            }
                            Err(e) => {
                                warn!("{}", e);
                                continue;
                            }
                        };
                    }

                    // construct (+ run tests on) clusters.
                    // TODO: safely parallelize
                    for ref cluster in &config.clusters {
                        match Manager::run_cluster(cluster, &templates) {
                            Ok(true) => {
                                info!("cluster {}: ok", cluster.name);
                            },
                            Ok(false) => {
                                info!("cluster {}: failed", cluster.name);
                            },
                            Err(e) => {
                                error!("cluster {} error: {}", cluster.name, e);
                            }
                        }
                    }

                    if ! config.setting.daemonized ||
                       unsafe { SIGTERM_RECVED } { break 'init }

                    sleep(5);
                    if unsafe { CONFIG_RELOAD } {
                        unsafe { CONFIG_RELOAD = false };
                        match Config::from_toml_file(Path::new(&config_path)) {
                            Ok(ref new_config) => {
                                if let Ok(true) = config_store.update(&new_config) {
                                    break 'cycle;
                                }
                            },
                            Err(e) => {
                                error!("{}", e);
                            }
                        }
                    }
                }
            },
            Err(e) => {
                error!("{}", e);
                sleep(5);
            }
        }
    }
    for child in WAIT_FOR.lock().unwrap().iter() {
        signal::kill(*child, libc::SIGKILL)
            .expect(format!("failed to send SIGKILL to {}", child).as_str());
    }
}
