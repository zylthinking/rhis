use crate::{db, shell_history};

pub fn clean(original: &str) {
    crate::runtime().block_on(db::delete_command(original));
    shell_history::delete_lines(original);
}
