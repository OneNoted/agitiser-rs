use clap::{Parser, Subcommand};

use agitiser_notify::agent::{Agent, SetupAgent};

#[derive(Debug, Parser)]
#[command(
    name = "agitiser-notify",
    version,
    about = "Agent task completion speech notifier"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Setup {
        #[arg(
            long,
            value_enum,
            value_delimiter = ',',
            default_values_t = [SetupAgent::Claude, SetupAgent::Codex]
        )]
        agents: Vec<SetupAgent>,
    },
    Remove {
        #[arg(
            long,
            value_enum,
            value_delimiter = ',',
            default_values_t = [SetupAgent::Claude, SetupAgent::Codex]
        )]
        agents: Vec<SetupAgent>,
    },
    Ingest {
        #[arg(long, value_enum)]
        agent: Agent,
        #[arg(long)]
        payload: Option<String>,
        #[arg(index = 1)]
        trailing_payload: Option<String>,
        #[arg(long)]
        source: Option<String>,
        #[arg(long, default_value_t = false)]
        verbose: bool,
    },
    Doctor,
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    Template {
        #[command(subcommand)]
        command: TemplateCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum TemplateCommand {
    Get {
        #[arg(long, value_enum)]
        agent: Option<Agent>,
    },
    Set {
        #[arg(long, value_enum)]
        agent: Option<Agent>,
        #[arg(long)]
        value: String,
    },
    Reset {
        #[arg(long, value_enum)]
        agent: Option<Agent>,
    },
}
