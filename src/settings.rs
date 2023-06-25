use crate::cli::{Cli, SubCommand};
use clap::Parser;
use directories_next::ProjectDirs;
use std::path::PathBuf;

#[derive(Debug)]
pub enum Mode {
    Add,
    Search,
    Init,
}

pub struct Settings {
    pub mode: Mode,
    pub sid: String,
    pub command: String,
    pub dir: String,
    pub results: u16,
    pub exit_code: i32,
    pub lightmode: bool,
    pub bottom: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            mode: Mode::Add,
            command: String::new(),
            sid: String::new(),
            dir: String::new(),
            results: 40,
            exit_code: 0,
            lightmode: false,
            bottom: false,
        }
    }
}

impl Settings {
    pub fn parse_args() -> Settings {
        let cli = Cli::parse();
        let mut settings = Settings { ..Default::default() };

        settings.sid = cli.sid.unwrap_or("".into());
        match cli.command {
            SubCommand::Add {
                command,
                exit,
                directory,
            } => {
                settings.mode = Mode::Add;
                settings.exit_code = exit;
                settings.dir = directory.unwrap();
                if !command.is_empty() {
                    settings.command = command.join(" ").trim().into();
                }
            }

            SubCommand::Search {
                command,
                directory,
                bottom,
                light,
            } => {
                settings.mode = Mode::Search;
                settings.dir = directory.unwrap();
                if !command.is_empty() {
                    settings.command = command.join(" ").trim().into();
                }
                settings.bottom = bottom;
                settings.lightmode = light;
            }

            SubCommand::Init { bottom, light } => {
                settings.mode = Mode::Init;
                settings.bottom = bottom;
                settings.lightmode = light;
            }
        }

        settings
    }

    pub fn db_path() -> PathBuf {
        let data_dir = ProjectDirs::from("", "", "rhis").unwrap().data_dir().to_path_buf();
        data_dir.join(PathBuf::from("history.db"))
    }
}
