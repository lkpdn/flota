use ::distro;
use ::exec::session::Session;
use ::flota::config::cluster::Host as HostConfig;
use ::util::errors::*;
use ::virt::ResourceBlend;
use ::virt::domain::Domain;
pub mod x86_64;

pub trait CentOS6 {}

impl<T: CentOS6> distro::InvasiveAdaption for T {
    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn adapt_network_state(&self,
                           host: &HostConfig,
                           sess: &Session,
                           domain: &Domain,
                           template: &ResourceBlend)
                           -> Result<()> {
        try!(sess.exec(format!("\
                       sudo sed -i 's/^HOSTNAME=.*$/HOSTNAME={}/' /etc/sysconfig/network;\
                       sudo grep -qE '^127.0.0.1   {}' /etc/hosts || sudo sed -i \
                       '1i\\127.0.0.1   {}' /etc/hosts;sudo hostname {}",
                       host.hostname,
                       host.hostname,
                       host.hostname,
                       host.hostname).as_str()));
        for interface in &host.interfaces {
            if let Some(mac) = domain.get_mac_of_ip(&interface.ip) {
                let cfg = format!("\
                            DEVICE={}\n\
                            BOOTPROTO=none\n\
                            HWADDR={}\n\
                            IPADDR={}\n\
                            NETMASK={}\n\
                            GATEWAY={}\n\
                            IPV6INIT=\"no\"\n\
                            MTU=\"1500\"\n\
                            NM_CONTROLLED=\"no\"\n\
                            ONBOOT=\"yes\"\n\
                            TYPE=\"Ethernet\"",
                    interface.dev,
                    mac,
                    interface.ip.ip(),
                    interface.ip.mask(),
                    interface.ip.nth_sibling(1));
                try!(sess.exec(format!("\
                               cat <<\"EOF\" | sudo tee \
                               /etc/sysconfig/network-scripts/ifcfg-{}\n{}\nEOF\n\n",
                               interface.dev,
                               cfg).as_str()));
            }
        }

        // get mac address which belongs to the default network, i.e. mgmt interface mac.
        if let Some(mac) = domain.get_mac_of_if_in_network(
            match template.network() {
                Some(ref n) => n.name().to_owned(),
                None => panic!("a"),
            }
        ) {
            let cfg = format!("\
                        DEVICE=eth999\n\
                        BOOTPROTO=dhcp\n\
                        PERSISTENT_DHCLIENT=1\n\
                        HWADDR={}\n\
                        IPV6INIT=\"no\"\n\
                        MTU=\"1500\"\n\
                        NM_CONTROLLED=\"yes\"\n\
                        ONBOOT=\"yes\"\n\
                        TYPE=\"Ethernet\"",
                mac);
            try!(sess.exec(format!("
                           echo -e \"{}\" | sudo tee \
                           /etc/sysconfig/network-scripts/ifcfg-eth999",
                           cfg).as_str()));
        }

        // XXX: one of the ugliest part human being ever seen.
        // the reason iproute2 are used instead of network service utility or
        // direct ifconfig is that in the previous part we changed ifcfgs online.
        let _pray = sess.exec(r#"sudo nohup sh -c '
             ls /sys/class/net | xargs -i ip l set dev {} down;
             cat /dev/null > /etc/udev/rules.d/70-persistent-net.rules;
             lspci -v | sed '\''/^$/{x;/Ethernet/{s/^.*modules: \(.*\)\n*.*$/\1/;s/,//;s/ /\n/;p;};d};H;$!d;'\'' |
             sort | uniq | xargs -i sh -c "sudo modprobe -r {}; sudo modprobe {}"
             udevadm trigger --attr-match=subsystem=net;\
             ls /sys/class/net | xargs -i ip l set dev {} up;'"#);
        Ok(())
    }
}
