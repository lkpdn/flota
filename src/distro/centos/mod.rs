use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use ::util::errors::*;

pub mod release_6;

#[allow(dead_code)]
pub struct KSFloppy {
    img_path_on_host: PathBuf,
    script_path_on_guest: PathBuf,
    script: String,
}

impl KSFloppy {
    pub fn new(img_path_on_host: &Path,
               script_path_on_guest: &Path,
               script: &str)
               -> Result<KSFloppy> {
        let ks_img_filename = img_path_on_host.file_name().unwrap();
        let ks_script_filename = script_path_on_guest.file_name().unwrap();
        let tmp = Path::new("/tmp");
        let tmp_mount = tmp.join(format!("{}.d", ks_img_filename.to_str().unwrap()));
        let tmp_ks_script_path = tmp.join(ks_script_filename);

        let _ = fs::remove_file(img_path_on_host.to_str().unwrap());
        if !Command::new("/usr/bin/dd")
            .args(&["if=/dev/zero",
                    format!("of={}", img_path_on_host.display()).as_str(),
                    "bs=1440K",
                    "count=1"])
            .stderr(Stdio::null())
            .status()
            .expect("failed to execute process")
            .success() {
            return Err("failed to dd".into());
        }
        if !Command::new("/sbin/mkfs")
            .args(&["-F", "-t", "ext2", img_path_on_host.to_str().unwrap()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("failed to execute mkfs ext2")
            .success() {
            return Err("failed to mkfs".into());
        }
        match fs::create_dir(tmp_mount.to_str().unwrap()) {
            Ok(_) => {}
            Err(e) => {
                warn!("cannot create dir `{}`: [{}]",
                      tmp_mount.to_str().unwrap(),
                      e)
            }
        }
        if !Command::new("sudo")
            .args(&["mount",
                    "-o",
                    "loop",
                    img_path_on_host.to_str().unwrap(),
                    tmp_mount.to_str().unwrap()])
            .stderr(Stdio::null())
            .status()
            .expect("failed to execute mount")
            .success() {
            return Err("failed to mount".into());
        }
        let mut f = try!(File::create(&tmp_ks_script_path));
        try!(f.write_all(script.to_string().into_bytes().as_slice()));
        try!(fs::copy(&tmp_ks_script_path,
                      format!("{}/{}",
                              tmp_mount.to_str().unwrap(),
                              ks_script_filename.to_str().unwrap())
                          .as_str()));
        if !Command::new("sudo")
            .args(&["umount", tmp_mount.to_str().unwrap()])
            .stderr(Stdio::null())
            .status()
            .expect("failed to execute umount")
            .success() {
            warn!("failed to umount {}", tmp_mount.to_str().unwrap());
        }
        try!(fs::remove_dir(&tmp_mount));
        Ok(KSFloppy {
            img_path_on_host: img_path_on_host.to_path_buf(),
            script_path_on_guest: script_path_on_guest.to_path_buf(),
            script: script.to_owned(),
        })
    }
}
