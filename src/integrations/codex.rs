use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use toml_edit::{Array, DocumentMut, Item, Value};

use crate::state::LocalState;

const SOURCE_VALUE: &str = "codex-notify";

pub fn managed_notify_command(executable_path: &Path) -> Vec<String> {
    vec![
        executable_path.to_string_lossy().to_string(),
        "ingest".to_string(),
        "--agent".to_string(),
        "codex".to_string(),
        "--source".to_string(),
        SOURCE_VALUE.to_string(),
    ]
}

pub fn setup(config_path: &Path, state: &mut LocalState, executable_path: &Path) -> Result<bool> {
    let mut doc = load_config(config_path)?;
    let desired = managed_notify_command(executable_path);
    let changed = apply_setup(&mut doc, state, &desired);
    if changed {
        write_config(config_path, &doc)?;
    }
    Ok(changed)
}

pub fn remove(config_path: &Path, state: &mut LocalState) -> Result<bool> {
    if !config_path.exists() {
        return Ok(false);
    }

    let mut doc = load_config(config_path)?;
    let changed = apply_remove(&mut doc, state);
    if changed {
        write_config(config_path, &doc)?;
    }
    Ok(changed)
}

pub fn is_configured(config_path: &Path) -> Result<bool> {
    if !config_path.exists() {
        return Ok(false);
    }

    let doc = load_config(config_path)?;
    Ok(extract_notify(&doc)
        .map(|n| is_managed_notify(&n))
        .unwrap_or(false))
}

pub fn apply_setup(doc: &mut DocumentMut, state: &mut LocalState, desired: &[String]) -> bool {
    let existing = extract_notify(doc);
    match existing {
        Some(ref notify) if notify == desired => false,
        Some(ref notify) if is_managed_notify(notify) => {
            set_notify(doc, desired);
            true
        }
        Some(notify) => {
            if state.codex.previous_notify.is_none() {
                state.codex.previous_notify = Some(notify);
            }
            set_notify(doc, desired);
            true
        }
        None => {
            set_notify(doc, desired);
            true
        }
    }
}

pub fn apply_remove(doc: &mut DocumentMut, state: &mut LocalState) -> bool {
    let Some(existing) = extract_notify(doc) else {
        return false;
    };

    if !is_managed_notify(&existing) {
        return false;
    }

    if let Some(previous) = state.codex.previous_notify.take() {
        set_notify(doc, &previous);
    } else {
        remove_notify(doc);
    }
    true
}

fn is_managed_notify(notify: &[String]) -> bool {
    let has = |needle: &str| notify.iter().any(|s| s == needle);
    has("ingest") && has("--agent") && has("codex") && has("--source") && has(SOURCE_VALUE)
}

fn load_config(config_path: &Path) -> Result<DocumentMut> {
    if !config_path.exists() {
        return Ok(DocumentMut::new());
    }

    let raw = fs::read_to_string(config_path)
        .with_context(|| format!("failed to read {}", config_path.display()))?;
    if raw.trim().is_empty() {
        return Ok(DocumentMut::new());
    }

    raw.parse::<DocumentMut>()
        .with_context(|| format!("failed to parse {}", config_path.display()))
}

fn write_config(config_path: &Path, doc: &DocumentMut) -> Result<()> {
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    fs::write(config_path, doc.to_string())
        .with_context(|| format!("failed to write {}", config_path.display()))
}

fn extract_notify(doc: &DocumentMut) -> Option<Vec<String>> {
    let notify = doc.get("notify")?;
    let array = notify.as_array()?;
    array
        .iter()
        .map(|item| item.as_str().map(ToOwned::to_owned))
        .collect()
}

fn set_notify(doc: &mut DocumentMut, command: &[String]) {
    let mut array = Array::default();
    for part in command {
        array.push(part.as_str());
    }
    doc["notify"] = Item::Value(Value::Array(array));
}

fn remove_notify(doc: &mut DocumentMut) {
    doc.remove("notify");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{CodexState, LocalState};

    #[test]
    fn setup_saves_previous_notify_and_sets_managed_command() {
        let mut doc =
            r#"notify = ["notify-send", "Codex"]"#.parse::<DocumentMut>().expect("valid toml");
        let mut state = LocalState {
            codex: CodexState::default(),
            templates: crate::state::TemplateConfig::default(),
        };
        let managed = vec![
            "/tmp/agitiser-notify".to_string(),
            "ingest".to_string(),
            "--agent".to_string(),
            "codex".to_string(),
            "--source".to_string(),
            "codex-notify".to_string(),
        ];

        assert!(apply_setup(&mut doc, &mut state, &managed));
        assert_eq!(
            state.codex.previous_notify,
            Some(vec!["notify-send".to_string(), "Codex".to_string()])
        );
        assert_eq!(extract_notify(&doc).as_deref(), Some(managed.as_slice()));
    }

    #[test]
    fn remove_restores_previous_notify() {
        let mut doc = r#"notify = ["/tmp/agitiser-notify", "ingest", "--agent", "codex", "--source", "codex-notify"]"#
            .parse::<DocumentMut>()
            .expect("valid toml");
        let mut state = LocalState {
            codex: CodexState {
                previous_notify: Some(vec!["notify-send".to_string(), "Codex".to_string()]),
            },
            templates: crate::state::TemplateConfig::default(),
        };

        assert!(apply_remove(&mut doc, &mut state));
        assert_eq!(
            extract_notify(&doc),
            Some(vec!["notify-send".to_string(), "Codex".to_string()])
        );
        assert!(state.codex.previous_notify.is_none());
    }
}
