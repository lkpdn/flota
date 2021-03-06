use ::flota::config;
use ::distro;
use ::exec::session::*;
use ::exec::session::ssh::SessSeedSsh;
use ::util::errors::*;
use ::virt::*;
use ::virt::domain::snapshot::*;

#[derive(Clone, Debug)]
pub struct Template<'a> {
    pub name: String,
    pub path_disk: String,
    pub resources: &'a ResourceBlend<'a>,
    pub session_seeds: SessionSeeds,
    pub distro: Box<distro::Distro>,
}

impl<'a> Template<'a> {
    pub fn new(resources: &'a ResourceBlend,
               template: &config::template::Template,
               distro: Box<distro::Distro>)
               -> Result<Self> {

        let (dom, volume) = distro.build_image(None,
                         resources.conn(),
                         resources.pool().as_ref().unwrap(),
                         resources.network().as_ref().unwrap(),
                         &template)
            .unwrap();

        let pool_root = resources.pool().as_ref().map(|ref p| p.target_path().unwrap()).unwrap();
        let snapshot_name = format!("{}.001", dom.name());
        let snapshot = DomainSnapshot::ensure(&dom, &pool_root, Some(&snapshot_name));

        try!(dom.destroy());

        // XXX: now its okay to have only one choice, maybe not?
        let session_seeds = vec![
            SessSeedSsh::new(
                &template.mgmt_user,
                None, 22,
                template.mgmt_user_ssh_private_key.as_path()
            )
        ];

        Ok(Template {
            name: template.name.to_owned(),
            path_disk: volume.path().to_owned(),
            resources: resources,
            session_seeds: session_seeds,
            distro: distro,
        })
    }
    pub fn destroy(&mut self) -> () {
        unimplemented!()
    }
}
