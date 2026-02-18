use agitiser_notify::agent::Agent;
use agitiser_notify::event::{normalize, project_name_from_cwd};
use agitiser_notify::integrations::{claude, codex};
use agitiser_notify::state::LocalState;
use serde_json::json;
use std::io::Write;
use tempfile::{NamedTempFile, TempDir};

// --- Claude setup/remove round-trip ---

#[test]
fn claude_setup_remove_round_trip() {
    let mut file = NamedTempFile::new().expect("temp file");
    write!(file, "{{}}").unwrap();
    let path = file.path().to_path_buf();

    let exe = std::path::Path::new("/tmp/agitiser-notify");
    assert!(claude::setup(&path, exe).expect("setup"));
    assert!(claude::is_configured(&path).expect("is_configured after setup"));

    assert!(claude::remove(&path).expect("remove"));
    assert!(!claude::is_configured(&path).expect("is_configured after remove"));
}

// --- Codex setup/remove round-trip ---

#[test]
fn codex_setup_remove_round_trip() {
    let mut file = NamedTempFile::new().expect("temp file");
    write!(file, "").unwrap();
    let path = file.path().to_path_buf();

    let exe = std::path::Path::new("/tmp/agitiser-notify");
    let mut state = LocalState::default();

    assert!(codex::setup(&path, &mut state, exe).expect("setup"));
    assert!(codex::is_configured(&path).expect("is_configured after setup"));

    assert!(codex::remove(&path, &mut state).expect("remove"));
    assert!(!codex::is_configured(&path).expect("is_configured after remove"));
}

// --- Claude empty Stop array cleanup ---

#[test]
fn claude_remove_cleans_up_empty_stop_array() {
    let mut settings = json!({
        "hooks": {
            "Stop": []
        }
    });

    assert!(!claude::apply_remove(&mut settings));
    assert!(settings.get("hooks").is_none());
}

#[test]
fn claude_remove_cleans_up_empty_stop_preserves_other_hooks() {
    let mut settings = json!({
        "hooks": {
            "Stop": [],
            "Other": [{"hooks": []}]
        }
    });

    assert!(!claude::apply_remove(&mut settings));
    // Stop should be removed, but Other should remain
    let hooks = settings.get("hooks").expect("hooks should remain");
    assert!(hooks.get("Stop").is_none());
    assert!(hooks.get("Other").is_some());
}

// --- Event normalization ---

#[test]
fn normalize_claude_stop_event() {
    let payload = json!({
        "session_id": "test-123",
        "hook_event_name": "Stop",
        "cwd": "/home/user/Projects/myapp"
    });

    let event = normalize(Agent::Claude, payload).expect("should normalize");
    assert_eq!(event.agent, Agent::Claude);
    assert_eq!(event.event_kind, "task-end");
    assert_eq!(event.project_name, "myapp");
}

#[test]
fn normalize_claude_ignores_non_stop() {
    let payload = json!({
        "hook_event_name": "SessionStart",
        "cwd": "/tmp"
    });

    assert!(normalize(Agent::Claude, payload).is_none());
}

#[test]
fn normalize_codex_turn_complete() {
    let payload = json!({
        "type": "agent-turn-complete",
        "cwd": "/home/user/Projects/backend"
    });

    let event = normalize(Agent::Codex, payload).expect("should normalize");
    assert_eq!(event.agent, Agent::Codex);
    assert_eq!(event.event_kind, "task-end");
    assert_eq!(event.project_name, "backend");
}

#[test]
fn normalize_codex_ignores_non_terminal() {
    let payload = json!({
        "type": "agent-turn-start",
        "cwd": "/tmp"
    });

    assert!(normalize(Agent::Codex, payload).is_none());
}

#[test]
fn normalize_generic_completed_event() {
    let payload = json!({
        "event_kind": "task-completed",
        "cwd": "/home/user/Projects/frontend"
    });

    let event = normalize(Agent::Generic, payload).expect("should normalize");
    assert_eq!(event.agent, Agent::Generic);
    assert_eq!(event.project_name, "frontend");
}

#[test]
fn normalize_generic_done_event() {
    let payload = json!({
        "type": "done",
        "project": "my-project"
    });

    let event = normalize(Agent::Generic, payload).expect("should normalize");
    assert_eq!(event.project_name, "my-project");
}

#[test]
fn normalize_generic_ignores_non_terminal() {
    let payload = json!({
        "event_kind": "task-started",
        "cwd": "/tmp"
    });

    assert!(normalize(Agent::Generic, payload).is_none());
}

#[test]
fn project_name_edge_cases() {
    assert_eq!(project_name_from_cwd(Some("/single")), "single");
    assert_eq!(project_name_from_cwd(Some("/a/b/c/")), "c");
    assert_eq!(project_name_from_cwd(Some("")), "unknown project");
    assert_eq!(project_name_from_cwd(Some("   ")), "unknown project");
}

// --- Binary smoke tests ---

#[test]
fn doctor_does_not_panic() {
    let bin = env!("CARGO_BIN_EXE_agitiser-notify");
    let output = std::process::Command::new(bin)
        .arg("doctor")
        .output()
        .expect("failed to run binary");

    // Doctor may exit 0 or 1 depending on whether spd-say is installed,
    // but it must not panic (exit code would be 101 on panic).
    assert_ne!(output.status.code(), Some(101), "binary panicked");
}

