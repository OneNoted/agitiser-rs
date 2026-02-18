mod cli;

use agitiser_notify::agent::{Agent, SetupAgent};
use agitiser_notify::event::normalize;
use agitiser_notify::integrations::{claude, codex};
use agitiser_notify::{paths, speech, state};
use anyhow::{bail, Context, Result};
use clap::{CommandFactory, Parser};
use clap_complete::{generate, Shell};
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::{self, IsTerminal, Read};
use std::path::Path;

use crate::cli::{Cli, Commands, ConfigCommand, EventKindCommand, ShellArg, TemplateCommand};

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Completions { shell } => {
            let resolved_shell = shell
                .or_else(detect_shell_from_env)
                .context(
                    "could not detect shell from $SHELL; pass --shell <bash|zsh|fish|elvish|powershell>",
                )?;
            print_completions(resolved_shell)
        }
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
        Commands::Config { command } => handle_config(command),
    }
}

fn detect_shell_from_env() -> Option<ShellArg> {
    let shell = std::env::var("SHELL").ok()?;
    let shell_name = Path::new(shell.trim()).file_name()?.to_str()?.to_ascii_lowercase();

    match shell_name.as_str() {
        "bash" => Some(ShellArg::Bash),
        "zsh" => Some(ShellArg::Zsh),
        "fish" => Some(ShellArg::Fish),
        "elvish" => Some(ShellArg::Elvish),
        "pwsh" | "powershell" => Some(ShellArg::Powershell),
        _ => None,
    }
}

fn completion_shell(shell: ShellArg) -> Shell {
    match shell {
        ShellArg::Bash => Shell::Bash,
        ShellArg::Zsh => Shell::Zsh,
        ShellArg::Fish => Shell::Fish,
        ShellArg::Elvish => Shell::Elvish,
        ShellArg::Powershell => Shell::PowerShell,
    }
}

fn print_completions(shell: ShellArg) -> Result<()> {
    let mut command = Cli::command();
    generate(
        completion_shell(shell),
        &mut command,
        "agitiser-notify",
        &mut io::stdout(),
    );
    Ok(())
}

fn handle_config(command: ConfigCommand) -> Result<()> {
    match command {
        ConfigCommand::Template { command } => handle_template_config(command),
        ConfigCommand::EventKind { command } => handle_event_kind_config(command),
    }
}

fn template_scope_label(agent: Option<Agent>) -> &'static str {
    match agent {
        Some(Agent::Claude) => "claude",
        Some(Agent::Codex) => "codex",
        Some(Agent::Generic) => "generic",
        None => "global",
    }
}

fn template_slot<'a>(templates: &'a state::TemplateConfig, agent: Option<Agent>) -> &'a Option<String> {
    match agent {
        Some(Agent::Claude) => &templates.agents.claude,
        Some(Agent::Codex) => &templates.agents.codex,
        Some(Agent::Generic) => &templates.agents.generic,
        None => &templates.global,
    }
}

fn template_slot_mut<'a>(
    templates: &'a mut state::TemplateConfig,
    agent: Option<Agent>,
) -> &'a mut Option<String> {
    match agent {
        Some(Agent::Claude) => &mut templates.agents.claude,
        Some(Agent::Codex) => &mut templates.agents.codex,
        Some(Agent::Generic) => &mut templates.agents.generic,
        None => &mut templates.global,
    }
}

fn event_kind_labels_slot<'a>(
    labels: &'a state::EventKindLabelsConfig,
    agent: Option<Agent>,
) -> &'a BTreeMap<String, String> {
    match agent {
        Some(Agent::Claude) => &labels.agents.claude,
        Some(Agent::Codex) => &labels.agents.codex,
        Some(Agent::Generic) => &labels.agents.generic,
        None => &labels.global,
    }
}

fn event_kind_labels_slot_mut<'a>(
    labels: &'a mut state::EventKindLabelsConfig,
    agent: Option<Agent>,
) -> &'a mut BTreeMap<String, String> {
    match agent {
        Some(Agent::Claude) => &mut labels.agents.claude,
        Some(Agent::Codex) => &mut labels.agents.codex,
        Some(Agent::Generic) => &mut labels.agents.generic,
        None => &mut labels.global,
    }
}

fn normalize_event_kind_key(key: &str) -> Result<String> {
    let normalized = key.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        bail!("event kind key must not be empty");
    }
    Ok(normalized)
}

