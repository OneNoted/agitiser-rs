use anyhow::{Context, Result};
use serde_json::{json, Map, Value};
use std::fs;
use std::path::Path;

const STOP_EVENT: &str = "Stop";
const SUBAGENT_STOP_EVENT: &str = "SubagentStop";
const PERMISSION_REQUEST_EVENT: &str = "PermissionRequest";
const PERMISSION_REQUEST_MATCHER: &str = "ExitPlanMode";
const SOURCE_MARKER: &str = "--source claude-hook";

pub fn managed_command(executable_path: &Path) -> String {
    let quoted_exe = shell_quote(executable_path.to_string_lossy().as_ref());
    format!(
        "AGITISER_NOTIFY=1 {quoted_exe} ingest --agent claude --source claude-hook >/dev/null 2>&1"
    )
}

pub fn setup(settings_path: &Path, executable_path: &Path) -> Result<bool> {
    let mut settings = load_settings(settings_path)?;
    let command = managed_command(executable_path);
    let changed = apply_setup(&mut settings, &command);
    if changed {
        write_settings(settings_path, &settings)?;
    }
    Ok(changed)
}

pub fn remove(settings_path: &Path) -> Result<bool> {
    if !settings_path.exists() {
        return Ok(false);
    }

    let mut settings = load_settings(settings_path)?;
    let changed = apply_remove(&mut settings);
    if changed {
        write_settings(settings_path, &settings)?;
    }
    Ok(changed)
}

pub fn is_configured(settings_path: &Path) -> Result<bool> {
    if !settings_path.exists() {
        return Ok(false);
    }

    let settings = load_settings(settings_path)?;
    Ok(has_managed_hook(&settings))
}

pub fn apply_setup(settings: &mut Value, command: &str) -> bool {
    let mut changed = false;

    let root_obj = ensure_root_object(settings);
    let hooks_obj = ensure_object_entry(root_obj, "hooks");
    let stop_hooks = ensure_array_entry(hooks_obj, STOP_EVENT);
    if ensure_managed_hook(stop_hooks, command, "*") {
        changed = true;
    }

    let subagent_stop_hooks = ensure_array_entry(hooks_obj, SUBAGENT_STOP_EVENT);
    if ensure_managed_hook(subagent_stop_hooks, command, "*") {
        changed = true;
    }

    let permission_request_hooks = ensure_array_entry(hooks_obj, PERMISSION_REQUEST_EVENT);
    if ensure_managed_hook(
        permission_request_hooks,
        command,
        PERMISSION_REQUEST_MATCHER,
    ) {
        changed = true;
    }

    changed
}

pub fn apply_remove(settings: &mut Value) -> bool {
    let mut changed = false;

    let root_obj = match settings.as_object_mut() {
        Some(root_obj) => root_obj,
        None => return false,
    };

    let hooks_obj = match root_obj.get_mut("hooks").and_then(Value::as_object_mut) {
        Some(hooks_obj) => hooks_obj,
        None => return false,
    };

    for event in [STOP_EVENT, SUBAGENT_STOP_EVENT, PERMISSION_REQUEST_EVENT] {
        let mut remove_event = false;
        if let Some(event_hooks) = hooks_obj.get_mut(event).and_then(Value::as_array_mut) {
            if remove_managed_hooks(event_hooks) {
                changed = true;
            }
            remove_event = event_hooks.is_empty();
        }
        if remove_event {
            hooks_obj.remove(event);
            changed = true;
        }
    }

    if hooks_obj.is_empty() {
        root_obj.remove("hooks");
        changed = true;
    }

    changed
}

fn has_managed_hook(settings: &Value) -> bool {
    let hooks_obj = match settings.get("hooks").and_then(Value::as_object) {
        Some(hooks_obj) => hooks_obj,
        None => return false,
    };

    [STOP_EVENT, SUBAGENT_STOP_EVENT, PERMISSION_REQUEST_EVENT]
        .iter()
        .any(|event| {
            hooks_obj
                .get(*event)
                .and_then(Value::as_array)
                .map(|entries| event_has_managed_hook(entries))
                .unwrap_or(false)
        })
}

