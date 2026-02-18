# agitiser-notify

Rust CLI that announces agent task completion with `speech-dispatcher` (`spd-say`), including the project name.

## Features

- Announces only end-of-task events.
- Speaks `"<Agent> finished a <event_kind> task in <project>"` by default.
- Supports configurable announcement templates (global and per-agent).
- Auto-setup for:
  - Claude Code (`~/.claude/settings.json` Stop hook)
  - Codex (`~/.codex/config.toml` notify command)
- Manual integration path for OpenCode.

## Build

```bash
cargo build --release
```

## Usage

```bash
# Install hooks/config for Claude + Codex
agitiser-notify setup

# Remove managed Claude + Codex integration
agitiser-notify remove

# Health checks
agitiser-notify doctor

# Manage spoken message templates
agitiser-notify config template get
agitiser-notify config template set --value '{{agent}} finished {{event_kind}} in {{project}}'
agitiser-notify config template get --agent codex
agitiser-notify config template set --agent codex --value 'Codex done in {{project}}'
agitiser-notify config template reset --agent codex
```

## Ingest API

```bash
# Agent-specific parsing
agitiser-notify ingest --agent claude
agitiser-notify ingest --agent codex '{"type":"agent-turn-complete","cwd":"/path/to/project"}'

# Generic payload mode
agitiser-notify ingest --agent generic --payload '{"event_kind":"completed","cwd":"/path/to/project"}'
```

## OpenCode Manual Integration (v1)

OpenCode auto-setup is not implemented in this release.

Use a manual hook/plugin command that calls:

```bash
agitiser-notify ingest --agent generic --payload '<json>'
```

Where payload contains at minimum:

```json
{
  "event_kind": "completed",
  "cwd": "/absolute/path/to/project"
}
```

## Template Variables

Templates use Handlebars-style placeholders:

- `{{agent}}` (display name, for example `Codex`)
- `{{event_kind}}` (normalized terminal event kind, for example `task-end`)
- `{{project}}` (project name inferred from `cwd`)
- `{{cwd}}` (full current working directory when present)

Template precedence is:

1. Per-agent override (`--agent claude|codex|generic`)
2. Global template
3. Built-in default message
