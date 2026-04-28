use std::{
    env, fs,
    fs::File,
    io,
    io::Read,
    path::{Path, PathBuf},
};

fn read_history_file(path: &Path) -> Result<String, io::Error> {
    let mut f = File::open(path)?;
    let mut buffer = Vec::new();
    f.read_to_end(&mut buffer)?;
    Ok(String::from_utf8_lossy(&buffer).to_string())
}

fn has_leading_timestamp(line: &str) -> bool {
    line.len() == 11 && line.starts_with('#') && line[1..].chars().all(|c| c.is_ascii_digit())
}

fn history_file_path() -> Option<PathBuf> {
    let path = PathBuf::from(env::var("HISTFILE").unwrap_or_default());
    fs::canonicalize(path).ok()
}

fn load_history(path: &Path) -> Option<Vec<String>> {
    let contents = read_history_file(path).ok()?;
    Some(
        contents
            .split('\n')
            .filter(|line| !has_leading_timestamp(line) && !line.is_empty())
            .map(|cmd| cmd.trim().to_string())
            .collect(),
    )
}

pub fn full_history() -> Vec<String> {
    let Some(path) = history_file_path() else {
        return vec![];
    };
    load_history(&path).unwrap_or_default()
}

pub fn delete_lines(command: &str) {
    let Some(path) = history_file_path() else {
        return;
    };
    let Some(commands) = load_history(&path) else {
        return;
    };
    if commands.is_empty() {
        return;
    }
    let lines: Vec<String> = commands
        .into_iter()
        .filter(|cmd| command != cmd)
        .chain(Some(String::new()))
        .collect();
    let _ = fs::write(&path, lines.join("\n"));
}