fn is_managed_command(command: &str) -> bool {
    command.contains("ingest --agent claude") && command.contains(SOURCE_MARKER)
}

fn event_has_managed_hook(entries: &[Value]) -> bool {
    entries.iter().any(|entry| {
        entry
            .get("hooks")
            .and_then(Value::as_array)
            .map(|hooks| {
                hooks.iter().any(|hook| {
                    hook.get("command")
                        .and_then(Value::as_str)
                        .map(is_managed_command)
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    })
}

fn ensure_managed_hook(event_hooks: &mut Vec<Value>, command: &str, matcher: &str) -> bool {
    let mut changed = false;
    let mut desired_exists = false;

    for entry in event_hooks.iter_mut() {
        let entry_matcher_matches = entry
            .get("matcher")
            .and_then(Value::as_str)
            .unwrap_or_default()
            == matcher;
        if let Some(hooks_array) = entry
            .as_object_mut()
            .and_then(|obj| obj.get_mut("hooks"))
            .and_then(Value::as_array_mut)
        {
            let original_len = hooks_array.len();
            hooks_array.retain(|hook| {
                let hook_command = hook
                    .get("command")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();

                if !is_managed_command(&hook_command) {
                    return true;
                }

                if hook_command == command && entry_matcher_matches && !desired_exists {
                    desired_exists = true;
                    true
                } else {
                    false
                }
            });

            if hooks_array.len() != original_len {
                changed = true;
            }
        }
    }

    let original_len = event_hooks.len();
    event_hooks.retain(|entry| {
        entry
            .get("hooks")
            .and_then(Value::as_array)
            .map(|hooks| !hooks.is_empty())
            .unwrap_or(true)
    });
    if event_hooks.len() != original_len {
        changed = true;
    }

    if !desired_exists {
        event_hooks.push(json!({
            "matcher": matcher,
            "hooks": [
                {
                    "type": "command",
                    "command": command,
                }
            ]
        }));
        changed = true;
    }

    changed
}

fn remove_managed_hooks(event_hooks: &mut Vec<Value>) -> bool {
    let mut changed = false;

    for entry in event_hooks.iter_mut() {
        if let Some(hooks_array) = entry
            .as_object_mut()
            .and_then(|obj| obj.get_mut("hooks"))
            .and_then(Value::as_array_mut)
        {
            let original_len = hooks_array.len();
            hooks_array.retain(|hook| {
                let hook_command = hook
                    .get("command")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                !is_managed_command(hook_command)
            });
            if hooks_array.len() != original_len {
                changed = true;
            }
        }
    }

    let original_len = event_hooks.len();
    event_hooks.retain(|entry| {
        entry
            .get("hooks")
            .and_then(Value::as_array)
            .map(|hooks| !hooks.is_empty())
            .unwrap_or(true)
    });
    if event_hooks.len() != original_len {
        changed = true;
    }

    changed
}

fn load_settings(settings_path: &Path) -> Result<Value> {
    if !settings_path.exists() {
        return Ok(json!({}));
    }

    let raw = fs::read_to_string(settings_path)
        .with_context(|| format!("failed to read {}", settings_path.display()))?;
    if raw.trim().is_empty() {
        return Ok(json!({}));
    }

    serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse {}", settings_path.display()))
}

fn write_settings(settings_path: &Path, settings: &Value) -> Result<()> {
    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let raw =
        serde_json::to_string_pretty(settings).context("failed to serialize settings.json")?;
    fs::write(settings_path, format!("{raw}\n"))
        .with_context(|| format!("failed to write {}", settings_path.display()))
}

fn ensure_root_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = json!({});
    }
    value.as_object_mut().expect("root should be an object")
}

fn ensure_object_entry<'a>(
    obj: &'a mut Map<String, Value>,
    key: &str,
) -> &'a mut Map<String, Value> {
    let value = obj.entry(key.to_string()).or_insert_with(|| json!({}));
    if !value.is_object() {
        *value = json!({});
    }
    value.as_object_mut().expect("entry should be an object")
}

