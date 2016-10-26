use libc;
use nix::sys::signal::kill;
use std::ffi::CString;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::Command;
use std::ptr;
use std::time::Instant;
use libvirt::*;
use url::Url;
use xml;

use ::distro;
use ::distro::{UnattendedInstallation, UnattendedInstallationParams};
use ::distro::opensuse::YastFlash;
use ::distro::opensuse::release_13::OpenSUSE13;
use ::flota::config;
use ::util::*;
use ::util::errors::*;
use ::util::notify::tailf_background;
use ::virt::conn::Conn;
use ::virt::domain::Domain;
use ::virt::network::Network;
use ::virt::storage::pool::StoragePool;
use ::virt::storage::volume::Volume;

lazy_static! {
    pub static ref ISO: Url = Url::parse(
        "http://download.opensuse.org/distribution/13.2/iso/openSUSE-13.2-NET-x86_64.iso").unwrap();
    pub static ref ISO_MD5SUM: Url = Url::parse(
        "http://download.opensuse.org/distribution/13.2/iso/openSUSE-13.2-NET-x86_64.iso.md5").unwrap();
    pub static ref VMLINUZ: Url = Url::parse(
        "http://download.opensuse.org/distribution/13.2/repo/oss/boot/x86_64/loader/linux").unwrap();
    pub static ref INITRD: Url = Url::parse(
        "http://download.opensuse.org/distribution/13.2/repo/oss/boot/x86_64/loader/initrd").unwrap();
}
const IDENT: &'static str = "opensuse13-x86_64";

// unit: MiB
const MEM_ON_INSTALL: i32 = 1280;

pub trait UrlExt {
    fn last_segment(&self) -> Option<String>;
}

impl UrlExt for Url {
    fn last_segment(&self) -> Option<String> {
        self.path_segments().map(|c| c.collect::<Vec<_>>().last().unwrap().to_string())
    }
}

fn download_iso(local_path: &Path) -> Result<()> {
    download_file(&ISO, local_path)
}

fn download_iso_md5checked(local_path: &Path) -> Result<()> {
    if ! local_path.exists() {
        let iso_filename = ISO.last_segment().unwrap();
        let iso_md5sum_filename = ISO_MD5SUM.last_segment().unwrap();
        let tmp = Path::new("/tmp");
        let md5_local_pathbuf = tmp.join(&iso_md5sum_filename);
        download_file(&ISO_MD5SUM, &md5_local_pathbuf).unwrap();
        let mut md5_file = File::open(md5_local_pathbuf.to_str().unwrap()).unwrap();
        let mut buffer = String::new();
        md5_file.read_to_string(&mut buffer).unwrap();
        let md5s = buffer.lines()
            // the md5sum.txt is assumed to have multiple entries
            // for each type of iso files.
            .filter(|l| l.ends_with(&iso_filename))
            .map(|l| l.split(' ').collect::<Vec<&str>>()[0])
            .collect::<Vec<_>>();
        if md5s.len() == 1 {
            md5sum::download_file(&ISO, local_path, md5s[0])
        } else {
            warn!("skip md5 checking");
            download_iso(local_path)
        }
    } else { Ok(()) }
}

fn download_vmlinuz(local_path: &Path) -> Result<()> {
    if ! local_path.exists() {
        download_file(&VMLINUZ, local_path)
    } else { Ok(()) }
}

fn download_initrd(local_path: &Path) -> Result<()> {
    if ! local_path.exists() {
        download_file(&INITRD, local_path)
    } else { Ok(()) }
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone)]
pub struct OpenSUSE13_x8664;

impl OpenSUSE13 for OpenSUSE13_x8664 {}

