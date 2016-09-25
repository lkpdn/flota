use std::ffi::CStr;
use std::fs;
use std::ptr;
use std::slice;
use xml;
use ::libvirt::*;
use ::util::errors::*;
use ::virt::conn::Conn;
use ::virt::storage::volume::Volume;

resource!(StoragePool, virStoragePool);

impl StoragePool {
    pub fn volumes(&self) -> Result<Vec<Volume>> {
        let mut volumes: *mut virStorageVolPtr = ptr::null_mut();
        match unsafe { virStoragePoolListAllVolumes(self.raw(), &mut volumes, 0) } {
            -1 => Err(format!("failed to list volumes of pool: {}", self.name()).into()),
            n => {
                Ok(unsafe {
                    slice::from_raw_parts(volumes, n as usize)
                        .iter()
                        .filter(|v| !v.is_null())
                        .map(|v| Volume { raw: &mut **v })
                        .collect::<Vec<Volume>>()
                })
            }
        }
    }
    pub fn ensure(conn: &Conn, name: &str, pool_root: &str) -> Result<StoragePool> {
        let _ = fs::create_dir(pool_root);
        unsafe {
            let raw = match virStoragePoolLookupByName(conn.raw(), rawCharPtr!(name)) {
                p if !p.is_null() => p,
                _ => {
                    let capacity_gb = 18;
                    let mut x = xE!("pool", type => "dir");
                    x.tag(xE!("name"))
                        .text(name.into());
                    x.tag(xE!("capacity", unit => "bytes"))
                        .text((capacity_gb * 2u64.pow(30)).to_string().into());
                    x.tag(xE!("target"))
                        .tag(xE!("path"))
                        .text(pool_root.into());
                    debug!("{}", x);
                    let defined = virStoragePoolDefineXML(conn.raw(), rawCharPtr!(x), 0);
                    if defined.is_null() {
                        return Err(format!(
                          "cannot create definition of pool: {}", name)
                            .into());
                    }
                    defined
                }
            };

            let mut info = virStoragePoolInfo::default();
            if virStoragePoolGetInfo(raw, &mut info) < 0 {
                return Err(format!(
                  "cannto get info of pool: {}", name)
                    .into());
            }
            match info.state {
                s if s == virStoragePoolState::VIR_STORAGE_POOL_INACTIVE as i32 => {
                    if virStoragePoolCreate(raw, 0) < 0 {
                        return Err(format!(
                          "cannot start pool: {}", name)
                            .into());
                    }
                }
                s if s == virStoragePoolState::VIR_STORAGE_POOL_RUNNING as i32 => {}
                s if s == virStoragePoolState::VIR_STORAGE_POOL_DEGRADED as i32 => {
                    return Err(format!(
                      "storage pool degraded: {}", name)
                        .into())
                }
                s if s == virStoragePoolState::VIR_STORAGE_POOL_INACCESSIBLE as i32 => {
                    return Err(format!(
                      "storage pool inaccessible: {}", name)
                        .into())
                }
                _ => unreachable!(),
            }
            let mut autostart = 0;
            if virStoragePoolGetAutostart(raw, &mut autostart) < 0 {
                return Err(format!(
                  "could not get the value of the autostart flag for pool: {}",
                  name
                )
                    .into());
            }
            if autostart == 0 && virStoragePoolSetAutostart(raw, 1) < 0 {
                return Err(format!(
                  "could not set autostart flag value for the storage pool: {}",
                  name
                )
                    .into());
            }
            Ok(StoragePool { raw: raw })
        }
    }
    pub fn target_path(&self) -> Result<String> {
        unsafe {
            let xml_desc = virStoragePoolGetXMLDesc(self.raw, 0);
            let elem: xml::Element = CStr::from_ptr(xml_desc)
                .to_str()
                .unwrap()
                .parse()
                .unwrap();
            Ok(elem.get_children("target", None)
                        .collect::<Vec<_>>()[0]
                    .get_children("path", None)
                    .collect::<Vec<_>>()[0]
                .content_str())
        }
    }
    #[allow(dead_code)]
    fn destroy(&self) -> Result<()> {
        if unsafe { virStoragePoolDestroy(self.raw) } < 0 {
            Err(format!(
              "cannot destroy storage pool: {}", self.name()
            )
                .into())
        } else {
            Ok(())
        }
    }
}
