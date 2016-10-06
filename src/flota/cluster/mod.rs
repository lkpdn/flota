use std::sync::Arc;
use ::exec::session::*;
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

        // pre-tests --> tests --> post-tests
        for tests in vec![
            &cluster.pre_tests,
            &cluster.tests,
            &cluster.post_tests
        ].iter() {
            for one_exec in tests.iter() {
                // XXX: just ugly. help me.
                // XXX: lazy validation might be a bad choice.
                if let Some(host) = hosts.iter().find(|h| Some(h.domain.name().to_string()) == one_exec.host) {
                    if let Some(seed_type) = SeedType::from_exec_type(&one_exec.exec_type) {
                        if let Some(seed) = host.template.session_seeds.iter().find(|s| {
                            s.seed_type() == seed_type
                        }) {
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
                        } else {
                            error!("requested method is not provided of that host");
                        }
                    } else {
                        panic!("would not panic")
                    }
                }
            }
        }

        Ok(Cluster {
            name: cluster.name.clone(),
            hosts: hosts,
        })
    }
}
