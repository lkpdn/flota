use ssh2;
use ssh2::{CheckResult, HostKeyType, KnownHostFileKind, KnownHostKeyFormat};
use std::env;
use std::path::Path;
use std::process::{Command, Stdio};
use url::Url;
use ::consts::*;

pub mod errors;
pub mod ipv4;
pub mod md5sum;
pub mod notify;
pub mod url;

use self::errors::*;
use self::ipv4::IPv4;

macro_rules! xE {
    ( $x:expr ) => { xE!($x,) };
    ( $x:expr, $($k:ident => $v:expr),* ) => {
    xml::Element::new($x.into(), None, vec![
    $(
        (stringify!($k).into(), None, $v.into()),
    )*])
    }
}

macro_rules! rawCharPtr {
    ( $x:expr ) => {{
    use std::ffi::CString;
    CString::new(format!("{}", $x)).unwrap().as_ptr()
    }}
}

macro_rules! cmd {
    ( $x:expr ) => {{
    let vec = $x.split(" ").collect::<Vec<&str>>();
    let (ref head, ref tail) = vec.split_first().unwrap();
    Command::new(head).args(tail)
    }}
}

/// Update $HOME/.ssh/known_hosts file on host side (where the entire
/// programme is running).
pub fn update_known_host(session: &ssh2::Session, host: &str) -> Result<()> {
    let mut known_hosts = try!(session.known_hosts());
    let file = Path::new(&env::var("HOME").unwrap()).join(".ssh/known_hosts");
    info!("updateing {}", file.to_str().unwrap());
    try!(known_hosts.read_file(&file, KnownHostFileKind::OpenSSH));
    let (key, key_type) = session.host_key().unwrap();
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

/// Update /etc/hosts file on host side (where the entire programme
/// is running) to enable ssh login guest node just specifying its
/// hostname.
pub fn update_etc_hosts(path: Option<&Path>, ip: &IPv4, hostname: &str) -> Result<()> {
    let hosts_path: &str = match path {
        Some(v) => v.to_str().unwrap(),
        None => "/etc/hosts",
    };
    // create autogenerated part if not exists.
    {
        let pat = format!("/# autogenerated by {}/{{:a;$!{{N;ba}};q;}};$ a\\\\n# \
                           autogenerated by {}\\n# >>>>\\n# <<<<\\n",
                          PROGNAME.as_str(),
                          PROGNAME.as_str());
        let mut cmd = Command::new("sed");
        cmd.args(&["-i", &pat, hosts_path]);
        info!("pat: {}", pat);
        info!("{:?}", cmd);
        if !cmd.status().expect("failed to execute sed on hosts file").success() {
            return Err("sed on hosts file returned non-zero".into());
        }
    }
    // update entry.
    {
        let pat = format!("x;/^$/{{x;h;ba;}};/.*/{{x;H;}};:a;/^$/{{x;\
                           /# autogenerate/{{\
                             s/^\\(.*\\n\\){}[^\\n]*\\n\\(.*\\)$/\\1\\2/;\
                             s/^\\(.*\\n\\)\\(# <<<<.*\\)$/\\1{} {}\\n\\2/;\
                           }};p;d;h;}};d",
                          ip.ip().as_str(),
                          ip.ip().as_str(),
                          hostname);
        let mut cmd = Command::new("sed");
        cmd.args(&["-i", &pat, hosts_path]);
        info!("pat: {}", pat);
        info!("{:?}", cmd);
        if cmd.status().expect("failed to execute sed on hosts file").success() {
            Ok(())
        } else {
            Err("sed on hosts file returned non-zero".into())
        }
    }
}

pub fn download_file<'a>(remote_url: &Url, local_path: &Path) -> Result<()> {
    let option = if local_path.is_dir() { "-P" } else { "-O" };
    match Command::new("wget")
        .args(&[remote_url.as_str(), option, local_path.to_str().unwrap()])
        .stderr(Stdio::null())
        .status() {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => {
            Err(format!("`wget {} {} {}` failed [error: {}]",
                        remote_url.as_str(),
                        option,
                        local_path.to_str().unwrap(),
                        status.code().unwrap())
                .into())
        }
        Err(e) => Err(format!("failed to execute command wget: {}", e).into()),
    }
}

#[cfg(test)]
mod tests {
    use difference::diff;
    use difference::Difference;
    use std::env;
    use std::fs;
    use std::fs::File;
    use std::io::prelude::*;
    use super::*;
    use ::consts::*;
    use ::util::ipv4::IPv4;

    #[test]
    fn test_update_etc_hosts() {
        let temp_file = env::temp_dir().join(".test_update_etc_hosts");
        let mut f = File::create(&temp_file).expect("failed to create file");
        let orig1 = "\
            # The following lines are desirable for IPv4 capable hosts\n\
            127.0.0.1 localhost.localdomain localhost\n\
            127.0.0.1 localhost4.localdomain4 localhost4\n\
            # The following lines are desirable for IPv6 capable hosts\n\
            ::1 test\n\
            ::1 localhost.localdomain localhost\n\
            ::1 localhost6.localdomain6 localhost6\n\
            \n\
            10.10.10.10 test1\n\
            20.20.20.20 test2";
        macro_rules! inserted {
            ( $e:expr ) => {{
                format!("\n\n# autogenerated by {}\n\
                    # >>>>\n{} {}\n# <<<<\n\n", PROGNAME.as_str(), $e.0.ip(), $e.1)
            }}
        }
        f.write(orig1.as_bytes()).expect("write into temp hosts file failed");

        // pattern 1.
        {
            let ip1 = IPv4::from_cidr_notation("11.11.11.11/24").unwrap();
            let entry1 = (&ip1, &"test11".to_string());
            // "3" is just an arbitrary num. to show idempotence
            for _ in 0..3 {
                update_etc_hosts(Some(temp_file.as_path()), &entry1.0, &entry1.1)
                    .expect("failed to update temp hosts file");
                let mut f = File::open(&temp_file).expect("File open failed");
                let mut buffer = Vec::new();
                f.read_to_end(&mut buffer).expect("read_to_end failed");
                let (_, changeset) = diff(&orig1, &String::from_utf8(buffer).unwrap(), "");
                assert_eq!(changeset.len(), 2);
                match (&changeset[0], &changeset[1]) {
                    (&Difference::Same(_), &Difference::Add(ref add)) => {
                        assert_eq!(add, inserted!(&entry1).as_str());
                    }
                    _ => { assert!(false); }
                }
            }
        }
        fs::remove_file(temp_file.as_path()).expect("failed to remove file");
    }
}
