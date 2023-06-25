use relative_path::RelativePath;
use std::{env, path::Path};
use unicode_segmentation::UnicodeSegmentation;

pub fn normalize_path(incoming_path: &str) -> Option<String> {
    let expanded_path = shellexpand::tilde(incoming_path);

    let cwd = env::var("PWD").ok()?;
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

pub fn parse_mv_command(command: &str) -> Vec<String> {
    let mut in_double_quote = false;
    let mut in_single_quote = false;
    let mut escaped = false;
    let mut buffer = String::new();
    let mut result: Vec<String> = Vec::new();

    for grapheme in command.graphemes(true) {
        match grapheme {
            "\\" => {
                escaped = true;
            }
            "\"" => {
                if escaped {
                    escaped = false;
                    buffer.push_str(grapheme);
                } else if in_double_quote {
                    in_double_quote = false;
                    if !buffer.is_empty() {
                        result.push(buffer);
                    }
                    buffer = String::new();
                } else if !in_single_quote {
                    in_double_quote = true;
                } else {
                    buffer.push_str(grapheme);
                }
            }
            "\'" => {
                if in_single_quote {
                    in_single_quote = false;
                    if !buffer.is_empty() {
                        result.push(buffer);
                    }
                    buffer = String::new();
                } else if !in_double_quote {
                    in_single_quote = true;
                } else {
                    buffer.push_str(grapheme);
                }
                escaped = false;
            }
            " " => {
                if in_double_quote || in_single_quote || escaped {
                    buffer.push_str(grapheme);
                } else {
                    if !buffer.is_empty() {
                        result.push(buffer);
                    }
                    buffer = String::new();
                }
                escaped = false;
            }
            _ => {
                buffer.push_str(grapheme);
                escaped = false;
            }
        }
    }

    if !buffer.is_empty() {
        result.push(buffer);
    }
    result.iter().skip(1).filter(|s| !s.starts_with('-')).map(|s| s.to_owned()).collect()
}
