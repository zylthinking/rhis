use crate::{history::History, shell_history};

pub fn clean(history: &mut History, command: &str) {
    history.delete_command(command);
    shell_history::delete_lines(command)
}
