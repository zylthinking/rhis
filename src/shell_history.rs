use std::{
    env, fs,
    fs::File,
    io,
    io::Read,
    path::{Path, PathBuf},
};

fn read_ignoring_utf_errors<const B: bool>(path: &Path) -> Result<String, io::Error> {
    let f = File::open(path);
    if f.is_err() {
        let e = f.err().unwrap();
        if B {
            panic!("{}", e);
        }
        return Err(e);
    }
    let mut f = f.unwrap();

    let mut buffer = Vec::new();
    let r = f.read_to_end(&mut buffer);
    if r.is_err() {
        let e = r.err().unwrap();
        if B {
            panic!("{}", e);
        }
        return Err(e);
    }
    Ok(String::from_utf8_lossy(&buffer).to_string())
}

fn has_leading_timestamp(line: &str) -> bool {
    let mut matched_chars = 0;
    for (index, c) in line.chars().enumerate() {
        if index > 11 {
            break;
        }

        if index == 0 && c != '#' || !c.is_ascii_digit() {
            break;
        }
        matched_chars += 1;
    }
    matched_chars == 11
}

fn history_file_path() -> Option<PathBuf> {
    let path = PathBuf::from(env::var("HISTFILE").unwrap_or("".into()));
    fs::canonicalize(path).ok()
}

fn __full_history<const B: bool>(path: &Path) -> Option<Vec<String>> {
    let history_contents = read_ignoring_utf_errors::<B>(path);
    if history_contents.is_err() {
        return None;
    }

    Some(
        history_contents
            .unwrap()
            .split('\n')
            .filter(|line| !has_leading_timestamp(line) && !line.is_empty())
            .map(|cmd| cmd.trim().to_string())
            .collect(),
    )
}

pub fn full_history() -> Vec<String> {
    let path = history_file_path().unwrap();
    __full_history::<true>(path.as_path()).unwrap()
}

pub fn delete_lines(command: &str) {
    let opt = history_file_path();
    if opt.is_none() {
        return;
    }

    let path = &opt.unwrap();
    let commands = __full_history::<false>(path.as_path());
    if commands.is_none() {
        return;
    }

    let commands = commands.unwrap();
    if commands.is_empty() {
        return;
    }

    let lines =
        commands.into_iter().filter(|cmd| !command.eq(cmd)).chain(Some(String::from(""))).collect::<Vec<String>>();
    let _ = fs::write(path, lines.join("\n"));
}
