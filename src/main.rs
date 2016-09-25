#![feature(concat_idents, inclusive_range_syntax, trace_macros, type_macros, custom_attribute)]
#![recursion_limit = "1024"]
extern crate ansi_term;
extern crate bit_vec;
extern crate crypto;
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
extern crate ssh2;
extern crate term;
extern crate time;
extern crate toml;
extern crate url;
extern crate uuid;
extern crate xml;

use getopts::Options;
use std::env;
use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;
use nix::sys::signal;
use nix::unistd::{close, dup2, fork, ForkResult, getppid, sleep};
use std::path::Path;
use std::process;
use std::process::Command;

use ::distro::Distros;

pub mod libvirt;

#[macro_use]
pub mod util;

pub mod flota;
use flota::cluster::Cluster;
use flota::config::Config;
use flota::template::Template;

#[macro_use]
pub mod virt;
use virt::conn::Conn;
use virt::ResourceBlend;
use virt::storage::pool::StoragePool;

pub mod distro;

fn print_usage(opts: Options) {
    let brief = format!("Usage: {}", *PROGNAME);
    print!("{}", opts.usage(&brief));
}

lazy_static! {
    pub static ref PROGNAME: String = {
        let args: Vec<String> = env::args().collect();
        let program = args[0].clone();
        Path::new(&program)
          .file_name().unwrap()
          .to_str().unwrap().to_string()
    };
}
static mut CONFIG_RELOAD: bool = false;

#[allow(unused_variables)]
fn daemonize() {
    if getppid() == 1 {
        return;
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
    match OpenOptions::new().read(true).write(true).open("/dev/null") {
        Ok(f) => {
            let f_raw = f.as_raw_fd();
            dup2(f_raw, libc::STDIN_FILENO).expect("dup2 failed");
            dup2(f_raw, libc::STDOUT_FILENO).expect("dup2 failed");
            dup2(f_raw, libc::STDERR_FILENO).expect("dup2 failed");
            if f_raw > 2 {
                close(f_raw).expect("cannot close fd opened for /dev/null");
            }
        }
        Err(e) => panic!("cannot open /dev/null ({})", e),
    }
}

extern "C" fn config_reload(_: i32) {
    unsafe { CONFIG_RELOAD = true };
}

fn main() {
    env_logger::init().unwrap();

    // parse options
    let args: Vec<String> = env::args().collect();
    let mut opts = Options::new();
    opts.optflag("h", "help", "print this help menu");
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
    if matches.opt_present("reset") {
        println!("would reset");
        return;
    }
    if matches.opt_present("clean") {
        println!("would clean");
        return;
    }

    // environment checking
    match Command::new("getenforce")
        .output()
        .expect("failed to execute getenforce")
        .stdout {
        ref s if String::from_utf8(s.clone()).unwrap() == "Disabled\n" => {}
        _ => {
            panic!("{}", "selinux must be disabled.");
        }
    }
    // XXX: version check
    // XXX: if dnsmasq running on hosts, check its config and make sure "bind-interfaces" uncommented.
    // XXX: cli utils availability check

    // outermost loop
    'init: loop {
        // read toml
        let config = Config::from_toml_file(Path::new("DevDef.toml"));
        debug!("{:#?}", config);

        // set up main connection
        let conn = Conn::new(&config.setting.hypervisor);

        // set up default storage pool + default network
        let default_storage_pool = StoragePool::ensure(&conn,
                                                       &config.setting.default_storage_pool_name,
                                                       &config.setting.pool_root)
            .expect("cannot make sure default storage exists and is active");
        let default_nw = config.setting.default_network;
        let default_nw_br_ip = default_nw.nth_sibling(1);
        let default_network =
            virt::network::Network::ensure_default(&conn, &default_nw_br_ip, true);
        let mut default_resources = ResourceBlend::new(&conn);
        default_resources.put_network(&default_network);
        default_resources.put_pool(&default_storage_pool);

        if config.setting.daemonized {
            daemonize();
            let hup_action = signal::SigAction::new(signal::SigHandler::Handler(config_reload),
                                                    signal::SaFlags::empty(),
                                                    signal::SigSet::empty());
            unsafe {
                signal::sigaction(signal::SIGHUP, &hup_action)
                    .expect("sigaction for SIGHUP failed");
            }
        }

        // unless some intentional signal received,
        // staying in this inner loop
        'cycle: loop {
            // construct templates.
            let mut templates = Vec::new();
            for ref template in config.templates.iter() {
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
            for cluster in config.clusters.iter() {
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
                    break 'cycle;
                }
            } else {
                break;
            }
        }
        if !config.setting.daemonized {
            break;
        }
    }
}
