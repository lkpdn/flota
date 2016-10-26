use ::distro::*;
use ::exec::session::Session;
use ::flota::config::cluster::host::Host as HostConfig;
use ::util::errors::*;
use ::virt::ResourceBlend;
use ::virt::domain::Domain;
pub mod x86_64;

#[derive(Debug, Clone)]
pub struct OpenSUSE13 {}

impl OpenSUSE13 {
    pub fn new() -> Self {
        OpenSUSE13 {}
    }
}

impl InvasiveAdaption for OpenSUSE13 {
    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn adapt_network_state(&self,
                           host: &HostConfig,
                           sess: &Session,
                           domain: &Domain,
                           template: &ResourceBlend)
                           -> Result<()> {
        // XXX: surely not Ok.
        Ok(())
    }
}

impl UnattendedInstallation for OpenSUSE13 {
    fn unattended_script(&self, params: &UnattendedInstallationParams) -> String {
        format!("
        <?xml version='1.0'?>
        <!DOCTYPE profile>
        <!-- Adapted from https://github.com/digital-wonderland/packer-templates -->
        <profile xmlns='http://www.suse.com/1.0/yast2ns' xmlns:config='http://www.suse.com/1.0/configns'>
          <general>
            <mode>
              <confirm config:type='boolean'>false</confirm>
              <forceboot config:type='boolean'>true</forceboot>
              <final_reboot config:type='boolean'>true</final_reboot>
            </mode>
          </general>
          <keyboard>
            <keymap>english-us</keymap>
          </keyboard>
          <language>
            <language>en_US</language>
            <languages>en_US</languages>
          </language>
          <partitioning config:type='list'>
            <drive>
              <device>/dev/sda</device>
              <initialize config:type='boolean'>true</initialize>
              <partitions config:type='list'>
                <partition>
                  <label>boot</label>
                  <mount>/boot</mount>
                  <mountby config:type='symbol'>label</mountby>
                  <partition_type>primary</partition_type>
                  <size>200M</size>
                </partition>
                <partition>
                  <label>system</label>
                  <lvm_group>system</lvm_group>
                  <partition_type>primary</partition_type>
                  <size>max</size>
                </partition>
              </partitions>
              <use>all</use>
            </drive>
            <drive>
              <device>/dev/system</device>
              <initialize config:type='boolean'>true</initialize>
              <is_lvm_vg config:type='boolean'>true</is_lvm_vg>
              <partitions config:type='list'>
                <partition>
                  <label>swap</label>
                  <mountby config:type='symbol'>label</mountby>
                  <filesystem config:type='symbol'>swap</filesystem>
                  <lv_name>swap</lv_name>
                  <mount>swap</mount>
                  <size>500M</size>
                </partition>
                <partition>
                  <label>root</label>
                  <mountby config:type='symbol'>label</mountby>
                  <filesystem config:type='symbol'>ext4</filesystem>
                  <lv_name>root</lv_name>
                  <mount>/</mount>
                  <size>max</size>
                </partition>
              </partitions>
              <pesize>4M</pesize>
              <type config:type='symbol'>CT_LVM</type>
              <use>all</use>
            </drive>
          </partitioning>
          <bootloader>
            <loader_type>grub2</loader_type>
          </bootloader>
          <networking>
            <ipv6 config:type='boolean'>false</ipv6>
            <keep_install_network config:type='boolean'>true</keep_install_network>
            <dns>
              <dhcp_hostname config:type='boolean'>true</dhcp_hostname>
              <dhcp_resolv config:type='boolean'>true</dhcp_resolv>
              <domain>local</domain>
              <hostname>linux</hostname>
            </dns>
            <interfaces config:type='list'>
              <interface>
                <bootproto>dhcp</bootproto>
                <device>eth0</device>
                <startmode>onboot</startmode>
              </interface>
            </interfaces>
          </networking>
          <firewall>
            <enable_firewall config:type='boolean'>false</enable_firewall>
            <start_firewall config:type='boolean'>false</start_firewall>
          </firewall>
          <software>
            <image/>
            <do_online_update config:type='boolean'>true</do_online_update>
            <kernel>kernel-default</kernel>
            <patterns config:type='list'>
              <pattern>enhanced_base</pattern>
              <pattern>sw_management</pattern>
              <pattern>yast2_basis</pattern>
            </patterns>
            <packages config:type='list'>
              <package>kernel-default</package>
              <package>glibc-locale</package>
              <package>grub2</package>
              <package>ntp</package>
              <package>sudo</package>
            </packages>
          </software>
          <runlevel>
            <default>3</default>
            <services config:type='list'>
              <service>
                <service_name>ntp</service_name>
                <service_status>enable</service_status>
              </service>
              <service>
                <service_name>sshd</service_name>
                <service_status>enable</service_status>
              </service>
            </services>
          </runlevel>
          <users config:type='list'>
            <user>
              <user_password>root</user_password>
              <username>root</username>
            </user>
          </users>
          <scripts>
            <post-scripts config:type='list'>
              <script>
                <filename>post.sh</filename>
                <interpreter>shell</interpreter>
                <source><![CDATA[
#!/bin/sh
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
]]>
                </source>
              </script>
            </chroot-scripts>
          </scripts>
          <kdump>
            <add_crash_kernel config:type='boolean'>false</add_crash_kernel>
          </kdump>
        </profile>
        ",
        mgmt_user = params.mgmt_user_name,
        mgmt_user_ssh_pubkey = params.mgmt_user_ssh_pubkey)
    }
}
