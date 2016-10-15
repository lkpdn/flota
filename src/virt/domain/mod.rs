extern crate xml;
use std::ffi::{CStr, CString};
use std::path::{Path, PathBuf};
use xml::{Event, Parser};
use ::libvirt::*;
use ::util::errors::*;
use ::util::ipv4::IPv4;
use ::virt::conn::Conn;
use ::virt::storage::volume::Volume;
use ::virt::network::Network;

pub mod snapshot;

resource!(Domain, virDomain);

impl Domain {
    pub fn volume_paths(&self) -> Vec<PathBuf> {
        let desc = self.xml().unwrap();
        let mut p = Parser::new();
        p.feed_str(&desc);
        let mut in_tag = false;
        let mut sources = Vec::new();
        for event in p {
            match event.unwrap() {
                Event::ElementStart(ref tag) if tag.name == "disk".to_string() => {
                    in_tag = true;
                }
                Event::ElementStart(ref tag) if in_tag && tag.name == "source".to_string() => {
                    match tag.attributes.get(&("file".to_string(), None)) {
                        Some(source) => {
                            sources.push(Path::new(source).to_path_buf());
                        }
                        _ => {}
                    }
                }
                Event::ElementEnd(ref tag) if in_tag && tag.name == "disk".to_string() => {
                    in_tag = false;
                }
                _ => (),
            }
        }
        sources
    }
    pub fn networks(&self) -> Vec<&Network> {
        vec![]
    }
    // TODO: secondary ip
    pub fn ip_in_network(&self, network: &Network) -> Result<IPv4> {
        if let Some(mgmt_mac) = self.mac_in_network(network.name().to_owned()) {
            if let Some(ip) = network.ip_linked_to_mac(&mgmt_mac, Some(20), Some(3)) {
                Ok(ip)
            } else {
                Err(format!("cannot detect ip of domain `{}` in network `{}`",
                            self.name(), network.name()).into())
            }
        } else {
            Err(format!("no interface found in network `{}` on domain `{}`",
                        network.name(), self.name()).into())
        }
    }
    pub fn mac_in_network(&self, network: String) -> Option<String> {
        let desc = self.xml().unwrap();
        let mut p = Parser::new();
        p.feed_str(&desc);
        let mut in_tag = false;
        let mut in_nw = false;
        let mut mac = None;
        for event in p {
            match event.unwrap() {
                Event::ElementStart(ref tag) if tag.name == "interface".to_string() => {
                    in_tag = true;
                }
                Event::ElementStart(ref tag) if in_tag && tag.name == "mac".to_string() => {
                    match tag.attributes.get(&("address".to_string(), None)) {
                        Some(mc) => mac = Some(mc.clone()),
                        _ => {}
                    }
                }
                Event::ElementStart(ref tag) if in_tag && tag.name == "source".to_string() => {
                    match tag.attributes.get(&("network".to_string(), None)) {
                        Some(nw) if *nw == network => in_nw = true,
                        _ => {}
                    }
                }
                Event::ElementEnd(ref tag) if in_tag && tag.name == "interface".to_string() => {
                    in_tag = false;
                    if !in_nw {
                        mac = None
                    }
                }
                _ => (),
            }
        }
        mac
    }
    pub fn mac_of_ip(&self, ip: &IPv4) -> Option<String> {
        use xml::{Event, Parser};
        let desc = self.xml().unwrap();
        let mut p = Parser::new();
        p.feed_str(&desc);
        let mut in_tag = false;
        let mut of_ip = false;
        let mut mac = None;
        for event in p {
            match event.unwrap() {
                Event::ElementStart(ref tag) if tag.name == "interface".to_string() => {
                    in_tag = true;
                }
                Event::ElementStart(ref tag) if in_tag && tag.name == "mac".to_string() => {
                    match tag.attributes.get(&("address".to_string(), None)) {
                        Some(mc) => mac = Some(mc.clone()),
                        _ => {}
                    }
                }
                Event::ElementStart(ref tag) if in_tag && tag.name == "ip".to_string() => {
                    match (tag.attributes.get(&("address".to_string(), None)),
                           tag.attributes.get(&("prefix".to_string(), None))) {
                        (Some(addr), Some(prefix)) if *addr == ip.ip() &&
                                                      *prefix == ip.mask_bit().to_string() => {
                            of_ip = true
                        }
                        _ => {}
                    }
                }
                Event::ElementEnd(ref tag) if in_tag && tag.name == "interface".to_string() => {
                    if of_ip {
                        return mac;
                    } else {
                        in_tag = false;
                        mac = None
                    }
                }
                _ => (),
            }
        }
        mac
    }
    pub fn find(name: &str, conn: &Conn) -> Option<Domain> {
        match unsafe {
            virDomainLookupByName(conn.raw(), CString::new(name.to_owned()).unwrap().as_ptr())
        } {
            p if !p.is_null() => Some(Domain { raw: p }),
            _ => None,
        }
    }
    pub fn boot_with_root_vol(conn: &Conn,
                              hostname: &str,
                              vol: &Volume,
                              interfaces: Vec<(String, IPv4)>,
                              default_network: Option<&Network>)
                              -> Result<Domain> {
        unsafe {
            let dom = match virDomainLookupByName(conn.raw(),
                                                  CString::new(hostname.to_owned())
                                                      .unwrap()
                                                      .as_ptr()) {
                p if !p.is_null() => {
                    if virDomainIsActive(p) == 1 {
                        if virDomainReboot(p, 0) != 0 {
                            return Err("domain already exists but failed to reboot".into())
                        }
                        p
                    } else if virDomainCreate(p) == 0 {
                        p
                    } else {
                        return Err("domain already exists but failed to start".into())
                    }
                },
                _ => {
                    let mem_mb = 768;
                    let mut x = xE!("domain", type => "kvm");
                    x.tag(xE!("name"))
                        .text(hostname.to_owned().into());
                    x.tag(xE!("memory", unit => "KiB"))
                        .text((mem_mb * 1024).to_string().into());
                    x.tag(xE!("vcpu", placement => "static"))
                        .text("1".into());

                    // os
                    let mut x_os = xE!("os");
                    x_os.tag(xE!("type", arch => "x86_64"))
                        .text("hvm".into());
                    x_os.tag(xE!("boot", dev => "hd"));
                    x.tag(x_os);

                    x.tag(xE!("features"))
                        .tag_stay(xE!("acpi"))
                        .tag_stay(xE!("apic"));
                    x.tag(xE!("clock", offset => "localtime"));

                    // devices
                    let mut x_dev = xE!("devices");
                    // base disk
                    x_dev.tag(xE!("disk", type => "volume", device => "disk"))
                        .tag_stay(xE!("driver", name => "qemu", type => "qed"))
                        .tag_stay(xE!("source",
                          pool => vol.pool().name(),
                          volume => vol.name()
                        ))
                        .tag_stay(xE!("target", dev => "hda", bus => "ide"));
                    for (_dev, o_ip) in interfaces {
                        let mut br_ip = o_ip.nw_addr();
                        br_ip.incr_node_id().unwrap();
                        let nw = Network::ensure_default(conn, &br_ip, false);
                        let ip = o_ip.ip();
                        let prefix = o_ip.mask_bit().to_string();
                        // it's assumed here that whatever dev name is chosen
                        // for dummy nics on host side. whatever l2 address is
                        // chosen in guest side, it wouldn't do much harm.
                        x_dev.tag(xE!("interface", type => "network"))
                          .tag_stay(xE!("source", network => nw.name()))
                          .tag_stay(xE!("ip",
                            address => ip.as_str(),
                            prefix => prefix.as_str()
                          ));
                    }
                    // XXX: default network interface just for digging
                    // to adjust network interfaces after installation.
                    // too lazy to create valid libguestfs rust binding
                    // or to mimic its behaviour.
                    if default_network.is_some() {
                        x_dev.tag(xE!("interface", type => "network"))
                            .tag_stay(xE!("start", mode => "onboot"))
                            .tag_stay(xE!("source", network => default_network.unwrap().name()));
                    }
                    // console
                    x_dev.tag(xE!("console", type => "pty"))
                        .tag_stay(xE!("target", type => "serial", port => "0"));

                    x.tag(x_dev);
                    virDomainDefineXML(conn.raw(), CString::new(format!("{}", x)).unwrap().as_ptr())
                }
            };
            let mut state = -1 as i32;
            let mut reason = -1 as i32;
            if virDomainGetState(dom, &mut state, &mut reason, 0) < 0 {
                return Err(format!("failed to get state of domain: {}", hostname).into());
            }
            match state {
                s if s == virDomainState::VIR_DOMAIN_RUNNING as i32 => {
                    info!("domain of hostname {} already running", hostname);
                    return Ok(Domain { raw: dom });
                }
                _ => {}
            }
            if virDomainCreate(dom) < 0 {
                Err("failed to create domain".into())
            } else {
                Ok(Domain { raw: dom })
            }
        }
    }
    pub fn destroy(&self) -> Result<()> {
        if unsafe { virDomainIsActive(self.raw()) } == 1 &&
           unsafe { virDomainDestroy(self.raw()) } < 0 {
            Err("failed to destroy".into())
        } else {
            Ok(())
        }
    }
    pub fn undefine(&self) -> Result<()> {
        if unsafe { virDomainUndefine(self.raw()) } < 0 {
            Err("failed to undefine".into())
        } else {
            Ok(())
        }
    }
    pub fn delete(&self) -> Result<()> {
        try!(self.destroy());
        self.undefine()
    }
}
