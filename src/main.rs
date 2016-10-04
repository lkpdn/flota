#![feature(concat_idents, inclusive_range_syntax, trace_macros, type_macros, custom_attribute)]
#![recursion_limit = "1024"]
extern crate ansi_term;
extern crate bit_vec;
extern crate crypto;
extern crate difference;
extern crate env_logger;
#[macro_use]
extern crate error_chain;
extern crate getopts;
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
extern crate term;
extern crate time;
extern crate toml;
extern crate url;
extern crate uuid;
extern crate xml;

use getopts::Options;
use std::env;
use std::fs;
use std::os::unix::io::AsRawFd;
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
use flota::cluster::Cluster;
use flota::config::*;
use flota::template::Template;

#[macro_use]
pub mod virt;
use virt::conn::Conn;
use virt::ResourceBlend;
use virt::storage::pool::StoragePool;

pub mod distro;
use distro::Distros;

fn print_usage(opts: Options) {
    let brief = format!("Usage: {}", *PROGNAME);
    print!("{}", opts.usage(&brief));
}

static mut CONFIG_RELOAD: bool = false;

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

    let mut first_cycle = true;

    // outermost loop
    'init: loop {
        // read toml
        match Config::from_toml_file(Path::new(&config_path)) {
            Ok(config) => {
                debug!("{:#?}", config);

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
                            unsafe {
                                signal::sigaction(signal::SIGHUP, &hup_action)
                                    .expect("sigaction for SIGHUP failed");
                            }
                            let _child = config_hup(Path::new(&config_path))
                                .expect("failed to setup config_hup");
                        },
                        Err(_) => {}
                    }
                }

                // staying in this inner loop
                'cycle: loop {
                    // construct templates.
                    let mut templates = Vec::new();

                    for ref template in &config.templates {
                        let distro = Distros::search("centos6", "x86_64");
                        match Template::new(&default_resources, template, distro) {
                            Ok(t) => {
                                templates.push(t);
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
                        let _c = match Cluster::new(cluster, &templates) {
                            Ok(c) => c,
                            Err(e) => {
                                error!("{}", e);
                                continue;
                            }
                        };
                    }

                    // if in daemon-mode, sleep five seconds and loop to next.
                    if config.setting.daemonized {
                        sleep(5);
                        if unsafe { CONFIG_RELOAD } {
                            unsafe { CONFIG_RELOAD = false };
                            match Config::from_toml_file(Path::new(&config_path)) {
                                Ok(ref new_config) => {
                                    if config.differ_from(new_config) {
                                        // even if CONFIG_RELOAD was set true and reached here,
                                        // it won't break inner loop and reset config unless the
                                        // new config substantially differs from old one.
                                        new_config.snapshot() 
                                                  .expect("cannot save config snapshot");
                                        break 'cycle;
                                    }
                                },
                                Err(e) => {
                                    error!("{}", e);
                                }
                            }
                        }
                    } else {
                        break 'init;
                    }
                }
                first_cycle = false;
            },
            Err(e) => {
                error!("{}", e);
                if first_cycle { 
                    break
                } else {
                    // when broken config left for a long time,
                    // you'd get error message flood so slightly long
                    // sleep time chosen here
                    sleep(20);
                }
            }
        }
    }
}
