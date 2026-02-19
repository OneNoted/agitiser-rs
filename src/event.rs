use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::agent::Agent;

#[derive(Debug, Clone)]
pub struct NormalizedEvent {
    pub agent: Agent,
    pub event_kind: String,
    pub cwd: Option<PathBuf>,
    pub project_name: String,
    pub raw_payload: Value,
}

pub fn normalize(agent: Agent, payload: Value) -> Option<NormalizedEvent> {
    match agent {
        Agent::Claude => normalize_claude(payload),
        Agent::Codex => normalize_codex(payload),
        Agent::Generic => normalize_generic(payload),
    }
}

pub fn announcement_message(event: &NormalizedEvent) -> String {
    format!(
        "{} finished a {} in {}",
        event.agent.display_name(),
        event.event_kind,
        event.project_name
    )
}

pub fn project_name_from_cwd(cwd: Option<&str>) -> String {
    let cwd = match cwd.map(str::trim).filter(|s| !s.is_empty()) {
        Some(cwd) => cwd,
        None => return "unknown project".to_string(),
    };

    let trimmed = cwd.trim_end_matches('/');
    let candidate = if trimmed.is_empty() { cwd } else { trimmed };

    Path::new(candidate)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| candidate.to_string())
}

fn normalize_claude(payload: Value) -> Option<NormalizedEvent> {
    let object = payload.as_object()?;
    let event_kind = claude_event_kind(object)?;

    let cwd_str = object.get("cwd").and_then(Value::as_str);
    let cwd = cwd_str.map(PathBuf::from);
    Some(NormalizedEvent {
        agent: Agent::Claude,
        event_kind: event_kind.to_string(),
        project_name: project_name_from_cwd(cwd_str),
        cwd,
        raw_payload: payload,
    })
}

fn claude_event_kind(object: &serde_json::Map<String, Value>) -> Option<&'static str> {
    let hook_event = object.get("hook_event_name").and_then(Value::as_str)?;
    match hook_event {
        "Stop" => Some("task-end"),
        "SubagentStop" => Some("plan-end"),
        "PermissionRequest" if is_exit_plan_mode_request(object) => Some("plan-end"),
        _ => None,
    }
}

fn is_exit_plan_mode_request(object: &serde_json::Map<String, Value>) -> bool {
    object.get("tool_name").and_then(Value::as_str) == Some("ExitPlanMode")
        || object.get("tool").and_then(Value::as_str) == Some("ExitPlanMode")
        || object.get("query").and_then(Value::as_str) == Some("ExitPlanMode")
}

fn normalize_codex(payload: Value) -> Option<NormalizedEvent> {
    let object = payload.as_object()?;
    let kind = object.get("type").and_then(Value::as_str)?;
    let event_kind = codex_event_kind(kind)?;

    let cwd_str = object.get("cwd").and_then(Value::as_str);
    let cwd = cwd_str.map(PathBuf::from);
    Some(NormalizedEvent {
        agent: Agent::Codex,
        event_kind: event_kind.to_string(),
        project_name: project_name_from_cwd(cwd_str),
        cwd,
        raw_payload: payload,
    })
}

fn codex_event_kind(kind: &str) -> Option<&'static str> {
    match kind {
        "agent-turn-complete" => Some("task-end"),
        "agent-plan-complete" => Some("plan-end"),
        _ => None,
    }
}

fn normalize_generic(payload: Value) -> Option<NormalizedEvent> {
    let object = payload.as_object()?;

    let event_kind = object
        .get("event_kind")
        .or_else(|| object.get("event-kind"))
        .or_else(|| object.get("type"))
        .or_else(|| object.get("kind"))
        .or_else(|| object.get("event"))
        .and_then(Value::as_str)?
        .to_string();

    if !is_terminal_event(&event_kind) {
        return None;
    }

    let cwd = object.get("cwd").and_then(Value::as_str).map(PathBuf::from);
    let project_name = match cwd.as_ref().and_then(|p| p.to_str()) {
        Some(path) => project_name_from_cwd(Some(path)),
        None => object
            .get("project")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| "unknown project".to_string()),
    };

    Some(NormalizedEvent {
        agent: Agent::Generic,
        event_kind,
        cwd,
        project_name,
        raw_payload: payload,
    })
}

