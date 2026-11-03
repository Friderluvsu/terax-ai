use serde::Serialize;

const MAX_CACHE_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ModelOption {
    pub id: String,
    pub label: String,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgentCatalog {
    pub agent: &'static str,
    pub installed: bool,
    pub models: Vec<ModelOption>,
}

const AGENTS: &[&str] = &["claude", "codex", "gemini", "pi", "opencode", "grok"];

fn option(id: &str, label: &str, note: &str) -> ModelOption {
    ModelOption {
        id: id.to_string(),
        label: label.to_string(),
        note: note.to_string(),
    }
}

fn presets(agent: &str) -> Vec<ModelOption> {
    match agent {
        "claude" => vec![
            option(
                "opus",
                "Opus",
                "strongest, slowest; hard design and tricky bugs",
            ),
            option("sonnet", "Sonnet", "balanced; most feature work"),
            option("haiku", "Haiku", "fastest and cheapest; mechanical edits"),
        ],
        "gemini" => vec![
            option("gemini-2.5-pro", "Gemini 2.5 Pro", "strong; large context"),
            option("gemini-2.5-flash", "Gemini 2.5 Flash", "fast and cheap"),
        ],
        _ => Vec::new(),
    }
}

/// The models Codex itself lists for this account, from its local cache.
pub fn parse_codex_cache(json: &str) -> Vec<ModelOption> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let list = value
        .get("models")
        .and_then(|models| models.as_array())
        .or_else(|| value.as_array());
    let Some(list) = list else {
        return Vec::new();
    };
    list.iter()
        .filter(|model| model.get("visibility").and_then(|v| v.as_str()) == Some("list"))
        .filter_map(|model| {
            let id = model.get("slug")?.as_str()?;
            if terax_control_protocol::validate_model(id).is_err() {
                return None;
            }
            let label = model
                .get("display_name")
                .and_then(|v| v.as_str())
                .unwrap_or(id);
            let note = model
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            Some(option(id, label, note))
        })
        .collect()
}

pub fn build(installed: impl Fn(&str) -> bool, codex_cache: Option<&str>) -> Vec<AgentCatalog> {
    AGENTS
        .iter()
        .map(|&agent| AgentCatalog {
            agent,
            installed: installed(agent),
            models: match agent {
                "codex" => codex_cache.map(parse_codex_cache).unwrap_or_default(),
                other => presets(other),
            },
        })
        .collect()
}

pub fn load() -> Vec<AgentCatalog> {
    let cache = dirs::home_dir()
        .map(|home| home.join(".codex").join("models_cache.json"))
        .filter(|path| {
            std::fs::metadata(path)
                .map(|meta| meta.len() <= MAX_CACHE_BYTES)
                .unwrap_or(false)
        })
        .and_then(|path| std::fs::read_to_string(path).ok());
    build(
        |agent| crate::modules::lsp::env::resolve_binary(agent).is_some(),
        cache.as_deref(),
    )
}

/// The list the planner chooses from: installed agents and their models.
pub fn describe(catalog: &[AgentCatalog]) -> String {
    let mut out = String::new();
    for entry in catalog.iter().filter(|entry| entry.installed) {
        out.push_str(&format!("- agent \"{}\"", entry.agent));
        if entry.models.is_empty() {
            out.push_str(" (default model only)\n");
            continue;
        }
        out.push_str(", models:\n");
        for model in &entry.models {
            out.push_str(&format!("    - \"{}\": {}\n", model.id, model.note));
        }
    }
    out
}

/// Keeps only picks that exist in the catalog, so a planner cannot invent an
/// agent or model.
pub fn check(
    catalog: &[AgentCatalog],
    agent: Option<&str>,
    model: Option<&str>,
) -> (Option<String>, Option<String>) {
    let Some(entry) = agent.and_then(|agent| {
        catalog
            .iter()
            .find(|entry| entry.installed && entry.agent == agent)
    }) else {
        return (None, None);
    };
    let model = model
        .filter(|model| entry.models.iter().any(|known| known.id == *model))
        .map(str::to_string);
    (Some(entry.agent.to_string()), model)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Vec<AgentCatalog> {
        let cache = r#"{"models":[
            {"slug":"gpt-a","display_name":"GPT A","visibility":"list","description":"fast"},
            {"slug":"hidden","visibility":"hide"},
            {"slug":"--bad","visibility":"list"}
        ]}"#;
        build(|agent| agent != "gemini", Some(cache))
    }

    #[test]
    fn codex_models_come_from_the_visible_cache_entries() {
        let codex = catalog().into_iter().find(|a| a.agent == "codex").unwrap();
        let ids: Vec<_> = codex.models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["gpt-a"]);
        assert!(parse_codex_cache("not json").is_empty());
    }

    #[test]
    fn the_planner_only_sees_installed_agents() {
        let text = describe(&catalog());
        assert!(text.contains("agent \"claude\""));
        assert!(text.contains("\"haiku\""));
        assert!(!text.contains("gemini"));
        assert!(text.contains("agent \"pi\"") && text.contains("default model only"));
    }

    #[test]
    fn invented_agents_and_models_are_dropped() {
        let catalog = catalog();
        assert_eq!(
            check(&catalog, Some("claude"), Some("haiku")),
            (Some("claude".into()), Some("haiku".into()))
        );
        assert_eq!(
            check(&catalog, Some("claude"), Some("gpt-9")),
            (Some("claude".into()), None)
        );
        assert_eq!(
            check(&catalog, Some("gemini"), Some("gemini-2.5-pro")),
            (None, None)
        );
        assert_eq!(check(&catalog, Some("rm -rf"), None), (None, None));
        assert_eq!(check(&catalog, None, Some("haiku")), (None, None));
    }
}
