use anyhow::{Context, Result};
use handlebars::Handlebars;
use serde::Serialize;
use std::collections::BTreeMap;

use crate::agent::Agent;
use crate::event::NormalizedEvent;
use crate::state::{EventKindLabelsConfig, TemplateConfig};

const TEMPLATE_NAME: &str = "announcement";
const BUILTIN_DEFAULT_TEMPLATE: &str =
    "{{agent}} finished a {{event_kind}} in the {{project}} project";

#[derive(Debug, Serialize)]
struct AnnouncementContext<'a> {
    agent: &'a str,
    event_kind: &'a str,
    event_kind_raw: &'a str,
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

fn normalize_event_kind_key(event_kind: &str) -> String {
    event_kind.trim().to_ascii_lowercase()
}

fn humanize_event_kind(event_kind: &str) -> String {
    let replaced = event_kind.replace('-', " ").replace('_', " ");
    let collapsed = replaced.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        "event".to_string()
    } else {
        collapsed
    }
}

fn agent_event_kind_labels(
    labels: &EventKindLabelsConfig,
    agent: Agent,
) -> &BTreeMap<String, String> {
    match agent {
        Agent::Claude => &labels.agents.claude,
        Agent::Codex => &labels.agents.codex,
        Agent::Generic => &labels.agents.generic,
    }
}

fn resolve_event_kind_label(event: &NormalizedEvent, labels: &EventKindLabelsConfig) -> String {
    let key = normalize_event_kind_key(&event.event_kind);
    let resolved = agent_event_kind_labels(labels, event.agent)
        .get(&key)
        .map(String::as_str)
        .or_else(|| labels.global.get(&key).map(String::as_str))
        .or_else(|| {
            if key == "task-end" {
                Some("task")
            } else {
                None
            }
        })
        .map(|label| label.trim())
        .filter(|label| !label.is_empty());

    match resolved {
        Some(label) => label.to_string(),
        None => humanize_event_kind(&event.event_kind),
    }
}

fn context_from_event<'a>(
    event: &'a NormalizedEvent,
    event_kind_label: &'a str,
) -> AnnouncementContext<'a> {
    let cwd = event
        .cwd
        .as_ref()
        .and_then(|path| path.to_str())
        .unwrap_or_default();

    AnnouncementContext {
        agent: event.agent.display_name(),
        event_kind: event_kind_label,
        event_kind_raw: &event.event_kind,
        project: &event.project_name,
        cwd,
    }
}

fn render_template(
    template: &str,
    event: &NormalizedEvent,
    event_kind_label: &str,
) -> Option<String> {
    let mut renderer = Handlebars::new();
    renderer.set_strict_mode(false);

    if renderer
        .register_template_string(TEMPLATE_NAME, template)
        .is_err()
    {
        return None;
    }

    renderer
        .render(TEMPLATE_NAME, &context_from_event(event, event_kind_label))
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

pub fn render_announcement_message(
    event: &NormalizedEvent,
    templates: &TemplateConfig,
    event_kind_labels: &EventKindLabelsConfig,
) -> String {
    let event_kind_label = resolve_event_kind_label(event, event_kind_labels);
    let default_message = render_template(BUILTIN_DEFAULT_TEMPLATE, event, &event_kind_label)
        .unwrap_or_else(|| {
            format!(
                "{} finished a {} in the {} project",
                event.agent.display_name(),
                event_kind_label,
                event.project_name
            )
        });

    match resolve_template(templates, event.agent) {
        Some(template) => {
            render_template(template, event, &event_kind_label).unwrap_or(default_message)
        }
        None => default_message,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::agent::Agent;
    use crate::event::normalize;
    use crate::state::{
        AgentEventKindLabelsConfig, AgentTemplateConfig, EventKindLabelsConfig, TemplateConfig,
    };

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

    fn empty_labels() -> EventKindLabelsConfig {
        EventKindLabelsConfig {
            global: BTreeMap::new(),
            agents: AgentEventKindLabelsConfig::default(),
        }
    }

    #[test]
    fn render_uses_context_fields() {
        let event = codex_event();
        let templates = TemplateConfig {
            global: Some("{{agent}} {{event_kind}} {{event_kind_raw}} {{project}} {{cwd}}".to_string()),
            agents: AgentTemplateConfig::default(),
        };

        let message = render_announcement_message(&event, &templates, &empty_labels());
        assert_eq!(
            message,
            "Codex task task-end backend /home/user/Projects/backend"
        );
    }

    #[test]
    fn render_falls_back_when_template_is_invalid() {
        let event = codex_event();
        let templates = TemplateConfig {
            global: Some("{{#if".to_string()),
            agents: AgentTemplateConfig::default(),
        };

        let message = render_announcement_message(&event, &templates, &empty_labels());
        assert_eq!(message, "Codex finished a task in the backend project");
    }

    #[test]
    fn render_falls_back_when_template_outputs_only_whitespace() {
        let event = codex_event();
        let templates = TemplateConfig::default();

        let message = render_announcement_message(&event, &templates, &empty_labels());
        assert_eq!(message, "Codex finished a task in the backend project");
    }

    #[test]
    fn render_uses_configured_global_event_kind_label() {
        let event = codex_event();
        let templates = TemplateConfig {
            global: Some("{{event_kind}}".to_string()),
            agents: AgentTemplateConfig::default(),
        };
        let labels = EventKindLabelsConfig {
            global: BTreeMap::from([("task-end".to_string(), "task".to_string())]),
            agents: AgentEventKindLabelsConfig::default(),
        };

        let message = render_announcement_message(&event, &templates, &labels);
        assert_eq!(message, "task");
    }

    #[test]
    fn render_prefers_agent_specific_event_kind_label() {
        let event = codex_event();
        let templates = TemplateConfig {
            global: Some("{{event_kind}}".to_string()),
            agents: AgentTemplateConfig::default(),
        };
        let labels = EventKindLabelsConfig {
            global: BTreeMap::from([("task-end".to_string(), "task".to_string())]),
            agents: AgentEventKindLabelsConfig {
                codex: BTreeMap::from([("task-end".to_string(), "turn".to_string())]),
                ..AgentEventKindLabelsConfig::default()
            },
        };

        let message = render_announcement_message(&event, &templates, &labels);
        assert_eq!(message, "turn");
    }
}
