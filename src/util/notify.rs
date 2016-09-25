use ansi_term::Colour::Green;
use libc;
use nix::unistd::{close, dup2, fork, ForkResult, pipe};
use notify;
use notify::{RecommendedWatcher, Watcher};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::mpsc::channel;

use ::util::errors::*;

pub fn tailf_background(path: &Path) -> Result<i32> {
    let (r, w) = pipe().unwrap();
    match fork().expect("fork failed") {
        ForkResult::Parent { child } => {
            close(w).expect("cannot close fd");
            dup2(r, libc::STDIN_FILENO).expect("dup2 failed");
            Ok(child)
        }
        ForkResult::Child => {
            close(r).expect("cannot close fd");
            dup2(libc::STDOUT_FILENO, w).expect("dup2 failed");
            let (tx, rx) = channel();
            let mut watcher: RecommendedWatcher = try!(Watcher::new(tx));
            try!(watcher.watch(path.to_str().unwrap()));
            let mut f = try!(File::open(path.to_str().unwrap()));
            loop {
                match rx.recv() {
                    Ok(notify::Event { path: _, op: _ }) => {
                        let mut buffer = Vec::new();
                        if let Err(_) = f.read_to_end(&mut buffer) {
                            continue;
                        }
                        if let Ok(s) = String::from_utf8(buffer) {
                            print!("{}", Green.paint(s));
                        }
                    }
                    Err(e) => {
                        error!("{}", e);
                    }
                }
            }
        }
    }
}
