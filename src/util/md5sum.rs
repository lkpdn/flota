use crypto::digest::Digest;
use crypto::md5::Md5;
use std::thread;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{sync_channel, SyncSender, Receiver};
use url::Url;

use ::util::errors::*;

pub fn download_file(remote_url: &Url, local_path: &Path, md5: &str) -> Result<()> {
    if !local_path.exists() {
        if let Err(e) = super::download_file(remote_url, local_path) {
            error!("{}", e);
            return Err(format!("failed to download {} [{}]", remote_url.as_str(), e).into());
        }
    }
    match compare_md5(local_path, md5) {
        Ok(true) => Ok(()),
        Ok(false) => Err("md5 unmatch error".into()),
        Err(e) => Err(e),
    }
}

fn read_file(path: PathBuf, chunk: usize, out_channel: SyncSender<Vec<u8>>) {
    let file = File::open(path.to_str().unwrap()).expect("failed to open file");
    let mut reader = BufReader::with_capacity(chunk, file);
    loop {
        let length = {
            let buffer = reader.fill_buf().unwrap();
            match out_channel.send(buffer.to_vec()) {
                Ok(_) => { buffer.len() },
                Err(e) => { panic!("{}", e) }
            }
        };
        if length == 0 { break };
        reader.consume(length);
    }
}

fn md5_input(hasher: &mut Md5, in_channel: Receiver<Vec<u8>>) {
    for chunk in in_channel.iter() {
        hasher.input(&chunk[..]);
    }
}

pub fn compare_md5(local_path: &Path, md5: &str) -> Result<bool> {
    let (sender, receiver) = sync_channel(16);
    let t = local_path.to_path_buf();
    let sender = thread::spawn(move || read_file(t, 10240, sender));
    let mut hasher = Md5::new();
    md5_input(&mut hasher, receiver);
    match sender.join() {
        Ok(_) => {
            let md5_result = hasher.result_str();
            Ok(md5_result == md5)
        },
        Err(_) => Err("failed to compare md5. maybe file reader aborted.".into())
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::fs::File;
    use std::io::prelude::*;
    use super::*;

    #[test]
    fn test_compare_md5() {
        let temp_file = env::temp_dir().join(".test_compare_md5");
        let mut f = File::create(&temp_file).expect("failed to create file");
        (0..1000).map(|_| "ok").collect::<String>();
        f.write_all((0..1000)
                .map(|_| "ok")
                .collect::<String>()
                .into_bytes()
                .as_slice())
            .expect("failed to write into file");
        assert!(compare_md5(temp_file.as_path(), "1475d0fe0bbf3f58901703267deb7560").unwrap());
        fs::remove_file(temp_file.as_path()).expect("failed to remove file");
    }
}
