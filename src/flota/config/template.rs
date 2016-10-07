use std::path::PathBuf;
use std::sync::Arc;
use toml;
use ::consts::*;
use ::util::errors::*;
use ::util::url::Url;
use super::setting::Setting;

#[allow(dead_code)]
enum UnattendedInstallation {
    Kickstart(String),
    KickstartFile(PathBuf),
}

#[derive(Debug, Clone, RustcEncodable, PartialEq, Eq, Hash)]
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
        let distro = unfold!(tml, "distro", String, optional);
        let iso = unfold!(tml, "iso", Url, optional);
        let iso_md5sum = unfold!(tml, "iso_md5sum", Url, optional);
        let vmlinuz = unfold!(tml, "vmlinuz", Url, optional);
        let initrd = unfold!(tml, "initrd", Url, optional);
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

#[derive(Debug, Clone, RustcEncodable, PartialEq, Eq, Hash)]
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
    /// SSH public key path
    pub mgmt_user_ssh_public_key: PathBuf,
    /// Arc for global setting
    pub setting: Arc<Setting>,
}

impl Template {
    pub fn from_toml(val: &toml::Value, setting: &Arc<Setting>) -> Result<Self> {
        let name = unfold!(val, "name", String);
        let arch = unfold!(val, "arch", String);
        let ingredient = match Ingredient::distinguish(val) {
            Ok(ing) => ing,
            Err(_) => panic!("would not panic!"),
        };
        let ks = unfold!(val, "ks", String, optional);
        let mgmt_user = unfold!(val, "mgmt_user", String, optional,
                                format!("admin_{}", *PROGNAME));
        let mgmt_user_ssh_private_key = unfold!(
            val, "mgmt_user_ssh_private_key", PathBuf, optional,
            PathBuf::from(format!("/home/{}/.ssh/id_rsa", mgmt_user)));
        let mgmt_user_ssh_public_key = unfold!(
            val, "mgmt_user_ssh_public_key", PathBuf, optional,
            PathBuf::from(format!("/home/{}/.ssh/id_rsa.pub", mgmt_user)));
        Ok(Template {
            name: name,
            arch: arch,
            ingredient: ingredient,
            ks: ks,
            mgmt_user: mgmt_user,
            mgmt_user_ssh_private_key: mgmt_user_ssh_private_key,
            mgmt_user_ssh_public_key: mgmt_user_ssh_public_key,
            setting: setting.clone(),
        })
    }
}
