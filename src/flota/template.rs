use std::path::PathBuf;
use ::flota::config;
use ::distro;
use ::util::errors::*;
use ::virt::*;
use ::virt::domain::snapshot::*;

#[derive(Debug, Clone)]
pub struct Template<'a, T: distro::Base + ?Sized> {
    pub name: String,
    pub path_disk: String,
    pub resources: &'a ResourceBlend<'a>,
    pub mgmt_user: String,
    pub mgmt_user_ssh_private_key: PathBuf,
    pub distro: Box<T>,
}

impl<'a, T: distro::Base + ?Sized> Template<'a, T> {
    pub fn new(resources: &'a ResourceBlend,
               template: &config::Template,
               distro: Box<T>)
               -> Result<Self> {

        let (dom, volume) = distro.build_image(None,
                         resources.conn(),
                         resources.pool().as_ref().unwrap(),
                         resources.network().as_ref().unwrap(),
                         template.ks.as_ref().unwrap())
            .unwrap();

        let pool_root = resources.pool().as_ref().map(|ref p| p.target_path().unwrap()).unwrap();
        let snapshot_name = format!("{}.001", dom.name());
        let snapshot = DomainSnapshot::ensure(&dom, &pool_root, Some(&snapshot_name));

        try!(dom.destroy());

        Ok(Template {
            name: template.name.to_owned(),
            path_disk: volume.get_path().to_owned(),
            resources: resources,
            mgmt_user: template.mgmt_user.to_owned(),
            mgmt_user_ssh_private_key: template.mgmt_user_ssh_private_key.to_owned(),
            distro: distro,
        })
    }
    pub fn destroy(&mut self) -> () {
        ()
    }
}
