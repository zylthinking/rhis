use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: SubCommand,
    #[arg(long = "sid")]
    pub sid: Option<String>,
    #[arg(short = 'c', long = "config")]
    pub config: Option<String>,
}

#[derive(Subcommand)]
pub enum SubCommand {
    Add {
        command: Vec<String>,
        #[arg(value_name = "EXIT_CODE", short, long)]
        exit: i32,
    },

    Search {
        command: Vec<String>,
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
    pub fn is_init(&self) -> bool {
        matches!(self.command, SubCommand::Init { .. })
    }
}
