use ssh2;
use ssh2::{CheckResult, HostKeyType, KnownHostFileKind, KnownHostKeyFormat};
use std::env;
use std::io::prelude::*;
use std::net::TcpStream;
use std::path::Path;
use ::util::errors::*;
use ::util::ipv4::IPv4;
use super::{Return, Session};

pub struct SessSsh {
    inner: ssh2::Session,
}

impl Session for SessSsh {
    fn exec(&self, command: String) -> Result<Return> {
        match self.inner.channel_session() {
            Ok(mut channel) => {
                channel.exec(&command).unwrap();
                let mut stdout = String::new();
                let mut stderr = String::new();
                try!(channel.read_to_string(&mut stdout));
                try!(channel.stderr().read_to_string(&mut stderr));
                Ok(Return {
                    stdout: stdout,
                    stderr: stderr,
                    status: channel.exit_status().unwrap(),
                })
            },
            Err(e) => Err(e.into())
        }
    }
}

impl SessSsh {
    pub fn new(user: &str, ip: &IPv4, port: i32, priv_key: &Path) -> Result<Self> {
        let tcp = TcpStream::connect(format!("{}:{}",
                                             &ip.ip(), port).as_str())
            .unwrap();
        let mut sess = ssh2::Session::new().unwrap();
        sess.handshake(&tcp).unwrap();
        sess.userauth_pubkey_file(user, None, priv_key, None).unwrap();
        sess.set_timeout(3000);
        sess.set_blocking(true);
        sess.set_allow_sigpipe(true);
        debug!("new session: {{user: {}, host: {}, priv_key: {}}}",
               user, ip.ip(), priv_key.to_str().unwrap());
        Ok(SessSsh { inner: sess })
    }
    // Update $HOME/.ssh/known_hosts file on host side (where the entire
    // programme is running).
    pub fn update_known_host(&self, host: &str) -> Result<()> {
        let mut known_hosts = try!(self.inner.known_hosts());
        let file = Path::new(&env::var("HOME").unwrap()).join(".ssh/known_hosts");
        info!("updateing {}", file.to_str().unwrap());
        try!(known_hosts.read_file(&file, KnownHostFileKind::OpenSSH));
        let (key, key_type) = self.inner.host_key().unwrap();
        match known_hosts.check(host, key) {
            CheckResult::Match => return Ok(()),
            CheckResult::NotFound => {}
            CheckResult::Mismatch => {
                for r in known_hosts.iter().filter(|h| match h {
                    &Ok(ref h) => h.name() == Some(host),
                    _ => false,
                }) {
                    try!(known_hosts.remove(r.unwrap()));
                }
            }
            CheckResult::Failure => panic!("failed to check the known hosts"),
        }
        info!("adding {} to the known hosts", host);
        try!(known_hosts.add(host,
                             key,
                             host,
                             match key_type {
                                 HostKeyType::Rsa => KnownHostKeyFormat::SshRsa,
                                 HostKeyType::Dss => KnownHostKeyFormat::SshDss,
                                 HostKeyType::Unknown => panic!("unknown type of key"),
                             }));
        try!(known_hosts.write_file(&file, KnownHostFileKind::OpenSSH));
        Ok(())
    }
}
