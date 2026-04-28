pub mod cli;
pub mod command_input;
pub mod conf;
pub mod db;
pub mod fixed_length_grapheme_string;
pub mod history;
pub mod history_cleaner;
pub mod interface;
pub mod normalize;
pub mod settings;
mod shell;
pub mod shell_history;

use std::sync::OnceLock;

static RUNTIME: OnceLock<tokio::runtime::Handle> = OnceLock::new();

pub fn set_runtime(handle: tokio::runtime::Handle) {
    let _ = RUNTIME.set(handle);
}

pub fn runtime() -> &'static tokio::runtime::Handle {
    RUNTIME.get().expect("runtime not initialized, call set_runtime first")
}
