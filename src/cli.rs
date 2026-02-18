use clap::{Parser, Subcommand, ValueEnum};

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
    Completions {
        #[arg(long, value_enum)]
        shell: Option<ShellArg>,
    },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum ShellArg {
    Bash,
    Zsh,
    Fish,
    Elvish,
    Powershell,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    Template {
        #[command(subcommand)]
        command: TemplateCommand,
    },
    EventKind {
        #[command(subcommand)]
        command: EventKindCommand,
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

#[derive(Debug, Subcommand)]
pub enum EventKindCommand {
    Get {
        #[arg(long, value_enum)]
        agent: Option<Agent>,
        #[arg(long)]
        key: String,
    },
    Set {
        #[arg(long, value_enum)]
        agent: Option<Agent>,
        #[arg(long)]
        key: String,
        #[arg(long)]
        value: String,
    },
    Reset {
        #[arg(long, value_enum)]
        agent: Option<Agent>,
        #[arg(long)]
        key: String,
    },
}
