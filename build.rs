extern crate bindgen;

use std::io::prelude::*;
use std::fs::File;
use std::path;

fn bind(hdr_path: &str, bnd_path: &str, lib: &str) {
    if path::Path::new(bnd_path).exists() { return () }
    let mut bindings = bindgen::Builder::new(hdr_path);
    bindings.link(lib, bindgen::LinkType::Dynamic);
    let generated_bindings = bindings.generate().expect("Failed to generate bindings");
    let mut file = File::create(bnd_path).expect("Failed to open file");
    file.write(generated_bindings.to_string().as_bytes()).unwrap();
}

fn main(){
    bind("/usr/include/libvirt/virterror.h", "./src/libvirt.rs", "virt");
}