fn ensure_array_entry<'a>(obj: &'a mut Map<String, Value>, key: &str) -> &'a mut Vec<Value> {
    let value = obj.entry(key.to_string()).or_insert_with(|| json!([]));
    if !value.is_array() {
        *value = json!([]);
    }
    value.as_array_mut().expect("entry should be an array")
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn managed_hook_count(settings: &Value, event: &str) -> usize {
        settings["hooks"][event]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.get("hooks").and_then(Value::as_array))
            .flatten()
            .filter_map(|hook| hook.get("command").and_then(Value::as_str))
            .filter(|command| is_managed_command(command))
            .count()
    }

    #[test]
    fn setup_is_idempotent() {
        let command =
            "AGITISER_NOTIFY=1 '/tmp/agitiser-notify' ingest --agent claude --source claude-hook";
        let mut settings = json!({});

        assert!(
            apply_setup(&mut settings, command),
            "first setup should change"
        );
        assert_eq!(managed_hook_count(&settings, STOP_EVENT), 1);
        assert_eq!(managed_hook_count(&settings, SUBAGENT_STOP_EVENT), 1);
        assert_eq!(managed_hook_count(&settings, PERMISSION_REQUEST_EVENT), 1);
        assert_eq!(
            settings["hooks"][PERMISSION_REQUEST_EVENT][0]["matcher"],
            PERMISSION_REQUEST_MATCHER
        );
        assert!(
            !apply_setup(&mut settings, command),
            "second setup should be idempotent"
        );
        assert_eq!(managed_hook_count(&settings, STOP_EVENT), 1);
        assert_eq!(managed_hook_count(&settings, SUBAGENT_STOP_EVENT), 1);
        assert_eq!(managed_hook_count(&settings, PERMISSION_REQUEST_EVENT), 1);
        assert_eq!(
            settings["hooks"][PERMISSION_REQUEST_EVENT][0]["matcher"],
            PERMISSION_REQUEST_MATCHER
        );
    }

    #[test]
    fn remove_keeps_unmanaged_hooks_for_stop_and_subagent_stop() {
        let mut settings = json!({
            "hooks": {
                "Stop": [
                    {
                        "hooks": [
                            {"type": "command", "command": "echo custom"},
                            {"type": "command", "command": "AGITISER_NOTIFY=1 '/tmp/agitiser-notify' ingest --agent claude --source claude-hook"}
                        ]
                    }
                ],
                "SubagentStop": [
                    {
                        "hooks": [
                            {"type": "command", "command": "echo custom-subagent"},
                            {"type": "command", "command": "AGITISER_NOTIFY=1 '/tmp/agitiser-notify' ingest --agent claude --source claude-hook"}
                        ]
                    }
                ],
                "PermissionRequest": [
                    {
                        "matcher": "ExitPlanMode",
                        "hooks": [
                            {"type": "command", "command": "echo custom-permission"},
                            {"type": "command", "command": "AGITISER_NOTIFY=1 '/tmp/agitiser-notify' ingest --agent claude --source claude-hook"}
                        ]
                    }
                ]
            }
        });

        assert!(apply_remove(&mut settings));
        let stop_hooks = settings["hooks"]["Stop"][0]["hooks"]
            .as_array()
            .expect("stop hook array");
        assert_eq!(stop_hooks.len(), 1);
        assert_eq!(stop_hooks[0]["command"], "echo custom");

        let subagent_stop_hooks = settings["hooks"]["SubagentStop"][0]["hooks"]
            .as_array()
            .expect("subagent stop hook array");
        assert_eq!(subagent_stop_hooks.len(), 1);
        assert_eq!(subagent_stop_hooks[0]["command"], "echo custom-subagent");

        let permission_hooks = settings["hooks"]["PermissionRequest"][0]["hooks"]
            .as_array()
            .expect("permission request hook array");
        assert_eq!(permission_hooks.len(), 1);
        assert_eq!(permission_hooks[0]["command"], "echo custom-permission");
    }

    #[test]
    fn remove_cleans_up_empty_stop_and_subagent_stop_arrays() {
        let mut settings = json!({
            "hooks": {
                "Stop": [],
                "SubagentStop": [],
                "PermissionRequest": []
            }
        });

        assert!(apply_remove(&mut settings));
        assert!(settings.get("hooks").is_none());
    }
}