impl distro::Base for OpenSUSE13_x8664 {
    fn distro(&self) -> String {
        "OpenSUSE".to_string()
    }
    fn release(&self) -> String {
        "13".to_string()
    }
    fn arch(&self) -> String {
        "x86_64".to_string()
    }
    fn build_image(&self,
                   name: Option<&str>,
                   conn: &Conn,
                   storage_pool: &StoragePool,
                   network: &Network,
                   template: &config::template::Template)
                   -> Result<(Domain, Volume)> {
        let dom_name = match name {
            Some(nm) => nm,
            None => IDENT,
        };
        let pool_name = storage_pool.name();

        // 1. ensure volume
        let volume = match Volume::ensure(&storage_pool, format!("{}.000", dom_name).as_str()) {
            Ok(v) => v,
            Err(e) => {
                error!("{}", e);
                return Err(e);
            }
        };
        let ctrl_path_on_host = Path::new("/tmp/autoinst.img");
        let ctrl_path_on_guest = Path::new("/autoinst.xml");
        let ctrl_script = match template.ks {
            Some(ref s) => { s.clone() },
            None => {
                let ssh_priv_key = {
                    let mut buf = String::new();
                    let mut f = try!(File::open(template.mgmt_user_ssh_private_key.as_os_str()));
                    try!(f.read_to_string(&mut buf));
                    buf
                };
                let ssh_pub_key = {
                    let mut buf = String::new();
                    let mut f = try!(File::open(template.mgmt_user_ssh_public_key.as_os_str()));
                    try!(f.read_to_string(&mut buf));
                    buf
                };
                let params = UnattendedInstallationParams {
                    mgmt_user_name: template.mgmt_user.clone(),
                    mgmt_user_ssh_pubkey: ssh_pub_key,
                    mgmt_user_ssh_privkey: ssh_priv_key,
                };
                self.unattended_script(&params)
            }
        };
        let yast_flash = try!(YastFlash::new(&ctrl_path_on_host,
                                             &ctrl_path_on_guest,
                                             &ctrl_script));
        // 2. download installation files
        let tmp = Path::new("/tmp");
        let iso_local_path = tmp.join(ISO.last_segment().unwrap());
        let vmlinuz_local_path = tmp.join(VMLINUZ.last_segment().unwrap());
        let initrd_local_path = tmp.join(INITRD.last_segment().unwrap());
        download_iso_md5checked(&iso_local_path).expect("failed to download");
        download_vmlinuz(&vmlinuz_local_path).expect("failed to download");
        download_initrd(&initrd_local_path).expect("failed to download");

        // 2. install
        unsafe {
            match virDomainLookupByName(conn.raw(), rawCharPtr!(dom_name)) {
                p if !p.is_null() => {
                    // XXX: if its xml is just what you want s.t. boot priority
                    // of some install media being highest let alone other points
                    // you care, we should keep it defined and running if it's active
                    if virDomainIsActive(p) == 1 && virDomainDestroy(p) < 0 {
                        warn!("failed to destroy {}", dom_name);
                    }
                }
                _ => {
                    let mut x = xE!("domain", type => "kvm");
                    x.tag(xE!("name"))
                        .text(dom_name.to_owned().into());
                    x.tag(xE!("memory", unit => "MiB"))
                        .text((MEM_ON_INSTALL).to_string().into());
                    x.tag(xE!("vcpu", placement => "static"))
                        .text("1".into());

                    // os
                    let mut x_os = xE!("os");
                    x_os.tag(xE!("type", arch => "x86_64"))
                        .text("hvm".into());
                    x_os.tag(xE!("kernel"))
                        .text(vmlinuz_local_path.to_str().unwrap().into());
                    x_os.tag(xE!("initrd"))
                        .text(initrd_local_path.to_str().unwrap().into());
                    x_os.tag(xE!("cmdline"))
                        .text(
                         "fbcon=map:99 text console=ttyS0 \
                          autoyast=default xhci_hcd.quirks=262144".into());
                    x_os.tag_stay(xE!("boot", dev => "hd"))
                        .tag_stay(xE!("boot", dev => "cdrom"));
                    x.tag(x_os);

                    x.tag(xE!("features"))
                        .tag_stay(xE!("acpi"))
                        .tag_stay(xE!("apic"));
                    x.tag(xE!("clock", offset => "localtime"));

                    let mut x_dev = xE!("devices");
                    // base disk
                    x_dev.tag(xE!("disk", type => "volume", device => "disk"))
                      .tag_stay(xE!("driver", name => "qemu", type => "qed"))
                      .tag_stay(xE!("source", pool => pool_name, volume => volume.name().clone()))
                      .tag_stay(xE!("target", dev => "hda", bus => "ide"));
                    // install media
                    x_dev.tag(xE!("disk", type => "file", device => "cdrom"))
                        .tag_stay(xE!("driver", name => "qemu", type => "raw"))
                        .tag_stay(xE!("source", file => iso_local_path.to_str().unwrap()))
                        .tag_stay(xE!("target", dev => "hdb", bus => "ide"));
                    // autoyast flash
                    x_dev.tag(xE!("disk", type => "file", device =>  "disk"))
                        .tag_stay(xE!("source",
                        file => yast_flash.img_path_on_host.to_str().unwrap()))
                        .tag_stay(xE!("target", dev => "usb"))
                        .tag_stay(xE!("read_only"));
                    // default network interface
                    x_dev.tag(xE!("interface", type => "network"))
                        .tag(xE!("source", network => network.name()));
                    // console
                    let var_qemu = Path::new("/var/lib/libvirt/qemu");
                    let log_file_serial0 = var_qemu.join(format!("{}-serial0.log", dom_name));
                    x_dev.tag(xE!("console", type => "file"))
                        .tag_stay(xE!("target", type => "serial", port => "0"))
                        .tag_stay(xE!("source", path => log_file_serial0.to_str().unwrap()));

                    x.tag(x_dev);
                    debug!("start direct installation:\n{}", x);
                    let dom = virDomainDefineXML(conn.raw(), rawCharPtr!(x));

                    if virEventRegisterDefaultImpl() < 0 {
                        panic!("cannot register default impl: domain `{}`", dom_name);
                    }
                    if virConnectDomainEventRegisterAny(conn.raw(),
                      dom,
                      virDomainEventID::VIR_DOMAIN_EVENT_ID_LIFECYCLE as i32,
                      Some(emptyDomLifeHandler),
                      ptr::null_mut(), None) < 0 {
                        panic!("cannot register empty handler: domain `{}`", dom_name);
                    }

                    if virDomainCreate(dom) < 0 {
                        panic!("cannot create domain: {}", dom_name);
                    }
                    if virEventRunDefaultImpl() < 0 {
                        panic!("cannot run default impl: domain `{}`", dom_name);
                    }

                    // prep stdouting serial0 log file
                    let child = tailf_background(log_file_serial0.as_path()).unwrap();
                    let _timeout_secs = 60 * 10;
                    let _start_time = Instant::now();
                    loop {
                        let mut state: i32 = 0;
                        let mut reason: i32 = 0;
                        if virDomainGetState(dom, &mut state, &mut reason, 0) < 0 {
                            panic!("cannot get state");
                        }
                        match state {
                            s if s == virDomainState::VIR_DOMAIN_RUNNING as i32 ||
                                 s == virDomainState::VIR_DOMAIN_SHUTDOWN as i32 => {}
                            s if s == virDomainState::VIR_DOMAIN_SHUTOFF as i32 => break,
                            s => panic!("unexpected state: {}", s),
                        }
                    }
                    cmd!("reset");
                    kill(child, libc::SIGKILL).expect("kill failed");

                    // undefine
                    if virDomainUndefine(dom) < 0 {
                        return Err(format!("cannot undfine domain: {}",
                                           dom_name)
                            .into());
                    }
                }
            }

            // 3. adjust so as to be utilised as a template
            let new_dom = match virDomainLookupByName(conn.raw(), rawCharPtr!(dom_name)) {
                p if !p.is_null() => p,
                _ => {
                    let mem_mb = 768;
                    let mut x = xE!("domain", type => "kvm");
                    x.tag(xE!("name"))
                        .text(dom_name.into());
                    x.tag(xE!("memory", unit => "KiB"))
                        .text((mem_mb * 1024).to_string().into());
                    x.tag(xE!("vcpu", placement => "static"))
                        .text("1".into());

                    // os
                    let mut x_os = xE!("os");
                    x_os.tag(xE!("type", arch => "x86_64"))
                        .text("hvm".into());
                    x_os.tag(xE!("boot", dev => "hd"));
                    x.tag(x_os);

                    x.tag(xE!("features"))
                        .tag_stay(xE!("acpi"))
                        .tag_stay(xE!("apic"));
                    x.tag(xE!("clock", offset => "localtime"));

                    // devices
                    let mut x_dev = xE!("devices");
                    // base disk
                    x_dev.tag(xE!("disk", type => "volume", device => "disk"))
                        .tag_stay(xE!("driver", name => "qemu", type => "qed"))
                        .tag_stay(xE!("source", pool => pool_name, volume => volume.name()))
                        .tag_stay(xE!("target", dev => "hda", bus => "ide"));
                    // default network interface
                    x_dev.tag(xE!("interface", type => "user"))
                        .tag_stay(xE!("source", network => network.name()));
                    // console
                    x_dev.tag(xE!("console", type => "pty"))
                        .tag_stay(xE!("target", type => "serial", port => "0"));

                    x.tag(x_dev);
                    virDomainDefineXML(conn.raw(), CString::new(format!("{}", x)).unwrap().as_ptr())
                }
            };
            let _ = virDomainCreate(new_dom);
            Ok((Domain { raw: new_dom }, volume))
        }
    }
}

#[allow(non_snake_case)]
unsafe extern "C" fn emptyDomLifeHandler(_conn: virConnectPtr,
                                         _dom: virDomainPtr,
                                         _opaque: *mut ::std::os::raw::c_void) {
}
