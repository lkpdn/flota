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
        // N.B. would look nicer if virDomainInterfaceAddress could always be used
        let mgmt_mac = dom.get_mac_of_if_in_network(template.resources
                .network()
                .as_ref()
                .map(|n| n.name().to_owned())
                .unwrap())
            .unwrap();
        debug!("mgmt interface's mac address: {} (domain: {})",
               mgmt_mac,
               &host.hostname);

        // detect an obtained lease trying 20 times with the sleep interval 3 sec.
        let mgmt_ip = match template.resources.network() {
            Some(ref nw) => {
                // retry:20, sleep:3sec
                if let Some(ip) = nw.get_ip_linked_to_mac(&mgmt_mac, Some(20), Some(3)) {
                    debug!("and its ip: {}", ip);
                    ip
                } else {
                    return Err(format!("cannot detect ip on mgmt interface on domain: {}",
                                       dom.name())
                        .into());
                }
            }
            None => panic!("yup"),
        };

        // implicit setups
        let mut seeds = Vec::new();
        for ref mut seed in template.session_seeds.clone().iter() {
            if seed.seed_type() != SeedType::Ssh {
                let s: Box<SessionSeed> = seed.clone();
                seeds.push(s);
                continue;
            }
            let mut new_seed = seed.clone();
            if let Some(ref mut x) = new_seed.as_mut_any().downcast_mut::<SessSeedSsh>() {
                x.ip = Some(mgmt_ip.clone());
            }
            seeds.push(new_seed);
        }

        let session = session::try_spawn(seeds, vec![SeedType::Ssh]).unwrap();
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
                    if let Some(seed) = template.session_seeds.iter().find(|s| s.seed_type() == seed_type) {
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
