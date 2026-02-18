mod cli;

use agitiser_notify::agent::SetupAgent;
use agitiser_notify::event::normalize;
use agitiser_notify::integrations::{claude, codex};
use agitiser_notify::{paths, speech, state};
use anyhow::{bail, Context, Result};
use clap::Parser;
use serde_json::Value;
use std::io::{IsTerminal, Read};

use crate::cli::{Cli, Commands};

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Setup { agents } => setup_agents(agents),
        Commands::Remove { agents } => remove_agents(agents),
        Commands::Ingest {
            agent,
            payload,
            trailing_payload,
            source,
            verbose,
        } => ingest_event(agent, payload, trailing_payload, source, verbose),
        Commands::Doctor => doctor(),
    }
}

fn setup_agents(agents: Vec<SetupAgent>) -> Result<()> {
    let executable_path =
        std::env::current_exe().context("failed to resolve current executable path")?;
    let claude_path = paths::claude_settings_path()?;
    let codex_path = paths::codex_config_path()?;
    let state_path = paths::local_state_path()?;

    let mut local_state = state::load(&state_path)?;
    let initial_state = local_state.clone();

    for agent in dedup_agents(agents) {
        match agent {
            SetupAgent::Claude => {
                let changed = claude::setup(&claude_path, &executable_path)?;
                if changed {
                    println!(
                        "Claude setup: installed Stop hook in {}",
                        claude_path.display()
                    );
                } else {
                    println!("Claude setup: already configured");
                }
            }
            SetupAgent::Codex => {
                let changed = codex::setup(&codex_path, &mut local_state, &executable_path)?;
                if changed {
                    println!(
                        "Codex setup: configured notify command in {}",
                        codex_path.display()
                    );
                } else {
                    println!("Codex setup: already configured");
                }
            }
            SetupAgent::Opencode => {
                println!(
                    "OpenCode setup: manual only in this release; see README for manual integration."
                );
            }
        }
    }

    if local_state != initial_state {
        state::save(&state_path, &local_state)?;
    }

    Ok(())
}

fn remove_agents(agents: Vec<SetupAgent>) -> Result<()> {
    let claude_path = paths::claude_settings_path()?;
    let codex_path = paths::codex_config_path()?;
    let state_path = paths::local_state_path()?;

    let mut local_state = state::load(&state_path)?;
    let initial_state = local_state.clone();

    for agent in dedup_agents(agents) {
        match agent {
            SetupAgent::Claude => {
                let changed = claude::remove(&claude_path)?;
                if changed {
                    println!("Claude remove: removed managed Stop hook");
                } else {
                    println!("Claude remove: no managed hook found");
                }
            }
            SetupAgent::Codex => {
                let changed = codex::remove(&codex_path, &mut local_state)?;
                if changed {
                    println!("Codex remove: removed managed notify command");
                } else {
                    println!("Codex remove: no managed notify command found");
                }
            }
            SetupAgent::Opencode => {
                println!("OpenCode remove: nothing to remove (manual integration only).");
            }
        }
    }

    if local_state != initial_state {
        state::save(&state_path, &local_state)?;
    }

    Ok(())
}

fn ingest_event(
    agent: agitiser_notify::agent::Agent,
    payload: Option<String>,
    trailing_payload: Option<String>,
    source: Option<String>,
    verbose: bool,
) -> Result<()> {
    let payload_text = match payload.or(trailing_payload) {
        Some(payload_text) => payload_text,
        None => {
            if std::io::stdin().is_terminal() {
                bail!("no payload provided and stdin is a terminal; pass --payload or pipe JSON via stdin");
            }
            let mut stdin_payload = String::new();
            std::io::stdin()
                .read_to_string(&mut stdin_payload)
                .context("failed to read payload from stdin")?;
            stdin_payload
        }
    };

    if payload_text.trim().is_empty() {
        if verbose {
            eprintln!("ingest: empty payload, skipping");
        }
        return Ok(());
    }

    let parsed_payload = match serde_json::from_str::<Value>(&payload_text) {
        Ok(value) => value,
        Err(error) => {
            if verbose {
                eprintln!("ingest: invalid JSON payload ({error})");
            }
            return Ok(());
        }
    };

    let Some(event) = normalize(agent, parsed_payload) else {
        if verbose {
            eprintln!("ingest: payload is not a terminal event for {agent:?}");
        }
        return Ok(());
    };

    speech::speak(&event)?;
    if verbose {
        let cwd = event
            .cwd
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "<unknown>".to_string());
        let payload_type = event
            .raw_payload
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("<none>");
        let source_label = source.as_deref().unwrap_or("<none>");
        eprintln!(
            "ingest: announced {} event for project {} (cwd: {}, type: {}, source: {})",
            event.event_kind, event.project_name, cwd, payload_type, source_label
        );
    }
    Ok(())
}

fn doctor() -> Result<()> {
    let claude_path = paths::claude_settings_path()?;
    let codex_path = paths::codex_config_path()?;

    let mut has_errors = false;

    match speech::spd_say_path() {
        Some(path) => println!(
            "[ok] speech-dispatcher: found spd-say at {}",
            path.display()
        ),
        None => {
            println!("[error] speech-dispatcher: spd-say not found in PATH");
            has_errors = true;
        }
    }

    match claude::is_configured(&claude_path)? {
        true => println!("[ok] claude: managed Stop hook configured"),
        false => {
            println!("[info] claude: managed Stop hook not configured");
        }
    }

    match codex::is_configured(&codex_path)? {
        true => println!("[ok] codex: managed notify command configured"),
        false => {
            println!("[info] codex: managed notify command not configured");
        }
    }

    println!("[info] opencode: auto-setup is not implemented in this release");

    if has_errors {
        bail!("doctor found critical issues");
    }

    Ok(())
}

fn dedup_agents(agents: Vec<SetupAgent>) -> Vec<SetupAgent> {
    let mut ordered = Vec::new();
    for agent in agents {
        if !ordered.contains(&agent) {
            ordered.push(agent);
        }
    }
    ordered
}
