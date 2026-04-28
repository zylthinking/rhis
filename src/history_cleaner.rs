use crate::{db, shell_history};

pub fn clean(original: &str) {
    let rt = tokio::runtime::Handle::current();
    rt.block_on(db::delete_command(original));
    shell_history::delete_lines(original);
}
