use crate::{db, shell_history};

pub fn clean(original: &str, dir: &str) {
    let rt = tokio::runtime::Handle::current();
    rt.block_on(db::delete_command(original, dir));
    shell_history::delete_lines(original);
}
