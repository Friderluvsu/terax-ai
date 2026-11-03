use std::path::{Path, PathBuf};

use serde_json::json;

const GUIDE: &str = include_str!("orchestrator-guide.md");

pub struct OrchestratorFiles {
    pub guide: PathBuf,
    pub settings: PathBuf,
}

/// Claude settings for the orchestrator session only: the Terax CLI runs
/// without a prompt per call. Nothing is written to the user's Claude config.
fn settings() -> String {
    json!({
        "permissions": {
            "allow": ["Bash(\"$TERAX_CLI\" *)"],
        },
    })
    .to_string()
}

/// Writes the orchestrator system prompt and settings under `dir`, rewriting
/// them only when their content changed.
pub fn prepare(dir: &Path) -> Result<OrchestratorFiles, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let files = OrchestratorFiles {
        guide: dir.join("guide.md"),
        settings: dir.join("settings.json"),
    };
    write_if_changed(&files.guide, GUIDE)?;
    write_if_changed(&files.settings, &settings())?;
    Ok(files)
}

fn write_if_changed(path: &Path, contents: &str) -> Result<(), String> {
    if std::fs::read_to_string(path).is_ok_and(|existing| existing == contents) {
        return Ok(());
    }
    let dir = path.parent().ok_or("orchestrator path has no parent")?;
    let mut file = tempfile::NamedTempFile::new_in(dir).map_err(|e| e.to_string())?;
    std::io::Write::write_all(&mut file, contents.as_bytes()).map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepares_guide_and_settings_idempotently() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("orchestrator");
        let files = prepare(&root).unwrap();
        assert_eq!(std::fs::read_to_string(&files.guide).unwrap(), GUIDE);
        let settings: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&files.settings).unwrap()).unwrap();
        assert_eq!(
            settings["permissions"]["allow"],
            json!(["Bash(\"$TERAX_CLI\" *)"])
        );
        std::fs::write(&files.guide, "stale").unwrap();
        prepare(&root).unwrap();
        assert_eq!(std::fs::read_to_string(&files.guide).unwrap(), GUIDE);
    }

    #[test]
    fn guide_uses_only_the_pre_approved_cli_form() {
        for command in [
            "\"$TERAX_CLI\" orchestrate --check",
            "\"$TERAX_CLI\" task create --brief -",
            "\"$TERAX_CLI\" task wait --timeout 540",
            "\"$TERAX_CLI\" task result",
            "\"$TERAX_CLI\" task send",
            "\"$TERAX_CLI\" task stop",
        ] {
            assert!(GUIDE.contains(command), "{command}");
        }
        assert!(!GUIDE.contains("`terax "));
        assert!(!GUIDE.contains('\u{2014}'));
    }
}
