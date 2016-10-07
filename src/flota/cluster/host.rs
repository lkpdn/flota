use std::ops::Deref;
use std::sync::Arc;
use ::exec::session;
use ::exec::session::*;
use ::exec::session::ssh::SessSeedSsh;
use ::flota::config;
use ::flota::template;
use ::util::errors::*;
use ::util::update_etc_hosts;
use ::virt::domain::*;
use ::virt::network::*;
use ::virt::storage::volume::*;

#[derive(Debug)]
pub struct Host<'a> {
    pub domain: Domain,
    pub template: Arc<template::Template<'a>>,
}

impl<'a> Host<'a> {
    pub fn new(host: &config::cluster::Host, template: &Arc<template::Template<'a>>)
        -> Result<Self>
    {
        // make sure networks are all available.
        // all but mgmt subnet are without dhcp functionality
        // so third arg for Network::ensure_default's is false.
        for ref interface in host.interfaces.iter() {
            let br = interface.ip.nth_sibling(1);
            Network::ensure_default(template.resources.conn(), &br, false);
        }

        // vol backed by template's external one
        let path_disk = &template.path_disk;
        let vol = Volume::create_descendant(&host.hostname,
                                            template.resources.pool().as_ref().unwrap(),
                                            &path_disk);

        // create
        let dom = match Domain::boot_with_root_vol(template.resources.conn(),
                                                   &host.hostname,
                                                   &vol,
                                                   host.interfaces
                                                       .iter()
                                                       .map(|v| (v.dev.clone(), v.ip.clone()))
                                                       .collect(),
                                                   template.resources.network()) {
            Ok(x) => x,
            Err(e) => {
                error!("{}", e);
                return Err(format!("failed to create Host: {}", host.hostname).into());
            }
        };

        // get mgmt interface's ip address
        let mgmt_ip = dom.get_ip_in_network(template.resources.network().unwrap()).unwrap();

        // if session seed type is ssh, we update ip
        // because we had not known what management ip it would have.
        let mut seeds = template.session_seeds.clone();
        for mut seed in seeds.iter_mut() {
            if seed.seed_type() == SeedType::Ssh {
                seed.as_mut_any()
                    .downcast_mut::<SessSeedSsh>()
                    .map(|s| s.override_ip(&mgmt_ip));
            }
        }

        let session = session::try_spawn(&seeds, vec![SeedType::Ssh]).unwrap();
        try!(template.distro.deref().adapt_network_state(
                &host, unsafe { &*Box::into_raw(session) }, &dom, &template.resources));

        // update host-side /etc/hosts
        for interface in host.interfaces.iter() {
            try!(update_etc_hosts(None,
                                  &interface.ip,
                                  &host.hostname));
        }

        // solo pre-tests --> tests --> post-tests
        for tests in vec![
            &host.solo_pre_tests,
            &host.solo_tests,
            &host.solo_post_tests
        ].iter() {
            for one_exec in tests.iter() {
                if let Some(seed_type) = SeedType::from_exec_type(&one_exec.exec_type) {
                    if let Some(ref seed) = seeds.iter().find(|s| s.seed_type() == seed_type) {
                        let sess = seed.spawn().unwrap();
                        match sess.exec(&one_exec.command) {
                            Ok(ret) => {
                                info!("exit status: {}", ret.status);
                                info!("stdout: {}", ret.stdout);
                                info!("stderr: {}", ret.stderr);
                            },
                            Err(e) => {
                                error!("{}", e);
                            }
                        }
                    }
                } else { panic!("would not panic") }
            }
        }

        Ok(Host {
            domain: dom,
            template: template.clone(),
        })
    }
}
