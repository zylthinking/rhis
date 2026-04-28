use crate::cli::{Cli, SubCommand};
use clap::Parser;

#[derive(Debug)]
pub enum Mode {
    Add,
    Search,
    Init,
}

pub struct Settings {
    pub mode: Mode,
    pub command: String,
    pub exit_code: i32,
    pub lightmode: bool,
    pub bottom: bool,
    pub config_path: Option<String>,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            mode: Mode::Add,
            command: String::new(),
            exit_code: 0,
            lightmode: false,
            bottom: false,
            config_path: None,
        }
    }
}

impl Settings {
    pub fn parse_args() -> Settings {
        let cli = Cli::parse();
        let mut settings = Settings {
            config_path: cli.config,
            ..Default::default()
        };

        match cli.command {
            SubCommand::Add {
                command,
                exit,
            } => {
                settings.mode = Mode::Add;
                settings.exit_code = exit;
                if !command.is_empty() {
                    settings.command = command.join(" ").trim().into();
                }
            }

            SubCommand::Search {
                command,
                bottom,
                light,
            } => {
                settings.mode = Mode::Search;
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
}
