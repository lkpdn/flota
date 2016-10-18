use rusted_cypher::graph::GraphClient;
use std::sync::Arc;
use ::exec::{ExecResult, Output};
use ::exec::session::SeedType;
use ::exec::session::ssh::SessSeedSsh;
use ::flota::{config, Cypherable};
use ::flota::entity::template;
use ::flota::entity::host::Host;
use ::flota::test::Cause;
use ::util::errors::*;

pub mod watch;
use self::watch::WatchPointPerception;

pub struct Manager {}

impl Manager {
    fn pin_cluster_watchpoints(cluster: &config::cluster::Cluster)
                               -> Result<()> {
        // prepare and start transaction
        let graph = GraphClient::connect(::NEO4J_ENDPOINT).unwrap();
        let mut transaction = graph.cypher().transaction();
        transaction.add_statement("MATCH (n: TRANSACTION) RETURN n");
        let (mut transaction, _) = transaction.begin().unwrap();

        // update perception linked-lists
        for ref watchpoint in &cluster.watchpoints {
            let current_perception = WatchPointPerception::new(watchpoint);
            try!(save_child_ll!(&mut transaction, watchpoint, current_perception, "IS_RESULT_OF")
                                .map(|_| ()));
        }

        // commit transaction
        transaction.commit().unwrap();
        Ok(())
    }
    fn cause_of_next_cluster_run(cluster: &config::cluster::Cluster)
                                 -> Result<Vec<Cause>> {
        if let Ok(true) = cluster.is_first_run() {
            try!(Self::pin_cluster_watchpoints(cluster));
            return Ok(vec![ Cause::FirstRun ])
        }

        // if we perceive some watchpoint state have changed since last run,
        // it needs to re-run test.
        let mut causes = vec![];
        // prepare and start transaction
        let graph = GraphClient::connect(::NEO4J_ENDPOINT).unwrap();
        let mut transaction = graph.cypher().transaction();
        transaction.add_statement("MATCH (n: TRANSACTION) RETURN n");
        let (mut transaction, _) = transaction.begin().unwrap();
        for ref watchpoint in &cluster.watchpoints {
            let current_perception = WatchPointPerception::new(watchpoint);
            match is_tail!(watchpoint, current_perception) {
                Ok(true) => {},
                _ => {
                    // XXX:
                    try!(save_child_ll!(&mut transaction, watchpoint, &current_perception,
                                        "IS_RESULT_OF").map(|_| ()));
                    causes.push(Cause::WatchPoint { ident: current_perception });
                },
            }
        }
        try!(transaction.commit());
        Ok(causes)
    }
    pub fn run_host_test(config: &config::cluster::host::Host,
                         host: &Host,
                         causes: &Vec<Cause>) -> Result<()> {
        // XXX: duplicate code
        let mgmt_ip = host.domain.ip_in_network(host.template.resources.network().unwrap())
                                 .unwrap();
        let mut seeds = host.template.session_seeds.clone();
        for mut seed in seeds.iter_mut() {
            if seed.seed_type() == SeedType::Ssh {
                seed.as_mut_any()
                    .downcast_mut::<SessSeedSsh>()
                    .map(|s| s.override_ip(&mgmt_ip));
            }
        }

        for tests in vec![
            &config.solo_pre_tests,
            &config.solo_tests,
            &config.solo_post_tests
        ].iter() {
            for one_exec in tests.iter() {
                if let Some(seed_type) = SeedType::from_exec_type(&one_exec.exec_type) {
                    if let Some(ref seed) = seeds.iter().find(|s| s.seed_type() == seed_type) {
                        let sess = seed.spawn().unwrap();
                        let expected = Output {
                            stdout: one_exec.expect_stdout.clone(),
                            stderr: one_exec.expect_stderr.clone(),
                            status: one_exec.expect_status.clone(),
                        };
                        match sess.exec(&one_exec.command) {
                            Ok(ret) => {
                                info!("{}", ret);
                                let passed = ret.satisfy(&expected);
                                let result = ExecResult {
                                    host: config.hostname.clone(),
                                    command: one_exec.command.clone(),
                                    expected: expected,
                                    result: ret.clone(),
                                    passed: passed,
                                };
                                // prepare and start transaction
                                let graph = GraphClient::connect(::NEO4J_ENDPOINT).unwrap();
                                let mut transaction = graph.cypher().transaction();
                                transaction.add_statement("MATCH (n: TRANSACTION) RETURN n");
                                let (mut transaction, _) = transaction.begin().unwrap();

                                try!(save_child_ll!(&mut transaction, one_exec,
                                                    result, "IS_RESULT_OF")
                                     .map(|_| ()));
                                for cause in causes.iter() {
                                    if let Cause::WatchPoint { ref ident } = *cause {
                                        try!(save_child_ll!(&mut transaction, ident,
                                                            result, "DUE_TO")
                                            .map(|_| ()));
                                    }
                                }
                                // commit transaction
                                try!(transaction.commit());
                            },
                            Err(e) => {
                                error!("{}", e);
                            }
                        }
                    }
                } else { panic!("would not panic") }
            }
        }
        Ok(())
    }
    pub fn run_cluster_test(cluster: &config::cluster::Cluster,
                            hosts: &Vec<Host>,
                            causes: &Vec<Cause>) -> Result<()> {
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
                            let sess = {
                                // if session seed type is ssh, we update ip
                                // because we had not known what management ip it would have.
                                if seed_type == SeedType::Ssh {
                                    let mut seed_updated = seed.clone();
                                    let mgmt_ip = host.domain
                                        .ip_in_network(host.template.resources.network().unwrap())
                                        .unwrap();
                                    seed_updated.as_mut_any()
                                                .downcast_mut::<SessSeedSsh>()
                                                .map(|s| s.override_ip(&mgmt_ip));
                                    seed_updated.spawn().unwrap()
                                } else {
                                    seed.spawn().unwrap()
                                }
                            };
                            let expected = Output {
                                stdout: one_exec.expect_stdout.clone(),
                                stderr: one_exec.expect_stderr.clone(),
                                status: one_exec.expect_status.clone(),
                            };
                            match sess.exec(&one_exec.command) {
                                Ok(ref ret) => {
                                    info!("{}", ret);
                                    let passed = ret.satisfy(&expected);
                                    let result = ExecResult {
                                        host: match one_exec.host {
                                            Some(ref hostname) => { hostname.clone() },
                                            None => { unreachable!() },
                                        },
                                        command: one_exec.command.clone(),
                                        expected: expected,
                                        result: ret.clone(),
                                        passed: passed,
                                    };
                                    // prepare and start transaction
                                    let graph = GraphClient::connect(::NEO4J_ENDPOINT).unwrap();
                                    let mut transaction = graph.cypher().transaction();
                                    transaction.add_statement("MATCH (n: TRANSACTION) RETURN n");
                                    let (mut transaction, _) = transaction.begin().unwrap();

                                    try!(save_child_ll!(&mut transaction, one_exec,
                                                        result, "IS_RESULT_OF")
                                        .map(|_| ()));
                                    for cause in causes.iter() {
                                        if let Cause::WatchPoint { ref ident } = *cause {
                                            try!(save_child_ll!(&mut transaction, ident,
                                                                result, "DUE_TO")
                                                 .map(|_| ()));
                                        }
                                    }

                                    // commit transaction
                                    try!(transaction.commit());
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
        Ok(())
    }
    pub fn run_cluster<'a>(cluster: &config::cluster::Cluster,
                           templates: &Vec<Arc<template::Template<'a>>>)
                       -> Result<bool> {
        let causes = try!(Manager::cause_of_next_cluster_run(&cluster));
        if causes.len() == 0 {
            return Ok(false)
        }
        let mut hosts = Vec::new();
        for host_config in cluster.hosts.iter() {
            // search for a template matched to the host
            let template = match templates.iter().find(
                |&t| t.name == host_config.template.name) {
                Some(v) => v,
                None => continue,
            };
            match Host::new(host_config, &template) {
                Ok(host) => {
                    if let Ok(_) = Manager::run_host_test(host_config,
                                                          &host,
                                                          &causes) {
                        // for cluster tests later
                        hosts.push(host);
                    } else {
                        panic!("would not panic")
                    }
                },
                Err(e) => {
                    error!("failed to create host error: {}", e);
                    return Err(e.into())
                },
            }
        }
        if let Ok(_) = Manager::run_cluster_test(cluster,
                                                 &hosts,
                                                 &causes) {
            // all done. shutdown hosts
            for host in hosts.iter() {
                try!(host.shutdown());
            }
        } else {
            panic!("would not panic")
        }
        Ok(true)
    }
}
