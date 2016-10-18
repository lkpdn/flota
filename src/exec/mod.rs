use std::fmt;
use ::flota::Cypherable;

pub mod session;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct Output {
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub status: Option<i32>,
}

impl fmt::Display for Output {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let fmt_stdout = match self.stdout {
            Some(ref stdout) => {
                format!("stdout: {}", stdout)
            },
            None => {
                "stdout: N/A".to_string()
            }
        };
        let fmt_stderr = match self.stderr {
            Some(ref stderr) => {
                format!("stderr: {}", stderr)
            },
            None => {
                "stderr: N/A".to_string()
            }
        };
        let fmt_status = match self.status {
            Some(ref status) => {
                format!("status: {}", status)
            },
            None => {
                "status: N/A".to_string()
            }
        };
        write!(f, "{}\n{}\n{}", fmt_stdout,
                                fmt_stderr,
                                fmt_status)
    }
}

impl Output {
    pub fn satisfy(&self, expected: &Output) -> bool {
        if expected.stdout.is_some() && self.stdout != expected.stdout {
            return false
        }
        if expected.stderr.is_some() && self.stderr != expected.stderr {
            return false
        }
        if expected.status.is_some() && self.status != expected.status {
            return false
        }
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct ExecResult {
    pub host: String,
    pub command: String,
    pub expected: Output,
    pub result: Output,
    pub passed: bool,
}

impl Cypherable for ExecResult {
    fn cypher_ident(&self) -> String {
        format!("ExecResult {{ host: '{host}',
                               command: '{command}',
                               expected: '{expected:?}',
                               result: '{result:?}',
                               passed: '{passed}' }}",
                host = self.host,
                command = self.command,
                expected = self.expected,
                result = self.result,
                passed = self.passed)
    }
}
