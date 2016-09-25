extern crate xml;
use std::ffi::CStr;
use ::util::errors::*;

#[macro_export]
macro_rules! resource {
( $name:ident, $prefix:ident ) => {
#[derive(Debug, Clone)]
// Never own it even if we'd be the very person.
pub struct $name {
    pub raw: concat_idents!($prefix, Ptr),
}
impl Drop for $name {
    fn drop(&mut self) {
        let f = concat_idents!($prefix, Free);
        if !self.raw.is_null() &&
            unsafe { f(self.raw()) } < 0 {
                error!("failed to drop raw ptr in $name");
        }
    }
}
impl $name {
    pub fn name(&self) -> &str {
        let f = concat_idents!($prefix, GetName);
        unsafe {
            CStr::from_ptr(f(self.raw)).to_str().unwrap()
        }
    }
    pub unsafe fn raw(&self) -> concat_idents!($prefix, Ptr) {
        self.raw
    }
    pub fn xml(&self) -> Option<String> {
        unsafe {
            let f = concat_idents!($prefix, GetXMLDesc);
            let ptr = f(self.raw(), 0);
            if ptr.is_null() {
                None
            } else {
                Some(CStr::from_ptr(ptr).to_str().unwrap().to_owned())
            }
        }
    }
}
}}

pub mod conn;
pub mod domain;
pub mod network;
pub mod storage;

use self::conn::Conn;
use self::domain::Domain;
use self::network::Network;
use self::storage::pool::StoragePool;
use self::storage::volume::Volume;

#[derive(Debug, Clone)]
pub struct ResourceBlend<'a> {
    conn: &'a Conn,
    domain: Option<&'a Domain>,
    network: Option<&'a Network>,
    pool: Option<&'a StoragePool>,
    volume: Option<&'a Volume>,
}
impl<'a> ResourceBlend<'a> {
    pub fn new(conn: &'a Conn) -> Self {
        ResourceBlend {
            conn: conn,
            domain: None,
            network: None,
            pool: None,
            volume: None,
        }
    }
    pub fn put_domain(&mut self, domain: &'a Domain) {
        self.domain = Some(domain);
    }
    pub fn put_network(&mut self, network: &'a Network) {
        self.network = Some(network);
    }
    pub fn put_pool(&mut self, pool: &'a StoragePool) {
        self.pool = Some(pool);
    }
    pub fn put_volume(&mut self, volume: &'a Volume) {
        self.volume = Some(volume);
    }
    pub fn conn(&self) -> &'a Conn {
        self.conn
    }
    pub fn domain(&self) -> Option<&'a Domain> {
        self.domain
    }
    pub fn network(&self) -> Option<&'a Network> {
        self.network
    }
    pub fn pool(&self) -> Option<&'a StoragePool> {
        self.pool
    }
    pub fn volume(&self) -> Option<&'a Volume> {
        self.volume
    }
}

pub fn clean_associated(rb: &ResourceBlend) -> Result<()> {
    let domains = try!(rb.conn().domains(0));
    for domain in domains.iter() {
        let vol_paths = domain.volume_paths();
        let networks = domain.networks();
        try!(domain.delete());
        for network in networks {
            match network.name() {
                "default" => (),
                _ => {
                    // intentionally skip error. it will be
                    // deleted at the last moment all domains
                    // utilising it were deleted.
                    let _ = network.delete();
                }
            }
        }
        for vol_path in vol_paths.iter() {
            if let Some(volume) = Volume::from_path(rb.conn(), &vol_path) {
                volume.delete();
            }
        }
        // sadly enough dummy nic will not be deleted after its virbr is
        // deleted, so try to do it here. We also skip error for the same
        // reasons aforementioned on network deletion part.
    }
    Ok(())
}
