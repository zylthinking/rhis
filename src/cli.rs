use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: SubCommand,
    #[arg(long = "sid")]
    pub sid: Option<String>,
}

#[derive(Subcommand)]
pub enum SubCommand {
    Add {
        command: Vec<String>,
        #[arg(value_name = "EXIT_CODE", short, long)]
        exit: i32,
        #[arg(value_name = "PATH", short, long = "dir")]
        directory: Option<String>,
    },

    Search {
        command: Vec<String>,
        #[arg(value_name = "PATH", short, long = "dir")]
        directory: Option<String>,
        #[arg(short, long = "bottom")]
        bottom: bool,
        #[arg(short, long = "light")]
        light: bool,
    },

    Init {
        #[arg(short, long = "bottom")]
        bottom: bool,
        #[arg(short, long = "light")]
        light: bool,
    },
}

impl Cli {
    pub fn is_init(&self) -> bool { matches!(self.command, SubCommand::Init { .. }) }
}
