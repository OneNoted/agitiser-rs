use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalState {
    #[serde(default)]
    pub codex: CodexState,
    #[serde(default)]
    pub templates: TemplateConfig,
    #[serde(default)]
    pub event_kind_labels: EventKindLabelsConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexState {
    #[serde(default)]
    pub previous_notify: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TemplateConfig {
    #[serde(default)]
    pub global: Option<String>,
    #[serde(default)]
    pub agents: AgentTemplateConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentTemplateConfig {
    #[serde(default)]
    pub claude: Option<String>,
    #[serde(default)]
    pub codex: Option<String>,
    #[serde(default)]
    pub generic: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventKindLabelsConfig {
    #[serde(default)]
    pub global: BTreeMap<String, String>,
    #[serde(default)]
    pub agents: AgentEventKindLabelsConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentEventKindLabelsConfig {
    #[serde(default)]
    pub claude: BTreeMap<String, String>,
    #[serde(default)]
    pub codex: BTreeMap<String, String>,
    #[serde(default)]
    pub generic: BTreeMap<String, String>,
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
