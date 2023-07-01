use is_executable::IsExecutable;
use relative_path::RelativePath;
use std::path::Path;

trait Shell {
    const NOT_FOUND: i32;
    fn arg0_from_cmdline(&mut self, line: &str) -> String;
}

pub(super) mod bash {
    use std::vec;

    pub struct Bash {
        fns: Vec<&'static dyn Fn(&mut Self, u8)>,
        cmd: Option<Vec<u8>>,
    }

    impl Bash {
        pub fn new() -> Self {
            Bash {
                fns: vec![&Self::bare],
                cmd: Some(vec![]),
            }
        }

        fn cmd_array(&mut self) -> &mut Vec<u8> { self.cmd.as_mut().unwrap() }

        fn bare(&mut self, byte: u8) {
            if byte == b' ' || byte == b'\t' {
                self.fns.pop();
            } else if byte == b'\'' {
                self.fns.push(&Self::quote::<b'\''>);
            } else if byte == b'"' {
                self.fns.push(&Self::quote::<b'"'>);
            } else if byte == b'\\' {
                self.fns.push(&Self::escape);
            } else {
                self.cmd_array().push(byte);
            }
        }

        fn quote<const I: u8>(&mut self, byte: u8) {
            if byte == I {
                self.fns.pop();
            } else if byte == b'\\' {
                self.fns.push(&Self::escape);
            } else {
                self.cmd_array().push(byte);
            }
        }

        fn escape(&mut self, byte: u8) {
            let n = self.fns.len();
            let v = self.cmd_array();
            if n == 2 {
                match byte {
                    b'n' => v.push(b'\n'),
                    b't' => v.push(b'\t'),
                    b'r' => v.push(b'\r'),
                    _ => v.push(byte),
                }
            } else {
                v.push(b'\\');
                v.push(byte);
            }
            self.fns.pop();
        }
    }

    impl super::Shell for Bash {
        const NOT_FOUND: i32 = 127;

        fn arg0_from_cmdline(&mut self, line: &str) -> String {
            let bytes = line.as_bytes();
            let nb = bytes.len();
            self.cmd_array().reserve(nb);

            let mut nr = self.fns.len();
            for &b in bytes {
                self.fns[nr - 1](self, b);
                nr = self.fns.len();
                if nr == 0 {
                    break;
                }
            }
            String::from_utf8(self.cmd.take().unwrap()).unwrap()
        }
    }
}

pub(super) fn normalize_path(incoming_path: &str, cwd: &str) -> Option<String> {
    let expanded_path = shellexpand::tilde(incoming_path);

    let cwd = Path::new(&cwd);
    if !cwd.is_absolute() {
        return None;
    }

    let path_buf = if expanded_path.starts_with('/') {
        RelativePath::new(&expanded_path).normalize().to_path("")
    } else {
        let to_current_dir = RelativePath::new(&expanded_path).to_path(cwd);
        RelativePath::new(to_current_dir.to_str().unwrap()).normalize().to_path("")
    };

    if path_buf.is_absolute() {
        path_buf.to_str().map(|s| s.to_owned())
    } else {
        None
    }
}

pub(super) fn execute_able(cmd: &str, cwd: &str, exit_code: i32) -> bool {
    if exit_code != bash::Bash::NOT_FOUND {
        return true;
    }

    let mut shell = bash::Bash::new();
    let cmd = shell.arg0_from_cmdline(cmd);
    let cmd = cmd.as_str();
    let cmd = shellexpand::tilde(cmd);
    let cmd = cmd.as_ref();
    let p = Path::new(cmd);

    let path = if p.is_absolute() {
        if p.is_file() { p.canonicalize().ok() } else { None }
    } else {
        let n = p.components().count();
        if n == 1 {
            which::which(cmd).ok()
        } else {
            RelativePath::from_path(&p).map(|rp| rp.normalize().to_path(cwd)).ok()
        }
    };

    match path {
        None => false,
        Some(ref pathbuf) => pathbuf.as_path().is_executable(),
    }
}
