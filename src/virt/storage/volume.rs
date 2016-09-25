use libvirt::*;
use std::ffi::{CString, CStr};
use std::path::Path;
use xml;
use ::util::errors::*;
use ::virt::conn::Conn;
use ::virt::storage::pool::StoragePool;

resource!(Volume, virStorageVol);

impl Volume {
    pub fn from_path(conn: &Conn, path: &Path) -> Option<Volume> {
        unsafe {
            match virStorageVolLookupByPath(conn.raw(), rawCharPtr!(path.to_str().unwrap())) {
                p if !p.is_null() => {
                    Some(Volume { raw: p })
                },
                _ => { None }
            }
        }
    }
    pub fn get_path(&self) -> &str {
        unsafe { CStr::from_ptr(virStorageVolGetPath(self.raw)).to_str().unwrap() }
    }
    pub fn get_pool(&self) -> StoragePool {
        StoragePool { raw: unsafe { virStoragePoolLookupByVolume(self.raw) } }
    }
    pub fn ensure(storage_pool: &StoragePool, name: &str) -> Result<Self> {
        // XXX: make selectable
        let format = "qed";
        let vol_name = format!("{}.{}", name, format);
        unsafe {
            let vol = match virStorageVolLookupByName(storage_pool.raw, rawCharPtr!(vol_name)) {
                p if !p.is_null() => p,
                _ => {
                    let capacity_gb = 9;
                    let pool_target_path = storage_pool.target_path().unwrap();

                    // volume
                    let mut x_vol = xE!("volume", type => "file");
                    x_vol.tag(xE!("name")).text(vol_name.clone().into());
                    x_vol.tag(xE!("capacity", unit => "bytes"))
                        .text((capacity_gb * 2u64.pow(30)).to_string());

                    // volume.target
                    let mut x_vol_target = xE!("target");
                    x_vol_target.tag(xE!("path"))
                        .text(pool_target_path.into());
                    x_vol_target.tag(xE!("format", type => format));

                    x_vol.tag(x_vol_target);
                    let p = virStorageVolCreateXML(storage_pool.raw, rawCharPtr!(x_vol), 0);
                    if p.is_null() {
                        return Err(format!(
                          "cannot create a storage volume: {}", vol_name)
                            .into());
                    }
                    p
                }
            };
            Ok(Volume { raw: vol })
        }
    }
    pub fn create_descendant(name: &str, storage_pool: &StoragePool, path_disk: &str) -> Volume {
        unsafe {
            let vol =
                match virStorageVolLookupByName(storage_pool.raw,
                                                CString::new(name.to_owned()).unwrap().as_ptr()) {
                    p if !p.is_null() => p,
                    _ => {
                        let capacity_gb = 9;
                        let pool_target_path = storage_pool.target_path().unwrap();
                        let default_vol_format = "qed";

                        let mut x_vol = xE!("volume", type => "file");
                        x_vol.tag(xE!("name")).text(name.to_owned().into());
                        x_vol.tag(xE!("capacity", unit => "bytes"))
                            .text((capacity_gb * 2u64.pow(30)).to_string());
                        let mut x_vol_target = xE!("target");
                        x_vol_target.tag(xE!("path"))
                            .text(pool_target_path.into());
                        x_vol_target.tag(xE!("format", type => default_vol_format));
                        x_vol.tag(x_vol_target);
                        let mut x_vol_back = xE!("backingStore");
                        x_vol_back.tag(xE!("path"))
                            .text(path_disk.into());
                        x_vol_back.tag(xE!("format", type => default_vol_format));
                        let mut x_vol_back_perm = xE!("permission");
                        x_vol_back_perm.tag(xE!("owner")).text("107".into());
                        x_vol_back_perm.tag(xE!("group")).text("107".into());
                        x_vol_back_perm.tag(xE!("mode")).text("0744".into());
                        x_vol_back.tag(x_vol_back_perm);
                        x_vol.tag(x_vol_back);
                        virStorageVolCreateXML(storage_pool.raw,
                                               CString::new(format!("{}", x_vol)).unwrap().as_ptr(),
                                               0)
                    }
                };
            Volume { raw: vol }
        }
    }
    pub fn delete(&self) -> Result<()> {
        unsafe {
            if virStorageVolDelete(self.raw, 0) < 0 {
                Err(format!("canno delete vol: {}",
                  self.name())
                    .into())
            } else {
                Ok(())
            }
        }
    }
}
