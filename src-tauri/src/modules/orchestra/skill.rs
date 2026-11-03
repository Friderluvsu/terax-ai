use std::path::{Path, PathBuf};

const SKILL: &str = include_str!("skill/SKILL.md");
const MARKER: &str = "terax-orchestra-skill-v";

fn skill_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".claude/skills/terax-orchestra/SKILL.md"))
}

/// Installs or refreshes the Claude Code skill that teaches the orchestrator
/// workflow. A file without the Terax marker belongs to the user and is kept.
pub fn install() {
    if let Some(path) = skill_path() {
        if let Err(error) = install_at(&path) {
            log::warn!("orchestra skill not installed: {error}");
        }
    }
}

fn install_at(path: &Path) -> Result<(), String> {
    match std::fs::read_to_string(path) {
        Ok(existing) if existing == SKILL => return Ok(()),
        Ok(existing) if !existing.contains(MARKER) => {
            return Err(format!("{} is not managed by Terax", path.display()));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("read {}: {error}", path.display())),
    }
    let dir = path.parent().ok_or("skill path has no parent")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let mut file = tempfile::NamedTempFile::new_in(dir).map_err(|e| e.to_string())?;
    std::io::Write::write_all(&mut file, SKILL.as_bytes()).map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_refreshes_and_keeps_foreign_skills() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("skills/terax-orchestra/SKILL.md");
        install_at(&path).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), SKILL);

        std::fs::write(&path, format!("old <!-- {MARKER}0 -->")).unwrap();
        install_at(&path).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), SKILL);

        std::fs::write(&path, "my own skill").unwrap();
        assert!(install_at(&path).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "my own skill");
    }

    #[test]
    fn skill_documents_every_cli_command_it_relies_on() {
        assert!(SKILL.starts_with("---\nname: terax-orchestra\n"));
        for command in [
            "terax orchestrate --check",
            "terax task create --brief",
            "terax task wait",
            "terax task result",
            "terax task send",
            "terax task stop",
        ] {
            assert!(SKILL.contains(command), "{command}");
        }
        assert!(!SKILL.contains('\u{2014}'));
    }
}
