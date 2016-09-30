use notify;
use ssh2;
use toml;
use std::io;
use std::string;
use std::sync::mpsc;

error_chain! {
    foreign_links {
        string::FromUtf8Error, FromUtf8;
        io::Error, IO;
        mpsc::RecvError, MpscRecv;
        notify::Error, Notify;
        ssh2::Error, SSH2;
    }
}
