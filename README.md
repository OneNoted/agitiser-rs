# agitiser-notify

Rust CLI that announces agent task completion with `speech-dispatcher` (`spd-say`), including configurable message. 

*this may be the most horrible thing I've ever made truly ai slop ;-; but hey it works!!*

## Features

- Announces only end-of-task events.
- Speaks `"<Agent> finished a <event_kind> in the <project> project"` by default.
- Supports configurable announcement templates (global and per-agent).
- Supports configurable event-kind labels (for example `task-end -> task`).
- Supports toggling Claude subagent completion notifications.
- Auto-setup for:
  - Claude Code (`~/.claude/settings.json` managed completion hooks)
  - Codex (`~/.codex/config.toml` notify command)
- Manual integration path for OpenCode.

## Platform Support

- Linux is the primary supported platform for this release.
- `speech-dispatcher` (`spd-say`) must be available in `PATH`.
- macOS and Windows are not currently first-class supported notifier backends.

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

# Generate shell completions (stdout)
agitiser-notify completions --shell fish > ~/.config/fish/completions/agitiser-notify.fish
agitiser-notify completions --shell zsh > ~/.zfunc/_agitiser-notify
agitiser-notify completions --shell bash > /etc/bash_completion.d/agitiser-notify
# Auto-detect shell from $SHELL
agitiser-notify completions > /tmp/agitiser-notify.completion

# Manage spoken message templates
agitiser-notify config template get
agitiser-notify config template set --value '{{agent}} finished a {{event_kind}} in the {{project}} project'
agitiser-notify config template get --agent codex
agitiser-notify config template set --agent codex --value 'Codex done in {{project}}'
agitiser-notify config template reset --agent codex

# Manage event-kind labels used by {{event_kind}}
agitiser-notify config event-kind set --key task-end --value task
agitiser-notify config event-kind set --agent codex --key task-end --value turn
agitiser-notify config event-kind set --agent codex --key plan-end --value plan
agitiser-notify config event-kind set --agent claude --key plan-end --value plan

# Toggle Claude subagent completion notifications (default: true)
agitiser-notify config subagent get
agitiser-notify config subagent set --enabled false

agitiser-notify config event-kind get --key task-end
agitiser-notify config event-kind reset --agent codex --key task-end
```

## Ingest API

```bash
# Agent-specific parsing
agitiser-notify ingest --agent claude
agitiser-notify ingest --agent claude '{"hook_event_name":"Stop","cwd":"/path/to/project"}'
agitiser-notify ingest --agent claude '{"hook_event_name":"SubagentStop","cwd":"/path/to/project"}'
agitiser-notify ingest --agent claude '{"hook_event_name":"PermissionRequest","tool_name":"ExitPlanMode","cwd":"/path/to/project"}'
agitiser-notify ingest --agent codex '{"type":"agent-turn-complete","cwd":"/path/to/project"}'
agitiser-notify ingest --agent codex '{"type":"agent-plan-complete","cwd":"/path/to/project"}'

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
- `{{event_kind}}` (friendly event-kind label with config and fallback humanization)
- `{{event_kind_raw}}` (raw normalized event kind, for example `task-end`)
- `{{project}}` (project name inferred from `cwd`)
- `{{cwd}}` (full current working directory when present)

Template precedence is:

1. Per-agent override (`--agent claude|codex|generic`)
2. Global template
3. Built-in default message

Event-kind label precedence for `{{event_kind}}` is:

1. Per-agent label from `config event-kind ... --agent ...`
2. Global label from `config event-kind ...`
3. Built-in label map (`task-end` -> `task`, `plan-end` -> `plan`)
4. Built-in humanized fallback (for example `task-completed` -> `task completed`)

For Claude events, normalization maps:
- `Stop` -> `task-end`
- `SubagentStop` -> `plan-end` (can be disabled with `config subagent set --enabled false`)
- `PermissionRequest` with `tool_name=ExitPlanMode` -> `plan-end`

For Codex events, normalization maps:
- `agent-turn-complete` -> `task-end`
- `agent-plan-complete` -> `plan-end`

Built-in labels map:
- `task-end` -> `task`
- `plan-end` -> `plan`

Built-in default plan announcement:
- `{{agent}} finished planning in {{project}}.`