fn is_terminal_event(event_kind: &str) -> bool {
    let lowered = event_kind.to_ascii_lowercase();
    lowered.contains("complete")
        || lowered.contains("finish")
        || lowered.contains("done")
        || lowered.contains("stop")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn parses_claude_stop_event() {
        let payload = json!({
            "session_id": "abc",
            "hook_event_name": "Stop",
            "cwd": "/home/notes/Projects/agitiser"
        });

        let normalized = normalize(Agent::Claude, payload).expect("expected stop event");
        assert_eq!(normalized.event_kind, "task-end");
        assert_eq!(normalized.project_name, "agitiser");
    }

    #[test]
    fn parses_claude_subagent_stop_event() {
        let payload = json!({
            "session_id": "abc",
            "hook_event_name": "SubagentStop",
            "cwd": "/home/notes/Projects/agitiser"
        });

        let normalized =
            normalize(Agent::Claude, payload).expect("expected claude subagent stop event");
        assert_eq!(normalized.event_kind, "plan-end");
        assert_eq!(normalized.project_name, "agitiser");
    }

    #[test]
    fn parses_claude_exit_plan_mode_permission_request() {
        let payload = json!({
            "session_id": "abc",
            "hook_event_name": "PermissionRequest",
            "tool_name": "ExitPlanMode",
            "cwd": "/home/notes/Projects/agitiser"
        });

        let normalized = normalize(Agent::Claude, payload)
            .expect("expected claude plan completion permission request");
        assert_eq!(normalized.event_kind, "plan-end");
        assert_eq!(normalized.project_name, "agitiser");
    }

    #[test]
    fn ignores_non_exit_plan_mode_permission_requests() {
        let payload = json!({
            "hook_event_name": "PermissionRequest",
            "tool_name": "Bash",
            "cwd": "/tmp/demo"
        });

        assert!(normalize(Agent::Claude, payload).is_none());
    }

    #[test]
    fn ignores_non_terminal_claude_events() {
        let payload = json!({
            "hook_event_name": "SessionStart",
            "cwd": "/tmp/demo"
        });
        assert!(normalize(Agent::Claude, payload).is_none());
    }

    #[test]
    fn parses_codex_turn_complete_event() {
        let payload = json!({
            "type": "agent-turn-complete",
            "cwd": "/home/notes/Projects/notiser"
        });

        let normalized = normalize(Agent::Codex, payload).expect("expected codex completion");
        assert_eq!(normalized.event_kind, "task-end");
        assert_eq!(normalized.project_name, "notiser");
    }

    #[test]
    fn parses_codex_plan_complete_event() {
        let payload = json!({
            "type": "agent-plan-complete",
            "cwd": "/home/notes/Projects/notiser"
        });

        let normalized =
            normalize(Agent::Codex, payload).expect("expected codex planning completion");
        assert_eq!(normalized.event_kind, "plan-end");
        assert_eq!(normalized.project_name, "notiser");
    }

    #[test]
    fn ignores_unknown_codex_completion_events() {
        let payload = json!({
            "type": "something-complete",
            "cwd": "/tmp/demo"
        });

        assert!(normalize(Agent::Codex, payload).is_none());
    }

    #[test]
    fn extracts_project_name_from_cwd() {
        assert_eq!(
            project_name_from_cwd(Some("/home/notes/Projects/agitiser")),
            "agitiser"
        );
        assert_eq!(project_name_from_cwd(Some("/")), "/");
        assert_eq!(project_name_from_cwd(None), "unknown project");
    }
}
