use is_executable::IsExecutable;
use relative_path::RelativePath;
use std::path::Path;

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

pub(super) fn execute_able(cmd: &str, cwd: &str) -> bool {
    let cmd = shellexpand::tilde(cmd);
    let cmd = cmd.as_ref();
    let p = Path::new(cmd);

    let path = if p.is_absolute() {
        if p.is_file() {
            p.canonicalize().ok()
        } else {
            None
        }
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
