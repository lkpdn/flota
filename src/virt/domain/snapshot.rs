extern crate xml;
use std::ffi::CString;
use time::now;
use ::libvirt::*;
use ::util::errors::*;
use ::virt::domain::Domain;

pub struct DomainSnapshot {
    raw_ptr: virDomainSnapshotPtr,
    disk_path: String,
    ram_path: String,
}

impl Drop for DomainSnapshot {
    fn drop(&mut self) {
        unsafe {
            if virDomainSnapshotFree(self.raw_ptr) < 0 {
                error!("{}", "failed to free DomainSnapshot.raw_ptr");
            }
        }
    }
}

impl DomainSnapshot {
    pub fn ensure(dom: &Domain, pool_root: &str, name: Option<&str>) -> DomainSnapshot {
        let dom_name = dom.name();
        let now = now();
        let snapshot_name = match name {
            Some(nm) => format!("{}", nm),
            None => format!("{}.{}", dom_name, now.rfc3339()),
        };
        let snapshot_disk_name = match name {
            Some(nm) => format!("{}.qcow2", nm),
            None => format!("{}.{}.qcow2", dom_name, now.rfc3339()),
        };
        let snapshot_ram_name = match name {
            Some(nm) => format!("{}.RAM", nm),
            None => format!("{}.{}.RAM", dom_name, now.rfc3339()),
        };
        unsafe {
            let snapshot_ptr =
                match virDomainSnapshotLookupByName(dom.raw(),
                                                    CString::new(snapshot_name.clone())
                                                        .unwrap()
                                                        .as_ptr(),
                                                    0) {
                    p if !p.is_null() => p,
                    _ => {
                        let mut x = xE!("domainsnapshot");
                        x.tag(xE!("name"))
                            .text(snapshot_name.to_owned().into());
                        x.tag(xE!("state"))
                            .text("running".into());
                        x.tag(xE!("memory", snapshot => "external",
                      file => format!("{}/{}", pool_root, snapshot_ram_name)));
                        x.tag(xE!("disks"))
                            .tag(xE!("disk",
                       name => "hda",
                       snapshot => "external",
                       type => "file"))
                            .tag_stay(xE!("driver",
                       type => "qcow2"))
                            .tag_stay(xE!("source",
                       file => format!("{}/{}", pool_root, snapshot_disk_name)));
                        debug!("{}", x);
                        virDomainSnapshotCreateXML(dom.raw(), rawCharPtr!(x), 0)
                    }
                };
            DomainSnapshot {
                raw_ptr: snapshot_ptr,
                disk_path: String::from(format!("{}/{}", pool_root, snapshot_disk_name)),
                ram_path: String::from(format!("{}/{}", pool_root, snapshot_ram_name)),
            }
        }
    }
    pub fn disk_path(&self) -> String {
        self.disk_path.clone()
    }
    pub fn ram_path(&self) -> String {
        self.ram_path.clone()
    }
    pub fn delete(&self) -> Result<()> {
        unsafe {
            if virDomainSnapshotDelete(self.raw_ptr, 0) < 0 {
                Err("failed to delete snapshot".into())
            } else {
                Ok(())
            }
        }
    }
}
