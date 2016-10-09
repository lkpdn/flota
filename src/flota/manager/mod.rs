use git2::{Direction, ErrorCode, Repository};
use serde_json;
use std::collections::HashMap;
use std::sync::Arc;
use ::exec::{ExecResult, Output};
use ::exec::session::SeedType;
use ::exec::session::ssh::SessSeedSsh;
use ::flota::{config, template};
use ::flota::config::cluster::WatchPoint;
use ::flota::cluster::host::Host;
use ::store::{TestResultStore, unqlite_backed};
use ::util::errors::*;
use ::util::md5sum::calc_md5;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterTestKey {
    cluster: config::cluster::Cluster,
    watchpoints: Vec<(config::cluster::WatchPoint, HashMap<String, Vec<u8>>)>,
}

impl ClusterTestKey {
    pub fn new(cluster: &config::cluster::Cluster) -> Self {
        let mut watchpoints = Vec::new();
        for watchpoint in cluster.watchpoints.iter() {
            match watchpoint {
                &WatchPoint::Git {
                    ref uri,
                    ref remote,
                    ref refs,
                    ref checkout_dir,
                } => {
                    let url = uri.as_str();
                    let repo = match Repository::clone(url, checkout_dir) {
                        Ok(repo) => { repo },
                        Err(ref e) if e.code() == ErrorCode::Exists => {
                            Repository::open(checkout_dir).expect(
                                format!("failed to open {:?}", checkout_dir).as_str())
                        },
                        Err(e) => panic!("failed to clone: {}", e),
                    };
                    let mut rem = repo.find_remote(remote.as_str()).unwrap();
                    rem.connect(Direction::Fetch).expect(format!(
                            "failed to connect to {}", remote).as_str());
                    let ref_commit_ids = rem
                        .list()
                        .unwrap()
                        .iter()
                        .map(|head| (head.name().to_owned(), head.oid().as_bytes().to_vec()))
                        .filter(|r1| {
                            if &refs[..] == &[ "*" ] {
                                true
                            } else {
                                refs.iter().find(|r2| **r2 == r1.0).is_some()
                            }
                        })
                        .collect::<HashMap<_, _>>();
                    watchpoints.push((
                        WatchPoint::Git {
                            uri: uri.clone(),
                            remote: remote.clone(),
                            refs: refs.clone(),
                            checkout_dir: checkout_dir.clone(),
                        },
                        ref_commit_ids
                    ))
                },
                &WatchPoint::File {
                    ref path
                } => {
                    let mut map = HashMap::new();
                    map.insert(path.to_str().unwrap().to_owned(),
                               calc_md5(path).unwrap().as_bytes().to_vec());
                    watchpoints.push((
                        WatchPoint::File {
                            path: path.clone(),
                        },
                        map
                    ))
                },
            }
        }
        ClusterTestKey {
            cluster: cluster.clone(),
            watchpoints: watchpoints,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestResult {
    host_test_results: Vec<ExecResult>,
    cluster_test_result: Vec<ExecResult>,
}

impl From<Vec<u8>> for TestResult {
    fn from(v: Vec<u8>) -> Self {
        let buf = String::from_utf8(v).unwrap();
        serde_json::from_str(&buf).unwrap()
    }
}

pub struct Manager {}

impl Manager {
    // judge whether or not to (re-)run tests.
    // if it turns out that we should do so,
    // returns key
    //
    // key: "{config.id}:{cluster.id}:{watchpoints hash}"
    fn needs_rerun_of_cluster(cluster: &config::cluster::Cluster, store: &unqlite_backed::TestResultStore)
                              -> Result<(ClusterTestKey, bool)> {
        let key = ClusterTestKey::new(cluster);
        match store.find(&key) {
            Some(_) => Ok((key, false)),
            None => Ok((key, true)),
        }
    }
    pub fn run_host_test(config: &config::cluster::Host, host: &Host) -> Result<Vec<ExecResult>> {
        // XXX: duplicate code
        let mgmt_ip = host.domain.get_ip_in_network(host.template.resources.network().unwrap()).unwrap();
        let mut seeds = host.template.session_seeds.clone();
        for mut seed in seeds.iter_mut() {
            if seed.seed_type() == SeedType::Ssh {
                seed.as_mut_any()
                    .downcast_mut::<SessSeedSsh>()
                    .map(|s| s.override_ip(&mgmt_ip));
            }
        }

        let mut results = Vec::new();
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
                                results.push(ExecResult {
                                    host: config.hostname.clone(),
                                    command: one_exec.command.clone(),
                                    expected: expected,
                                    result: ret.clone(),
                                    passed: passed,
                                });
                            },
                            Err(e) => {
                                error!("{}", e);
                            }
                        }
                    }
                } else { panic!("would not panic") }
            }
        }
        Ok(results)
    }
    pub fn run_cluster_test(cluster: &config::cluster::Cluster, hosts: &Vec<Host>) -> Result<Vec<ExecResult>> {
        let mut results = Vec::new();
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
                                        .get_ip_in_network(host.template.resources.network().unwrap())
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
                                    results.push(ExecResult {
                                        host: match one_exec.host {
                                            Some(ref hostname) => { hostname.clone() },
                                            None => { unreachable!() },
                                        },
                                        command: one_exec.command.clone(),
                                        expected: expected,
                                        result: ret.clone(),
                                        passed: passed,
                                    });
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
        Ok(results)
    }
    pub fn run_cluster<'a>(cluster: &config::cluster::Cluster,
                           templates: &Vec<Arc<template::Template<'a>>>)
                       -> Result<bool> {
        let store = unqlite_backed::TestResultStore::new(
            ::consts::DATA_DIR.join("fuga").as_path());
        match Manager::needs_rerun_of_cluster(&cluster, &store) {
            Ok((_, false)) => {
                return Ok(false)
            },
            Err(e) => {
                error!("{}", e);
            },
            Ok((key, true)) => {
                let mut hosts = Vec::new();
                let mut host_test_results = Vec::new();
                for host in cluster.hosts.iter() {
                    // search for a template matched to the host
                    let template = match templates.iter().find(|&t| t.name == host.template.name) {
                        Some(v) => v,
                        None => continue,
                    };
                    match Host::new(host, &template) {
                        Ok(h) => {
                            let result = Manager::run_host_test(host, &h);
                            host_test_results.append(&mut result.unwrap());
                            hosts.push(h);
                        },
                        Err(e) => {
                            error!("failed to create host error: {}", e);
                            return Err(e.into())
                        },
                    }
                }
                if let Ok(ref result) = Manager::run_cluster_test(&cluster, &hosts) {
                    let test_result = TestResult {
                        host_test_results: host_test_results,
                        cluster_test_result: result.clone(),
                    };
                    try!(store.set(&key, &test_result));
                }
            }
        }
        Ok(true)
    }
}
