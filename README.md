# agitiser-notify

Rust CLI that announces agent task completion with `speech-dispatcher` (`spd-say`), including the project name.

## Features

- Announces only end-of-task events.
- Speaks `"<Agent> finished task in <project>"`.
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
