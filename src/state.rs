use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalState {
    #[serde(default)]
    pub codex: CodexState,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexState {
    #[serde(default)]
    pub previous_notify: Option<Vec<String>>,
}

pub fn load(path: &Path) -> Result<LocalState> {
    if !path.exists() {
        return Ok(LocalState::default());
    }

    let raw =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    if raw.trim().is_empty() {
        return Ok(LocalState::default());
    }

    toml::from_str::<LocalState>(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))
}

pub fn save(path: &Path, state: &LocalState) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let raw = toml::to_string_pretty(state).context("failed to serialize local state")?;
    fs::write(path, format!("{raw}\n"))
        .with_context(|| format!("failed to write {}", path.display()))
}
