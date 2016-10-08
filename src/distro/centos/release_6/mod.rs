use ::distro::*;
use ::exec::session::Session;
use ::flota::config::cluster::Host as HostConfig;
use ::util::errors::*;
use ::virt::ResourceBlend;
use ::virt::domain::Domain;
pub mod x86_64;

pub trait CentOS6 {}

impl<T: CentOS6> InvasiveAdaption for T {
    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn adapt_network_state(&self,
                           host: &HostConfig,
                           sess: &Session,
                           domain: &Domain,
                           template: &ResourceBlend)
                           -> Result<()> {
        try!(sess.exec(format!("\
                       sudo sed -i 's/^HOSTNAME=.*$/HOSTNAME={host}/' /etc/sysconfig/network;\
                       sudo grep -qE '^127.0.0.1   {host}' /etc/hosts || sudo sed -i \
                       '1i\\127.0.0.1   {host}' /etc/hosts;sudo hostname {host}",
                       host = host.hostname).as_str()));
        for interface in &host.interfaces {
            if let Some(mac) = domain.get_mac_of_ip(&interface.ip) {
                let cfg = format!("\
                            DEVICE={dev}\n\
                            BOOTPROTO=none\n\
                            HWADDR={mac}\n\
                            IPADDR={ip}\n\
                            NETMASK={mask}\n\
                            GATEWAY={gw}\n\
                            IPV6INIT=\"no\"\n\
                            MTU=\"1500\"\n\
                            NM_CONTROLLED=\"no\"\n\
                            ONBOOT=\"yes\"\n\
                            TYPE=\"Ethernet\"",
                    dev = interface.dev,
                    mac = mac,
                    ip = interface.ip.ip(),
                    mask = interface.ip.mask(),
                    gw = interface.ip.nth_sibling(1));
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

        // FIXME: to make 100% sure connection persiste (whether or not with ssh2).
        try!(sess.exec(r#"sudo nohup sh -c '
             ls /sys/class/net | xargs -i ip l set dev {} down;
             cat /dev/null > /etc/udev/rules.d/70-persistent-net.rules;
             lspci -v | sed '\''/^$/{x;/Ethernet/{s/^.*modules: \(.*\)\n*.*$/\1/;s/,//;s/ /\n/;p;};d};H;$!d;'\'' |
             sort | uniq | xargs -i sh -c "sudo modprobe -r {}; sudo modprobe {}"
             udevadm trigger --attr-match=subsystem=net;\
             ls /sys/class/net | xargs -i ip l set dev {} up;'"#));
        Ok(())
    }
}

impl<T: CentOS6> UnattendedInstallation for T {
    fn unattended_script(&self, params: &UnattendedInstallationParams) -> String {
        format!("
install
cdrom
text
cmdline
skipx
lang en_US.UTF-8
keyboard us
timezone Asia/Tokyo --isUtc
network --activate --bootproto=dhcp --noipv6
zerombr
bootloader --location=mbr
clearpart --all --initlabel
part / --fstype=ext4 --grow --size=1 --asprimary --label=root
rootpw --plaintext password
auth --enableshadow --passalgo=sha512
selinux --disabled
firewall --disabled
firstboot --disabled
repo --name='CentOS' --baseurl='http://mirror.centos.org/centos/6/os/x86_64/'
poweroff
%packages
%end

## post instalattion
## -----------------
%post
/usr/sbin/useradd {mgmt_user} -G wheel -d /home/{mgmt_user} -s /bin/bash
echo {mgmt_user} | passwd --stdin {mgmt_user}

mkdir /home/{mgmt_user}/.ssh
chmod 700 -R /home/{mgmt_user}/.ssh/
cat > /home/{mgmt_user}/.ssh/authorized_keys << EOF
{mgmt_user_ssh_pubkey}
EOF

chmod 600 -R /home/{mgmt_user}/.ssh/authorized_keys
chown {mgmt_user}:{mgmt_user} -R /home/{mgmt_user}/.ssh/

cat >> /etc/sudoers << EOF
### Allow wheel group sudo access ###
%wheel ALL=(ALL) NOPASSWD:ALL
EOF

cat >> /etc/ssh/ssh_config << EOF
StrictHostKeyChecking no
EOF

/bin/sed -i 's/#PermitRootLogin yes/PermitRootLogin no/' /etc/ssh/sshd_config
/sbin/service sshd restart

### Ugly workaround. This template is assumed to be
### used as a backing store for any hosts no matter
### in which network they will reside in. Note that
### we haven't met systemd yet. CentOS6 based here.
echo dhclient >> /etc/rc.local",
        mgmt_user = params.mgmt_user_name,
        mgmt_user_ssh_pubkey = params.mgmt_user_ssh_pubkey)
    }
}
