# agitiser-notify

[![CI](https://github.com/OneNoted/agitiser-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/OneNoted/agitiser-rs/actions/workflows/ci.yml)
[![Rust 1.85+](https://img.shields.io/badge/rust-1.85%2B-orange?logo=rust)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

`agitiser-notify` is a Rust CLI that announces agent completion events using `speech-dispatcher` (`spd-say`).

*the goofiest thing "I've" ever made...*

## Highlights

- Announces terminal task/planning events for Claude, Codex, and generic payloads.
- Supports automated setup with setup/remove for Claude and Codex.
- Supports configurable speech templates and event-kind labels.
- Supports toggling Claude subagent completion notifications.
- Includes shell completions and a `doctor` command for health checks.

## Requirements

- Rust `1.85+`
- Linux with `speech-dispatcher` installed (`spd-say` in `PATH`)
- macOS and Windows are not currently first-class supported backends

## Build

```bash
cargo build --release
```

## Quick Start

```bash
# Install managed integration for Claude + Codex
agitiser-notify setup

# Health check
agitiser-notify doctor

# Remove managed integration
agitiser-notify remove
```

## Common Commands

```bash
# Generate shell completions (stdout)
agitiser-notify completions --shell fish > ~/.config/fish/completions/agitiser-notify.fish
agitiser-notify completions --shell zsh > ~/.zfunc/_agitiser-notify
agitiser-notify completions --shell bash > /etc/bash_completion.d/agitiser-notify
agitiser-notify completions > /tmp/agitiser-notify.completion  # auto-detect via $SHELL

# Template configuration
agitiser-notify config template get
agitiser-notify config template set --value '{{agent}} finished a {{event_kind}} in the {{project}} project'
agitiser-notify config template get --agent codex
agitiser-notify config template set --agent codex --value 'Codex done in {{project}}'
agitiser-notify config template reset --agent codex

# Event-kind labels used by {{event_kind}}
agitiser-notify config event-kind set --key task-end --value task
agitiser-notify config event-kind set --agent codex --key task-end --value turn
agitiser-notify config event-kind set --agent codex --key plan-end --value plan
agitiser-notify config event-kind set --agent claude --key plan-end --value plan
agitiser-notify config event-kind get --key task-end
agitiser-notify config event-kind reset --agent codex --key task-end

# Claude subagent notification toggle (default: true)
agitiser-notify config subagent get
agitiser-notify config subagent set --enabled false
```

## Ingest API

```bash
# Claude
agitiser-notify ingest --agent claude '{"hook_event_name":"Stop","cwd":"/path/to/project"}'
agitiser-notify ingest --agent claude '{"hook_event_name":"SubagentStop","cwd":"/path/to/project"}'
agitiser-notify ingest --agent claude '{"hook_event_name":"PermissionRequest","tool_name":"ExitPlanMode","cwd":"/path/to/project"}'

# Codex
agitiser-notify ingest --agent codex '{"type":"agent-turn-complete","cwd":"/path/to/project"}'
agitiser-notify ingest --agent codex '{"type":"agent-plan-complete","cwd":"/path/to/project"}'

# Generic
agitiser-notify ingest --agent generic --payload '{"event_kind":"completed","cwd":"/path/to/project"}'
```

## Event Normalization

Claude mappings:
- `Stop` -> `task-end`
- `SubagentStop` -> `plan-end` (can be disabled with `config subagent set --enabled false`)
- `PermissionRequest` with `tool_name=ExitPlanMode` -> `plan-end`

Codex mappings:
- `agent-turn-complete` -> `task-end`
- `agent-plan-complete` -> `plan-end`

Built-in label map:
- `task-end` -> `task`
- `plan-end` -> `plan`

Built-in default plan announcement:
- `{{agent}} finished planning in {{project}}.`

## Template Variables

Templates use Handlebars-style placeholders:

- `{{agent}}` (display name, for example `Codex`)
- `{{event_kind}}` (friendly event-kind label with config and fallback humanization)
- `{{event_kind_raw}}` (raw normalized event kind, for example `task-end`)
- `{{project}}` (project name inferred from `cwd`)
- `{{cwd}}` (full current working directory when present)

Template precedence:
1. Per-agent override (`--agent claude|codex|generic`)
2. Global template
3. Built-in default message

Event-kind label precedence for `{{event_kind}}`:
1. Per-agent label from `config event-kind ... --agent ...`
2. Global label from `config event-kind ...`
3. Built-in label map (`task-end` -> `task`, `plan-end` -> `plan`)
4. Built-in humanized fallback (for example `task-completed` -> `task completed`)

## OpenCode Manual Integration

OpenCode auto-setup is not implemented in this release.

Use a manual hook/plugin command that calls:

```bash
agitiser-notify ingest --agent generic --payload '<json>'
```

Payload must include at minimum:

```json
{
  "event_kind": "completed",
  "cwd": "/absolute/path/to/project"
}
```

## Development

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```
