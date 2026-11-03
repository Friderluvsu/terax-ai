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

const AGENTS: &[&str] = &["claude", "codex", "antigravity", "pi", "opencode", "grok"];
const MODELS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

/// The executable behind each agent id.
pub fn binary(agent: &str) -> &str {
    match agent {
        "antigravity" => "agy",
        other => other,
    }
}

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

fn agy_note(id: &str) -> &'static str {
    let id = id.to_ascii_lowercase();
    if id.contains("opus") {
        "strongest; hard design and tricky bugs"
    } else if id.contains("sonnet") {
        "balanced; most feature work"
    } else if id.contains("flash") && id.ends_with("low") {
        "fastest and cheapest; mechanical edits"
    } else if id.contains("flash") && id.ends_with("high") {
        "fast with deeper reasoning"
    } else if id.contains("flash") {
        "fast and cheap; everyday edits"
    } else if id.contains("gpt-oss") {
        "open-weight model"
    } else {
        ""
    }
}

/// `agy models` prints one `<id>\t<label>` line per model after a banner.
pub fn parse_agy_models(output: &str) -> Vec<ModelOption> {
    output
        .lines()
        .filter_map(|line| {
            let (id, label) = line.split_once('\t')?;
            let id = id.trim();
            if terax_control_protocol::validate_model(id).is_err() {
                return None;
            }
            Some(option(id, label.trim(), agy_note(id)))
        })
        .collect()
}

fn agy_fallback() -> Vec<ModelOption> {
    parse_agy_models(
        "gemini-3.8-flash-high\tGemini 3.8 Flash (High)\n\
         gemini-3.8-flash-medium\tGemini 3.8 Flash (Medium)\n\
         gemini-3.8-flash-low\tGemini 3.8 Flash (Low)\n\
         claude-sonnet-4-6\tClaude Sonnet 4.6 (Thinking)\n\
         claude-opus-4-6-thinking\tClaude Opus 4.6 (Thinking)\n\
         gpt-oss-120b-medium\tGPT-OSS 120B (Medium)\n",
    )
}

fn run_agy_models() -> Option<Vec<ModelOption>> {
    let program = crate::modules::lsp::env::resolve_binary("agy")?;
    let mut cmd = std::process::Command::new(program);
    cmd.arg("models")
        .envs(crate::modules::lsp::env::server_env_overlay())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    crate::modules::proc::hide_console(&mut cmd);
    let mut child = cmd.spawn().ok()?;
    let started = std::time::Instant::now();
    loop {
        match child.try_wait().ok()? {
            Some(_) => break,
            None if started.elapsed() >= MODELS_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            None => std::thread::sleep(std::time::Duration::from_millis(100)),
        }
    }
    let mut text = String::new();
    std::io::Read::read_to_string(&mut child.stdout.take()?, &mut text).ok()?;
    let models = parse_agy_models(&text);
    (!models.is_empty()).then_some(models)
}

/// Asked once per app session: it needs the network and takes a few seconds.
fn agy_models() -> Vec<ModelOption> {
    static MODELS: std::sync::OnceLock<Vec<ModelOption>> = std::sync::OnceLock::new();
    MODELS
        .get_or_init(|| run_agy_models().unwrap_or_else(agy_fallback))
        .clone()
}

pub fn build(
    installed: impl Fn(&str) -> bool,
    codex_cache: Option<&str>,
    agy_models: impl Fn() -> Vec<ModelOption>,
) -> Vec<AgentCatalog> {
    AGENTS
        .iter()
        .map(|&agent| {
            let installed = installed(agent);
            AgentCatalog {
                agent,
                installed,
                models: match agent {
                    "codex" => codex_cache.map(parse_codex_cache).unwrap_or_default(),
                    "antigravity" if installed => agy_models(),
                    other => presets(other),
                },
            }
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
        |agent| crate::modules::lsp::env::resolve_binary(binary(agent)).is_some(),
        cache.as_deref(),
        agy_models,
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
        build(
            |agent| agent != "antigravity",
            Some(cache),
            || unreachable!("not installed, so never asked"),
        )
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
        assert!(!text.contains("antigravity"));
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
            check(&catalog, Some("antigravity"), Some("claude-sonnet-4-6")),
            (None, None)
        );
        assert_eq!(check(&catalog, Some("rm -rf"), None), (None, None));
        assert_eq!(check(&catalog, None, Some("haiku")), (None, None));
    }

    #[test]
    fn agy_models_parse_from_its_listing_with_notes() {
        let out = "Fetching available models...\ngemini-3.8-flash-low\tGemini 3.8 Flash (Low)\nclaude-opus-4-6-thinking\tClaude Opus 4.6 (Thinking)\nnot a model line\n--bad\tx\n";
        let models = parse_agy_models(out);
        let ids: Vec<_> = models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["gemini-3.8-flash-low", "claude-opus-4-6-thinking"]);
        assert_eq!(models[0].label, "Gemini 3.8 Flash (Low)");
        assert!(models[0].note.contains("cheapest"));
        assert!(models[1].note.contains("strongest"));
        assert!(!agy_fallback().is_empty());
    }

    #[test]
    fn installed_antigravity_lists_its_models_and_maps_to_agy() {
        let catalog = build(|_| true, None, agy_fallback);
        let agy = catalog.iter().find(|a| a.agent == "antigravity").unwrap();
        assert!(agy.models.iter().any(|m| m.id == "claude-sonnet-4-6"));
        assert_eq!(binary("antigravity"), "agy");
        assert_eq!(binary("claude"), "claude");
    }
}
