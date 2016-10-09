use std::sync::Arc;
use ::flota::config;
use ::flota::template;
use ::util::errors::*;

pub mod host;
use self::host::Host;

#[derive(Debug)]
pub struct Cluster<'a> {
    pub name: String,
    pub hosts: Vec<Host<'a>>,
}

impl<'a> Cluster<'a> {
    pub fn new(cluster: &config::cluster::Cluster,
               templates: &Vec<Arc<template::Template<'a>>>)
                              -> Result<Self>
    {
        let mut hosts = Vec::new();
        for host in cluster.hosts.iter() {
            // search for a template matched to the host
            let template = match templates.iter().find(|&t| t.name == host.template.name) {
                Some(v) => v,
                None => continue,
            };
            match Host::new(host, &template) {
                Ok(h) => hosts.push(h),
                Err(e) => {
                    error!("{}", e);
                    // who needs a cluster missing any of its defined hosts
                    return Err(format!("failed to create Cluster: {}", cluster.name).into());
                }
            }
        }

        Ok(Cluster {
            name: cluster.name.clone(),
            hosts: hosts,
        })
    }
}