fn handle_template_config(command: TemplateCommand) -> Result<()> {
    match command {
        TemplateCommand::Get { agent } => template_get(agent),
        TemplateCommand::Set { agent, value } => template_set(agent, value),
        TemplateCommand::Reset { agent } => template_reset(agent),
    }
}

fn handle_event_kind_config(command: EventKindCommand) -> Result<()> {
    match command {
        EventKindCommand::Get { agent, key } => event_kind_get(agent, &key),
        EventKindCommand::Set { agent, key, value } => event_kind_set(agent, &key, &value),
        EventKindCommand::Reset { agent, key } => event_kind_reset(agent, &key),
    }
}

fn template_get(agent: Option<Agent>) -> Result<()> {
    let state_path = paths::local_state_path()?;
    let local_state = state::load(&state_path)?;
    if let Some(value) = template_slot(&local_state.templates, agent) {
        println!("{value}");
    } else {
        println!("<unset>");
    }
    Ok(())
}

fn template_set(agent: Option<Agent>, value: String) -> Result<()> {
    agitiser_notify::template::validate_template(&value)?;

    let state_path = paths::local_state_path()?;
    let mut local_state = state::load(&state_path)?;
    let slot = template_slot_mut(&mut local_state.templates, agent);
    if slot.as_deref() == Some(value.as_str()) {
        println!("template for {} unchanged", template_scope_label(agent));
        return Ok(());
    }

    *slot = Some(value);
    state::save(&state_path, &local_state)?;
    println!("template for {} updated", template_scope_label(agent));
    Ok(())
}

fn template_reset(agent: Option<Agent>) -> Result<()> {
    let state_path = paths::local_state_path()?;
    let mut local_state = state::load(&state_path)?;
    let slot = template_slot_mut(&mut local_state.templates, agent);
    if slot.take().is_none() {
        println!("template for {} already unset", template_scope_label(agent));
        return Ok(());
    }

    state::save(&state_path, &local_state)?;
    println!("template for {} reset", template_scope_label(agent));
    Ok(())
}

fn event_kind_get(agent: Option<Agent>, key: &str) -> Result<()> {
    let state_path = paths::local_state_path()?;
    let local_state = state::load(&state_path)?;
    let normalized_key = normalize_event_kind_key(key)?;

    if let Some(value) = event_kind_labels_slot(&local_state.event_kind_labels, agent).get(&normalized_key) {
        println!("{value}");
    } else {
        println!("<unset>");
    }
    Ok(())
}

fn event_kind_set(agent: Option<Agent>, key: &str, value: &str) -> Result<()> {
    let normalized_key = normalize_event_kind_key(key)?;
    let trimmed_value = value.trim();
    if trimmed_value.is_empty() {
        bail!("event kind label value must not be empty");
    }

    let state_path = paths::local_state_path()?;
    let mut local_state = state::load(&state_path)?;
    let slot = event_kind_labels_slot_mut(&mut local_state.event_kind_labels, agent);
    let changed = match slot.get(&normalized_key) {
        Some(previous) if previous == trimmed_value => false,
        _ => {
            slot.insert(normalized_key, trimmed_value.to_string());
            true
        }
    };

    if !changed {
        println!(
            "event-kind label for {} unchanged",
            template_scope_label(agent)
        );
        return Ok(());
    }

    state::save(&state_path, &local_state)?;
    println!("event-kind label for {} updated", template_scope_label(agent));
    Ok(())
}

fn event_kind_reset(agent: Option<Agent>, key: &str) -> Result<()> {
    let normalized_key = normalize_event_kind_key(key)?;
    let state_path = paths::local_state_path()?;
    let mut local_state = state::load(&state_path)?;
    let slot = event_kind_labels_slot_mut(&mut local_state.event_kind_labels, agent);
    if slot.remove(&normalized_key).is_none() {
        println!(
            "event-kind label for {} already unset",
            template_scope_label(agent)
        );
        return Ok(());
    }

    state::save(&state_path, &local_state)?;
    println!("event-kind label for {} reset", template_scope_label(agent));
    Ok(())
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
    agent: Agent,
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

    let state_path = paths::local_state_path()?;
    let local_state = match state::load(&state_path) {
        Ok(state) => state,
        Err(error) => {
            if verbose {
                eprintln!(
                    "ingest: failed to load {} ({error:#}), using default template",
                    state_path.display()
                );
            }
            state::LocalState::default()
        }
    };

    speech::speak(&event, &local_state)?;
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
