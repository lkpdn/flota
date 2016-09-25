use std::ptr;
use std::slice;
use ::libvirt::*;
use ::util::errors::*;
use ::virt::domain::Domain;

#[derive(Debug, Clone)]
pub struct Conn {
    raw: virConnectPtr,
}

impl Drop for Conn {
    fn drop(&mut self) {
        unsafe {
            virConnectClose(self.raw());
        }
    }
}

impl Conn {
    pub fn new(uri: &str) -> Self {
        unsafe {
            let raw = virConnectOpen(rawCharPtr!(uri));
            if raw.is_null() {
                panic!("failed to open connection to {}", uri);
            }
            virConnSetErrorFunc(raw, ptr::null_mut(), Some(defaultVirtErrorFunc));
            Conn { raw: raw }
        }
    }
    pub fn raw(&self) -> virConnectPtr {
        self.raw
    }
    pub fn domains(&self, flags: u32) -> Result<Vec<Domain>> {
        let mut domains: *mut virDomainPtr = ptr::null_mut();
        match unsafe { virConnectListAllDomains(self.raw(), &mut domains, flags) } {
            -1 => Err("failed to list domains".into()),
            n => {
                Ok(unsafe {
                    slice::from_raw_parts(domains, n as usize)
                        .iter()
                        .filter(|d| !d.is_null())
                        .map(|d| Domain { raw: &mut **d })
                        .collect::<Vec<Domain>>()
                })
            }
        }
    }
}

#[allow(non_snake_case)]
unsafe extern "C" fn defaultVirtErrorFunc(_data: *mut ::std::os::raw::c_void, _err: virErrorPtr) {}

#[cfg(test)]
mod tests {
    use super::Conn;

    #[test]
    fn test_conn_new() {
        Conn::new("test:///default");
        assert!(true);
    }
}
