use ssh2;
use std::net::TcpStream;
use std::ops::Deref;
use std::path::Path;
use ::distro;
use ::flota::config;
use ::flota::template;
use ::util::errors::*;
use ::util::ipv4::IPv4;
use ::util::{update_etc_hosts, update_known_host};
use ::virt::domain::*;
use ::virt::network::*;
use ::virt::storage::volume::*;

#[derive(Debug)]
pub struct Host {
    pub domain: Domain,
    pub mgmt_ip: IPv4,
    pub mgmt_user: String,
}

impl Host {
    pub fn new<'a, D: ?Sized>(host: &config::Host, template: &template::Template<D>) -> Result<Self>
        where D: distro::Base + distro::InvasiveAdaption
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

        // management user
        let mgmt_user = if let Some(ref u) = host.mgmt_user {
            u.to_owned()
        } else {
            template.mgmt_user.clone()
        };
        let mgmt_user_ssh_private_key = if let Some(ref p) = host.mgmt_user_ssh_private_key {
            &p
        } else {
            &template.mgmt_user_ssh_private_key
        };

        // implicit setups
        let tcp = TcpStream::connect(format!("{}:22", &mgmt_ip.ip()).as_str()).unwrap();
        let mut sess = ssh2::Session::new().unwrap();
        sess.handshake(&tcp).unwrap();
        sess.userauth_pubkey_file(&mgmt_user, None, &mgmt_user_ssh_private_key, None).unwrap();
        sess.set_timeout(3000);
        sess.set_blocking(true);
        sess.set_allow_sigpipe(true);
        try!(update_known_host(&sess, &mgmt_ip.ip()));
        debug!("trying to ssh -i {} -l {} {}",
               mgmt_user_ssh_private_key.to_str().unwrap(),
               mgmt_user,
               &mgmt_ip.ip());

        // invasive adaptions
        try!(template.distro.deref().adapt_network_state(&host, &sess, &dom, &template.resources));

        // update host-side /etc/hosts
        for interface in host.interfaces.iter() {
            try!(update_etc_hosts(None,
                                  &interface.ip,
                                  &host.hostname));
        }

        // solo pre-tests

        // solo tests

        // solo post-tests

        Ok(Host {
            domain: dom,
            mgmt_ip: mgmt_ip,
            mgmt_user: mgmt_user,
        })
    }
}
