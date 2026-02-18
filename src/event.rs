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
    let hook_event = object.get("hook_event_name").and_then(Value::as_str)?;
    if hook_event != "Stop" {
        return None;
    }

    let cwd_str = object.get("cwd").and_then(Value::as_str);
    let cwd = cwd_str.map(PathBuf::from);
    Some(NormalizedEvent {
        agent: Agent::Claude,
        event_kind: "task-end".to_string(),
        project_name: project_name_from_cwd(cwd_str),
        cwd,
        raw_payload: payload,
    })
}

fn normalize_codex(payload: Value) -> Option<NormalizedEvent> {
    let object = payload.as_object()?;
    let kind = object.get("type").and_then(Value::as_str)?;
    if kind != "agent-turn-complete" {
        return None;
    }

    let cwd_str = object.get("cwd").and_then(Value::as_str);
    let cwd = cwd_str.map(PathBuf::from);
    Some(NormalizedEvent {
        agent: Agent::Codex,
        event_kind: "task-end".to_string(),
        project_name: project_name_from_cwd(cwd_str),
        cwd,
        raw_payload: payload,
    })
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
        assert_eq!(normalized.project_name, "agitiser");
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
        assert_eq!(normalized.project_name, "notiser");
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
