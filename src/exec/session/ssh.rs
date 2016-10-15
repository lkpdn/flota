use std::any::Any;
use std::error::Error as StdError;
use ssh2;
use ssh2::{CheckResult, HostKeyType, KnownHostFileKind, KnownHostKeyFormat};
use std::env;
use std::io::prelude::*;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use ::exec::Output;
use ::exec::session::{SeedType, Session, SessionSeed};
use ::util::errors::*;
use ::util::ipv4::IPv4;

#[allow(dead_code)]
pub struct SessSsh {
    session: ssh2::Session,
    tcp_stream: TcpStream,
}

impl Session for SessSsh {
    fn exec(&self, command: &str) -> Result<Output> {
        debug!("command: {}", command);
        // deadline 30 seconds.
        self.session.set_timeout(30 * 1000);
        match self.session.channel_session() {
            Ok(mut channel) => {
                channel.exec(command).unwrap();
                let mut stdout = String::new();
                let mut stderr = String::new();
                loop {
                    match (channel.read_to_string(&mut stdout),
                           channel.stderr().read_to_string(&mut stderr)) {
                        (Ok(_), Ok(_)) => { break },
                        (Err(e), _) | (_, Err(e)) => {
                            // XXX
                            if e.description().eq("would block") {
                                continue
                            }
                            return Err(e.into())
                        }
                    }
                }
                Ok(Output {
                    stdout: Some(stdout),
                    stderr: Some(stderr),
                    status: channel.exit_status().ok(),
                })
            },
            Err(e) => {
                Err(e.into())
            }
        }
    }
}

impl SessSsh {
    pub fn new(user: &str, ip: &IPv4, port: i32, priv_key: &Path) -> Result<Box<Self>> {
        debug!("tcp stream connect: {}:{}", &ip.ip(), port);
        let tcp = try!(TcpStream::connect(format!("{}:{}",
                                          &ip.ip(), port).as_str()));
        let mut sess = ssh2::Session::new().unwrap();
        sess.handshake(&tcp).unwrap();
        sess.userauth_pubkey_file(user, None, priv_key, None).unwrap();
        sess.set_blocking(true);
        sess.set_keepalive(false, 5);
        sess.set_allow_sigpipe(true);
        debug!("new session: {{user: {}, host: {}, priv_key: {}}}",
               user, ip.ip(), priv_key.to_str().unwrap());
        Ok(Box::new(SessSsh {
            session: sess,
            tcp_stream: tcp,
        }))
    }

    // Update $HOME/.ssh/known_hosts file on host side (where the entire
    // programme is running).
    pub fn update_known_host(&self, host: &str) -> Result<()> {
        let mut known_hosts = try!(self.session.known_hosts());
        let file = Path::new(&env::var("HOME").unwrap()).join(".ssh/known_hosts");
        info!("updateing {}", file.to_str().unwrap());
        try!(known_hosts.read_file(&file, KnownHostFileKind::OpenSSH));
        let (key, key_type) = self.session.host_key().unwrap();
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

#[derive(Debug, Clone)]
pub struct SessSeedSsh {
    pub user: String,
    pub ip: Option<IPv4>,
    pub port: i32,
    pub priv_key: PathBuf,
}

impl SessSeedSsh {
    pub fn new(user: &str, ip: Option<&IPv4>, port: i32, priv_key: &Path) -> Box<SessionSeed> {
        Box::new(SessSeedSsh {
            user: user.to_owned(),
            ip: match ip { Some(v) => Some(v.clone()), None => None },
            port: port,
            priv_key: priv_key.to_path_buf(),
        })
    }
    pub fn override_ip(&mut self, ip: &IPv4) -> () {
        self.ip = Some(ip.clone());
    }
}

impl SessionSeed for SessSeedSsh {
    fn spawn(&self) -> Result<Box<Session>> {
        // at this moment self.ip must be some.
        Ok(try!(
            self::SessSsh::new(&self.user,
                               match self.ip {
                                   Some(ref v) => v,
                                   None => panic!("would not panic")
                               },
                               self.port,
                               self.priv_key.as_path())
        ))
    }
    fn seed_type(&self) -> SeedType {
        SeedType::Ssh
    }
    fn as_mut_any(&mut self) -> &mut Any {
        self
    }
}
