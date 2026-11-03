use std::path::{Path, PathBuf};

use serde_json::json;

const GUIDE: &str = include_str!("supervisor-guide.md");

pub struct SupervisorFiles {
    pub guide: PathBuf,
    pub settings: PathBuf,
    pub job: PathBuf,
}

/// Claude refuses to match a permission rule against a command that contains
/// a variable, so the supervisor calls the CLI by its real path.
fn quoted_cli(cli: &str) -> String {
    format!("\"{}\"", cli.replace('"', ""))
}

/// Claude settings for the supervisor session only: the Terax CLI runs
/// without a prompt, and edits are allowed in the integration worktree alone,
/// so the user's own checkout cannot be changed by accident.
fn settings(integration: &Path, cli: &str) -> String {
    let dir = integration.to_string_lossy().replace('\\', "/");
    let dir = dir.trim_start_matches('/');
    json!({
        "permissions": {
            "allow": [
                format!("Bash({} *)", quoted_cli(cli)),
                format!("Edit(//{dir}/**)"),
                format!("Read(//{dir}/**)"),
            ],
            "additionalDirectories": [integration.to_string_lossy()],
        },
    })
    .to_string()
}

/// Writes the supervisor system prompt, settings and job brief under `dir`,
/// rewriting each only when its content changed.
pub fn prepare(
    dir: &Path,
    integration: &Path,
    job: &str,
    run: &str,
    cli: &str,
) -> Result<SupervisorFiles, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let files = SupervisorFiles {
        guide: dir.join("guide.md"),
        settings: dir.join("settings.json"),
        job: dir.join("job.md"),
    };
    write_if_changed(
        &files.guide,
        &GUIDE.replace("\"$TERAX_CLI\"", &quoted_cli(cli)),
    )?;
    write_if_changed(&files.settings, &settings(integration, cli))?;
    let job = format!(
        "Run id: {run}\nIntegration branch: orchestra/{run}\nIntegration worktree: {}\n\nJob:\n{}\n",
        integration.display(),
        job.trim()
    );
    write_if_changed(&files.job, &job)?;
    Ok(files)
}

fn write_if_changed(path: &Path, contents: &str) -> Result<(), String> {
    if std::fs::read_to_string(path).is_ok_and(|existing| existing == contents) {
        return Ok(());
    }
    let dir = path.parent().ok_or("supervisor path has no parent")?;
    let mut file = tempfile::NamedTempFile::new_in(dir).map_err(|e| e.to_string())?;
    std::io::Write::write_all(&mut file, contents.as_bytes()).map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepares_guide_settings_and_job_idempotently() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("supervisor");
        let integration = Path::new("/data/orchestra/r1/integration/worktree");
        let cli = "/Apps/Terax Dev.app/Contents/MacOS/terax-cli";
        let files = prepare(&root, integration, "  add limits  ", "r1", cli).unwrap();
        let guide = std::fs::read_to_string(&files.guide).unwrap();
        assert!(guide.contains(&format!("\"{cli}\" task wait")));
        assert!(!guide.contains("$TERAX_CLI"));
        let settings: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&files.settings).unwrap()).unwrap();
        let allow = settings["permissions"]["allow"].as_array().unwrap();
        assert!(allow.contains(&json!(format!("Bash(\"{cli}\" *)"))));
        assert!(allow.contains(&json!("Edit(//data/orchestra/r1/integration/worktree/**)")));
        assert_eq!(
            settings["permissions"]["additionalDirectories"],
            json!(["/data/orchestra/r1/integration/worktree"])
        );
        let job = std::fs::read_to_string(&files.job).unwrap();
        assert!(job.contains("Integration branch: orchestra/r1"));
        assert!(job.ends_with("Job:\nadd limits\n"));
        std::fs::write(&files.guide, "stale").unwrap();
        prepare(&root, integration, "add limits", "r1", cli).unwrap();
        assert_eq!(std::fs::read_to_string(&files.guide).unwrap(), guide);
    }

    #[test]
    fn guide_uses_only_the_substituted_cli_form() {
        for command in [
            "task wait",
            "task screen",
            "task send",
            "task merge",
            "task result",
        ] {
            assert!(
                GUIDE.contains(&format!("\"$TERAX_CLI\" {command}")),
                "{command}"
            );
        }
        assert!(!GUIDE.contains(" terax task"));
        assert!(!GUIDE.contains("```\nterax"));
    }
}
