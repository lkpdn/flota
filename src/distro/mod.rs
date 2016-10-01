use ::flota::config::Host as HostConfig;
use ::exec::session::Session;
use ::util::errors::*;
use ::virt::ResourceBlend;
use ::virt::conn::Conn;
use ::virt::domain::Domain;
use ::virt::network::Network;
use ::virt::storage::pool::StoragePool;
use ::virt::storage::volume::Volume;

pub trait Distro: Base + InvasiveAdaption {}
impl<T: Base + InvasiveAdaption> Distro for T {}

pub trait Base {
    fn distro(&self) -> String;
    fn release(&self) -> String;
    fn arch(&self) -> String;
    fn build_image(&self,
                   name: Option<&str>,
                   conn: &Conn,
                   storage_pool: &StoragePool,
                   network: &Network,
                   unattended_script: &str)
                   -> Result<(Domain, Volume)>;
}

#[allow(unused_variables)]
pub trait InvasiveAdaption {
    // Guest OS chosen is likely to be stateful in the sense that to change
    // from some network configuration to another one, some burdonsome operations
    // s.t. editing files, reloading some daemons, cleaning up some residues, etc.
    // might be required. If that's the case, With an implementation of this
    // you realize "host" config via "sess" session on "domain" which was
    // generated in "template" resources environment.
    fn adapt_network_state(&self,
                           host: &HostConfig,
                           sess: &Session,
                           domain: &Domain,
                           template: &ResourceBlend)
                           -> Result<()>;
}

// off-the-shelf distros
pub mod centos;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Distros;
impl Distros {
    pub fn search(ident: &str, arch: &str) -> Box<Distro> {
        match (ident, arch) {
            ("centos6", "x86_64") => Box::new(centos::release_6::x86_64::CentOS6_x8664 {}),
            _ => unimplemented!(),
        }
    }
}
