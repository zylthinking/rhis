fn tokenize(command: &str) -> Vec<String> {
    let mut tokens = vec![];
    let mut buf = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;

    for c in command.chars() {
        match c {
            '\\' if !in_single => {
                if escaped {
                    buf.push('\\');
                }
                escaped = !escaped;
            }
            '\'' if !escaped && !in_double => {
                in_single = !in_single;
            }
            '"' if !escaped && !in_single => {
                in_double = !in_double;
            }
            ' ' | '\t' if !in_single && !in_double && !escaped => {
                if !buf.is_empty() {
                    tokens.push(buf.clone());
                    buf.clear();
                }
            }
            _ => {
                if escaped {
                    escaped = false;
                }
                buf.push(c);
            }
        }
    }
    if !buf.is_empty() {
        tokens.push(buf);
    }
    tokens
}

fn is_flag(t: &str) -> bool {
    (t.starts_with('-') || t.starts_with("--")) && t != "-" && t != "--" && !t.starts_with("---") && t.len() > 1
}

pub fn normalize(command: &str) -> String {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let tokens = tokenize(trimmed);
    if tokens.is_empty() {
        return String::new();
    }

    let mut result = vec![tokens[0].clone()];
    let mut flag_buf: Vec<&str> = vec![];

    for t in &tokens[1..] {
        if is_flag(t) {
            flag_buf.push(t);
        } else {
            if !flag_buf.is_empty() {
                let last = flag_buf.pop().unwrap();
                flag_buf.sort_unstable();
                for f in &flag_buf {
                    result.push(f.to_string());
                }
                result.push(last.to_string());
                flag_buf.clear();
            }
            result.push(t.clone());
        }
    }

    if !flag_buf.is_empty() {
        flag_buf.sort_unstable();
        for f in &flag_buf {
            result.push(f.to_string());
        }
    }

    result.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_collapse() {
        assert_eq!(normalize("ls   -la   -sa"), "ls -la -sa");
        assert_eq!(normalize("  ls  -la  "), "ls -la");
    }

    #[test]
    fn flag_sorting() {
        assert_eq!(normalize("ls -sa -la"), "ls -la -sa");
        assert_eq!(normalize("ls -b -a -c"), "ls -a -b -c");
    }

    #[test]
    fn positional_wall() {
        assert_eq!(normalize("cmd -v -o output.txt"), "cmd -v -o output.txt");
        assert_eq!(
            normalize("cmd -x -v -o output.txt -b -a"),
            "cmd -v -x -o output.txt -a -b"
        );
    }

    #[test]
    fn no_flags() {
        assert_eq!(normalize("echo hello world"), "echo hello world");
    }

    #[test]
    fn single_flag() {
        assert_eq!(normalize("ls -la"), "ls -la");
        assert_eq!(normalize("git commit -m msg"), "git commit -m msg");
    }
}