#[test]
fn ingest_with_payload_works() {
    let bin = env!("CARGO_BIN_EXE_agitiser-notify");
    let output = std::process::Command::new(bin)
        .args([
            "ingest",
            "--agent",
            "claude",
            "--verbose",
            "--payload",
            r#"{"hook_event_name":"SessionStart","cwd":"/tmp"}"#,
        ])
        .output()
        .expect("failed to run binary");

    // SessionStart is not a terminal event, so it should be skipped gracefully
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not a terminal event"));
}

#[test]
fn ingest_empty_payload_skips() {
    let bin = env!("CARGO_BIN_EXE_agitiser-notify");
    let output = std::process::Command::new(bin)
        .args(["ingest", "--agent", "generic", "--verbose", "--payload", ""])
        .output()
        .expect("failed to run binary");

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("empty payload"));
}

// --- Codex state round-trip with previous_notify ---

#[test]
fn codex_preserves_and_restores_previous_notify() {
    let mut file = NamedTempFile::new().expect("temp file");
    write!(file, r#"notify = ["notify-send", "Codex done"]"#).unwrap();
    let path = file.path().to_path_buf();

    let exe = std::path::Path::new("/tmp/agitiser-notify");
    let mut state = LocalState::default();

    // Setup should save the previous notify
    assert!(codex::setup(&path, &mut state, exe).expect("setup"));
    assert_eq!(
        state.codex.previous_notify,
        Some(vec!["notify-send".to_string(), "Codex done".to_string()])
    );

    // Remove should restore the previous notify
    assert!(codex::remove(&path, &mut state).expect("remove"));
    assert!(state.codex.previous_notify.is_none());
    assert!(codex::is_configured(&path).expect("should not be configured") == false);
}

fn temp_home() -> TempDir {
    tempfile::tempdir().expect("temp home dir")
}

#[test]
fn config_template_global_round_trip() {
    let bin = env!("CARGO_BIN_EXE_agitiser-notify");
    let home = temp_home();
    let template = "{{agent}} finished {{event_kind}} in {{project}}";

    let set_output = std::process::Command::new(bin)
        .args(["config", "template", "set", "--value", template])
        .env("HOME", home.path())
        .output()
        .expect("failed to run template set");
    assert!(set_output.status.success());

    let get_output = std::process::Command::new(bin)
        .args(["config", "template", "get"])
        .env("HOME", home.path())
        .output()
        .expect("failed to run template get");
    assert!(get_output.status.success());
    assert_eq!(String::from_utf8_lossy(&get_output.stdout).trim(), template);

    let reset_output = std::process::Command::new(bin)
        .args(["config", "template", "reset"])
        .env("HOME", home.path())
        .output()
        .expect("failed to run template reset");
    assert!(reset_output.status.success());

    let get_after_reset = std::process::Command::new(bin)
        .args(["config", "template", "get"])
        .env("HOME", home.path())
        .output()
        .expect("failed to run template get after reset");
    assert!(get_after_reset.status.success());
    assert_eq!(String::from_utf8_lossy(&get_after_reset.stdout).trim(), "<unset>");
}

#[test]
fn config_template_agent_override_round_trip() {
    let bin = env!("CARGO_BIN_EXE_agitiser-notify");
    let home = temp_home();
    let template = "Codex override {{project}}";

    let set_output = std::process::Command::new(bin)
        .args([
            "config",
            "template",
            "set",
            "--agent",
            "codex",
            "--value",
            template,
        ])
        .env("HOME", home.path())
        .output()
        .expect("failed to run template set");
    assert!(set_output.status.success());

    let get_agent_output = std::process::Command::new(bin)
        .args(["config", "template", "get", "--agent", "codex"])
        .env("HOME", home.path())
        .output()
        .expect("failed to run template get");
    assert!(get_agent_output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&get_agent_output.stdout).trim(),
        template
    );

    let get_global_output = std::process::Command::new(bin)
        .args(["config", "template", "get"])
        .env("HOME", home.path())
        .output()
        .expect("failed to run global template get");
    assert!(get_global_output.status.success());
    assert_eq!(String::from_utf8_lossy(&get_global_output.stdout).trim(), "<unset>");

    let reset_agent_output = std::process::Command::new(bin)
        .args(["config", "template", "reset", "--agent", "codex"])
        .env("HOME", home.path())
        .output()
        .expect("failed to run template reset");
    assert!(reset_agent_output.status.success());

    let get_agent_after_reset = std::process::Command::new(bin)
        .args(["config", "template", "get", "--agent", "codex"])
        .env("HOME", home.path())
        .output()
        .expect("failed to run template get after reset");
    assert!(get_agent_after_reset.status.success());
    assert_eq!(
        String::from_utf8_lossy(&get_agent_after_reset.stdout).trim(),
        "<unset>"
    );
}

#[test]
fn config_template_set_rejects_invalid_syntax() {
    let bin = env!("CARGO_BIN_EXE_agitiser-notify");
    let home = temp_home();

    let output = std::process::Command::new(bin)
        .args(["config", "template", "set", "--value", "{{#if"])
        .env("HOME", home.path())
        .output()
        .expect("failed to run template set");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid template syntax"));
}
