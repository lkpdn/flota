use ::distro;
use ::flota::config;
use ::flota::template;
use ::util::errors::*;

pub mod host;
use self::host::Host;

#[derive(Debug)]
pub struct Cluster {
    pub name: String,
    pub hosts: Vec<Host>,
}

impl Cluster {
    pub fn new<'a, D: ?Sized>(cluster: &config::Cluster,
                              templates: &Vec<template::Template<D>>)
                              -> Result<Self>
        where D: distro::Base + distro::InvasiveAdaption
    {
        let mut res = Vec::new();
        for host in cluster.hosts.iter() {
            // search for a template matched to the host
            let template = match templates.iter().find(|&t| t.name == host.template) {
                Some(v) => v,
                None => continue,
            };
            match Host::new(host, &template) {
                Ok(h) => res.push(h),
                Err(e) => {
                    error!("{}", e);
                    // who needs a cluster missing any of its defined hosts
                    return Err(format!("failed to create Cluster: {}", cluster.name).into());
                }
            }
        }
        Ok(Cluster {
            name: cluster.name.clone(),
            hosts: res,
        })
    }
}
