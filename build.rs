extern crate bindgen;

use std::env;
use std::fs;
use std::io::prelude::*;
use std::fs::{create_dir, File};
use std::path;

fn bind(hdr_path: &str, bnd_path: &str, lib: &str) {
    if path::Path::new(bnd_path).exists() {
        return ();
    }
    let mut bindings = bindgen::Builder::new(hdr_path);
    bindings.link(lib, bindgen::LinkType::Dynamic);
    let generated_bindings = bindings.generate().expect("Failed to generate bindings");
    let mut file = File::create(bnd_path).expect("Failed to open file");
    file.write(generated_bindings.to_string().as_bytes()).unwrap();
}

#[cfg_attr(rustfmt, rustfmt_skip)]
fn gen_consts() {
    let data_dir = match env::var("DATA_DIR_ROOT") {
        Ok(ref d) if d.as_str().chars().rev().take(1).collect::<String>() == "/"
            => { d.to_owned() },
        Ok(d) => {  d + "/" },
        Err(_) => { "/var/lib/".to_string() }
    };
    let consts_rs = "./src/consts.rs";
    if let Ok(mut f) = File::create(consts_rs) {
        write!(&mut f, "\
            use std::path::PathBuf;\n\
            \n\
            lazy_static! {{\n\
                pub static ref DATA_DIR: PathBuf = PathBuf::from(\n\
                    format!(\"{}{{}}\", *::PROGNAME).as_str());\n\
                pub static ref CONFIG_HISTORY_DIR: PathBuf = DATA_DIR.join(\"/config/history\");\n\
            }}", data_dir).unwrap();
    } else { panic!("cannot open file") }
    if let Ok(d) = fs::metadata(&data_dir) {
        if ! d.is_dir() {
            create_dir(data_dir).expect("Failed to create data dir");
        }
    }
}

fn main() {
    bind("/usr/include/libvirt/virterror.h",
         "./src/libvirt.rs",
         "virt");
    gen_consts();
}
