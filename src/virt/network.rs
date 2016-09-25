extern crate xml;
use nix::unistd;
use std::ffi::CStr;
use std::ptr;
use std::slice;
use ::libvirt::*;
use ::util::errors::*;
use ::util::ipv4::IPv4;
use ::virt::conn::Conn;

resource!(Network, virNetwork);

impl Network {
    /// get DHCP leases.
    pub fn get_leases(&self) -> Result<Vec<virNetworkDHCPLease>> {
        let mut leases: *mut virNetworkDHCPLeasePtr = ptr::null_mut();
        match unsafe { virNetworkGetDHCPLeases(self.raw(), ptr::null(), &mut leases, 0) } {
            -1 => Err(format!("failed to get leases info of: {}", self.name()).into()),
            n => {
                Ok(unsafe {
                    slice::from_raw_parts(leases, n as usize)
                        .iter()
                        .filter(|p| !p.is_null())
                        .map(|p| **p)
                        .collect()
                })
            }
        }
    }
    /// get primary ip associated to mac
    pub fn get_ip_linked_to_mac(&self,
                                mac: &str,
                                retry: Option<u32>,
                                sleep: Option<u32>)
                                -> Option<IPv4> {
        let mut retry_count = retry.unwrap_or(0);
        let siesta = sleep.unwrap_or(1);
        loop {
            match self.get_leases() {
                Ok(ref leases) => {
                    for lease in leases.iter() {
                        if let Ok(mc) = unsafe { CStr::from_ptr(lease.mac).to_str() } {
                            if mc != mac {
                                continue;
                            }
                            return Some(IPv4::from_cidr_notation(format!("{}/{}",
                              unsafe { CStr::from_ptr(lease.ipaddr).to_str().unwrap() },
                              lease.prefix
                            )
                                    .as_str())
                                .unwrap());
                        }
                    }
                }
                Err(e) => {
                    error!("{}", e);
                }
            }
            if retry_count == 0 {
                break;
            }
            retry_count -= 1;
            unistd::sleep(siesta);
        }
        None
    }
    /// ensure_default:
    /// @conn: raw connection pointer
    /// @br_ip: borrowed IPv4 obj whose address is bridge ip
    /// @with_dhcp: if true, ip range between the next one after br_ip
    ///             and the one before the end of this network.
    ///             otherwise no ranges will be reserved for dhcp.
    pub fn ensure_default(conn: &Conn, br_ip: &IPv4, with_dhcp: bool) -> Network {
        let dhcp = if with_dhcp == true {
            let mut dhcp_start = br_ip.clone();
            dhcp_start.incr_node_id().unwrap();
            let dhcp_end = br_ip.nth_sibling(-2);
            Some((dhcp_start, dhcp_end))
        } else {
            None
        };
        Network::ensure(conn, &br_ip.hyphenated(), &br_ip.hyphenated(), br_ip, dhcp)
    }
    #[allow(unused_must_use)]
    pub fn ensure(conn: &Conn,
                  br_name: &str,
                  nw_name: &str,
                  br_ipv4: &IPv4,
                  dhcp: Option<(IPv4, IPv4)>)
                  -> Network {
        unsafe {
            let raw = match virNetworkLookupByName(conn.raw(), rawCharPtr!(nw_name)) {
                p if !p.is_null() => p,
                _ => {
                    let mut x_nw = xE!("network");
                    x_nw.tag(xE!("name"))
                        .text(nw_name.into());
                    x_nw.tag(xE!("bridge", name => br_name));
                    x_nw.tag(xE!("forward", mode => "nat"));
                    let mut x_nw_ip = xE!("ip",
                      address => br_ipv4.ip(),
                      netmask => br_ipv4.mask()
                    );
                    match dhcp {
                        Some((dhcp_start, dhcp_end)) => {
                            x_nw_ip.tag(xE!("dhcp"))
                                .tag(xE!("range",
                                start => dhcp_start.ip(),
                                end => dhcp_end.ip()
                              ));
                        }
                        None => (),
                    }
                    x_nw.tag(x_nw_ip);
                    virNetworkDefineXML(conn.raw(), rawCharPtr!(x_nw))
                }
            };
            if virNetworkIsActive(raw) == 0 && virNetworkCreate(raw) < 0 {
                panic!("cannot create network: {}", nw_name);
            }
            let mut autostart = 0;
            if virNetworkGetAutostart(raw, &mut autostart as *mut i32) < 0 {
                panic!("cannot get autostart setting of network: {}", nw_name);
            }
            if autostart == 0 && virNetworkSetAutostart(raw, 1) < 0 {
                panic!("cannot set autostart setting of network: {}", nw_name);
            }
            Network { raw: raw }
        }
    }
    pub fn destroy(&self) -> Result<()> {
        if unsafe { virNetworkIsActive(self.raw) } == 1 &&
           unsafe { virNetworkDestroy(self.raw) } < 0 {
            Err(format!("failed to destroy network: {}", self.name()).into())
        } else {
            Ok(())
        }
    }
    pub fn undefine(&self) -> Result<()> {
        if unsafe { virNetworkUndefine(self.raw) } < 0 {
            Err(format!("failed to undefine network: {}", self.name()).into())
        } else {
            Ok(())
        }
    }
    pub fn delete(&self) -> Result<()> {
        try!(self.destroy());
        self.undefine()
    }
}
