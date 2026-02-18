use anyhow::{Context, Result};
use handlebars::Handlebars;
use serde::Serialize;

use crate::agent::Agent;
use crate::event::{announcement_message, NormalizedEvent};
use crate::state::TemplateConfig;

const TEMPLATE_NAME: &str = "announcement";

#[derive(Debug, Serialize)]
struct AnnouncementContext<'a> {
    agent: &'a str,
    event_kind: &'a str,
    project: &'a str,
    cwd: &'a str,
}

fn agent_template<'a>(templates: &'a TemplateConfig, agent: Agent) -> Option<&'a str> {
    match agent {
        Agent::Claude => templates.agents.claude.as_deref(),
        Agent::Codex => templates.agents.codex.as_deref(),
        Agent::Generic => templates.agents.generic.as_deref(),
    }
}

fn normalize_template(value: Option<&str>) -> Option<&str> {
    value.filter(|candidate| !candidate.trim().is_empty())
}

fn context_from_event(event: &NormalizedEvent) -> AnnouncementContext<'_> {
    let cwd = event
        .cwd
        .as_ref()
        .and_then(|path| path.to_str())
        .unwrap_or_default();

    AnnouncementContext {
        agent: event.agent.display_name(),
        event_kind: &event.event_kind,
        project: &event.project_name,
        cwd,
    }
}

fn render_template(template: &str, event: &NormalizedEvent) -> Option<String> {
    let mut renderer = Handlebars::new();
    renderer.set_strict_mode(false);

    if renderer
        .register_template_string(TEMPLATE_NAME, template)
        .is_err()
    {
        return None;
    }

    renderer
        .render(TEMPLATE_NAME, &context_from_event(event))
        .ok()
        .filter(|rendered| !rendered.trim().is_empty())
}

pub fn validate_template(template: &str) -> Result<()> {
    let mut renderer = Handlebars::new();
    renderer.set_strict_mode(false);
    renderer
        .register_template_string(TEMPLATE_NAME, template)
        .context("invalid template syntax")?;
    Ok(())
}

pub fn resolve_template<'a>(templates: &'a TemplateConfig, agent: Agent) -> Option<&'a str> {
    normalize_template(agent_template(templates, agent))
        .or_else(|| normalize_template(templates.global.as_deref()))
}

pub fn render_announcement_message(event: &NormalizedEvent, templates: &TemplateConfig) -> String {
    let fallback = announcement_message(event);
    let template = resolve_template(templates, event.agent).unwrap_or_default();
    if template.is_empty() {
        return fallback;
    }

    render_template(template, event).unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::agent::Agent;
    use crate::event::normalize;
    use crate::state::{AgentTemplateConfig, TemplateConfig};

    use super::*;

    fn codex_event() -> NormalizedEvent {
        normalize(
            Agent::Codex,
            json!({
                "type": "agent-turn-complete",
                "cwd": "/home/user/Projects/backend"
            }),
        )
        .expect("expected codex event")
    }

    #[test]
    fn resolve_prefers_agent_override_then_global() {
        let templates = TemplateConfig {
            global: Some("global".to_string()),
            agents: AgentTemplateConfig {
                codex: Some("agent".to_string()),
                ..AgentTemplateConfig::default()
            },
        };

        assert_eq!(resolve_template(&templates, Agent::Codex), Some("agent"));
        assert_eq!(resolve_template(&templates, Agent::Claude), Some("global"));
    }

    #[test]
    fn render_uses_context_fields() {
        let event = codex_event();
        let templates = TemplateConfig {
            global: Some("{{agent}} {{event_kind}} {{project}} {{cwd}}".to_string()),
            agents: AgentTemplateConfig::default(),
        };

        let message = render_announcement_message(&event, &templates);
        assert_eq!(
            message,
            "Codex task-end backend /home/user/Projects/backend"
        );
    }

    #[test]
    fn render_falls_back_when_template_is_invalid() {
        let event = codex_event();
        let templates = TemplateConfig {
            global: Some("{{#if".to_string()),
            agents: AgentTemplateConfig::default(),
        };

        let message = render_announcement_message(&event, &templates);
        assert_eq!(message, "Codex finished a task-end task in backend");
    }

    #[test]
    fn render_falls_back_when_template_outputs_only_whitespace() {
        let event = codex_event();
        let templates = TemplateConfig {
            global: Some("   ".to_string()),
            agents: AgentTemplateConfig::default(),
        };

        let message = render_announcement_message(&event, &templates);
        assert_eq!(message, "Codex finished a task-end task in backend");
    }
}
