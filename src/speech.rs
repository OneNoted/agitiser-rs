use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use std::process::Command;

use crate::event::NormalizedEvent;
use crate::state::TemplateConfig;
use crate::template::render_announcement_message;

pub fn spd_say_path() -> Option<PathBuf> {
    which::which("spd-say").ok()
}

pub fn speak(event: &NormalizedEvent, templates: &TemplateConfig) -> Result<()> {
    let spd_say = spd_say_path()
        .context("spd-say not found in PATH; install speech-dispatcher")?;
    let message = render_announcement_message(event, templates);
    let status = Command::new(&spd_say)
        .arg(message)
        .status()
        .with_context(|| format!("failed to execute {}", spd_say.display()))?;
    if !status.success() {
        bail!("spd-say exited with {}", status);
    }

    Ok(())
}
