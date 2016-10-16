use nix::unistd::sleep;
use std::ops::Deref;
use std::sync::Arc;
use ::exec::session;
use ::exec::session::*;
use ::exec::session::ssh::SessSeedSsh;
use ::flota::config;
use ::flota::entity::template;
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
    pub fn new(host: &config::cluster::host::Host, template: &Arc<template::Template<'a>>)
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
        // if residue found, delete it.
        match Volume::find(&host.hostname,
                           template.resources.pool().as_ref().unwrap()) {
            Some(vol) => { try!(vol.delete()); },
            None => {},
        }
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
        let mgmt_ip = dom.ip_in_network(template.resources.network().unwrap()).unwrap();

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

        // wait at most 60 seconds until guest-side sshd boots up.
        'try_adaption: for _ in 0..20 {
            match session::try_spawn(&seeds, vec![SeedType::Ssh]) {
                Ok(session) => {
                    if let Err(_) = template.distro.deref()
                        .adapt_network_state(
                            &host, unsafe { &*Box::into_raw(session) },
                            &dom, &template.resources
                        ) {
                        // XXX: if we seem to have failed to adapt network state,
                        //      we try to connect again. max retry count is ten, sleep
                        //      interval is 3sec.
                        'wait_wakeup: for i in 0..10 {
                            match session::try_spawn(&seeds, vec![SeedType::Ssh]) {
                                Err(_) if i >= 9 => { break 'wait_wakeup },
                                Ok(_) => { break 'try_adaption },
                                _ => {
                                    sleep(3);
                                    continue
                                }
                            }
                        }
                        return Err("network adaption failed".into())
                    }
                    break 'try_adaption
                },
                _ => {
                    sleep(3);
                    continue
                }
            }
        }

        // update host-side /etc/hosts
        for interface in host.interfaces.iter() {
            try!(update_etc_hosts(None,
                                  &interface.ip,
                                  &host.hostname));
        }

        Ok(Host {
            domain: dom,
            template: template.clone(),
        })
    }
    pub fn shutdown(&self) -> Result<()> {
        self.domain.destroy()
    }
}
