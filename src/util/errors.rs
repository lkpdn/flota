use rusted_cypher::error as cypher;
use notify;
use ssh2;
use std::io;
use std::string;
use std::sync::mpsc;

error_chain! {
    foreign_links {
        cypher::GraphError, GraphError;
        string::FromUtf8Error, FromUtf8;
        io::Error, IO;
        mpsc::RecvError, MpscRecv;
        notify::Error, Notify;
        ssh2::Error, SSH2;
    }
}
