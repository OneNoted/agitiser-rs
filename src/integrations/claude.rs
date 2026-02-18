use anyhow::{Context, Result};
use serde_json::{json, Map, Value};
use std::fs;
use std::path::Path;

const STOP_EVENT: &str = "Stop";
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

    let mut desired_exists = false;
    for entry in stop_hooks.iter_mut() {
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

                if hook_command == command && !desired_exists {
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

    let original_len = stop_hooks.len();
    stop_hooks.retain(|entry| {
        entry
            .get("hooks")
            .and_then(Value::as_array)
            .map(|hooks| !hooks.is_empty())
            .unwrap_or(true)
    });
    if stop_hooks.len() != original_len {
        changed = true;
    }

    if !desired_exists {
        stop_hooks.push(json!({
            "matcher": "*",
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

pub fn apply_remove(settings: &mut Value) -> bool {
    let mut changed = false;

    // Check for empty Stop array upfront and clean it up
    if let Some(is_empty) = settings
        .get("hooks")
        .and_then(|h| h.get(STOP_EVENT))
        .and_then(Value::as_array)
        .map(|a| a.is_empty())
    {
        if is_empty {
            let root_obj = settings.as_object_mut().unwrap();
            let hooks_obj = root_obj.get_mut("hooks").unwrap().as_object_mut().unwrap();
            hooks_obj.remove(STOP_EVENT);
            if hooks_obj.is_empty() {
                root_obj.remove("hooks");
            }
            return false;
        }
    }

    let root_obj = match settings.as_object_mut() {
        Some(root_obj) => root_obj,
        None => return false,
    };

    let hooks_obj = match root_obj.get_mut("hooks").and_then(Value::as_object_mut) {
        Some(hooks_obj) => hooks_obj,
        None => return false,
    };

    let stop_hooks = match hooks_obj.get_mut(STOP_EVENT).and_then(Value::as_array_mut) {
        Some(stop_hooks) => stop_hooks,
        None => return false,
    };

    for entry in stop_hooks.iter_mut() {
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

    let original_len = stop_hooks.len();
    stop_hooks.retain(|entry| {
        entry
            .get("hooks")
            .and_then(Value::as_array)
            .map(|hooks| !hooks.is_empty())
            .unwrap_or(true)
    });
    if stop_hooks.len() != original_len {
        changed = true;
    }

    if stop_hooks.is_empty() {
        hooks_obj.remove(STOP_EVENT);
        changed = true;
    }
    if hooks_obj.is_empty() {
        root_obj.remove("hooks");
        changed = true;
    }

    changed
}

fn has_managed_hook(settings: &Value) -> bool {
    settings
        .get("hooks")
        .and_then(|hooks| hooks.get(STOP_EVENT))
        .and_then(Value::as_array)
        .map(|entries| {
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
        })
        .unwrap_or(false)
}

fn is_managed_command(command: &str) -> bool {
    command.contains("ingest --agent claude") && command.contains(SOURCE_MARKER)
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

    #[test]
    fn setup_is_idempotent() {
        let command =
            "AGITISER_NOTIFY=1 '/tmp/agitiser-notify' ingest --agent claude --source claude-hook";
        let mut settings = json!({});

        assert!(apply_setup(&mut settings, command));
        assert!(!apply_setup(&mut settings, command));
    }

    #[test]
    fn remove_keeps_unmanaged_stop_hooks() {
        let mut settings = json!({
            "hooks": {
                "Stop": [
                    {
                        "hooks": [
                            {"type": "command", "command": "echo custom"},
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
    }

    #[test]
    fn remove_cleans_up_empty_stop_array() {
        let mut settings = json!({
            "hooks": {
                "Stop": []
            }
        });

        // Should return false (no managed hooks were removed) but clean up the empty array
        assert!(!apply_remove(&mut settings));
        assert!(settings.get("hooks").is_none());
    }
}
