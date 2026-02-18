use anyhow::{Context, Result};
use std::path::PathBuf;

pub fn home_dir() -> Result<PathBuf> {
    dirs::home_dir().context("could not resolve home directory")
}

pub fn claude_settings_path() -> Result<PathBuf> {
    Ok(home_dir()?.join(".claude").join("settings.json"))
}

pub fn codex_config_path() -> Result<PathBuf> {
    Ok(home_dir()?.join(".codex").join("config.toml"))
}

pub fn local_state_path() -> Result<PathBuf> {
    Ok(home_dir()?
        .join(".config")
        .join("agitiser-notify")
        .join("config.toml"))
}
