use crate::harness::Harness;
use crate::registry::Registry;
use color_eyre::eyre::{Context, Result, bail};
use std::path::{Path, PathBuf};
use tracing::info;

/// Canonical skill file name per the agentskills.io standard.
pub const SKILL_FILE_NAME: &str = "SKILL.md";

/// Shared struct holding the project-local and global skill directory paths.
#[derive(Debug, Clone)]
pub struct SkillsDir {
    pub project: PathBuf,
    pub global: PathBuf,
}

/// Represents the content of a downloaded skill.
#[derive(Debug)]
pub struct SkillContent {
    pub name: String,
    pub content: String,
    pub sha: String,
    pub encoding: Option<String>,
    pub size: usize,
}

/// Validates that a skill name is a single safe path component.
///
/// Rejects empty names, `.`, `..`, and characters that are invalid or
/// problematic across platforms (`/`, `\`, `:`, `<`, `>`, `"`, `|`, `?`,
/// `*`, and NUL) to prevent directory traversal and ensure consistent
/// behavior on Windows, macOS, and Linux.
pub fn validate_skill_name(name: &str) -> Result<&str> {
    if name.is_empty() {
        bail!("Skill name must not be empty");
    }

    if name == "." || name == ".." {
        bail!("Invalid skill name `{}`: must not be `.` or `..`", name);
    }

    const FORBIDDEN: &[char] = &['/', '\\', ':', '<', '>', '"', '|', '?', '*', '\0'];
    if let Some(c) = name.chars().find(|c| FORBIDDEN.contains(c)) {
        bail!(
            "Invalid skill name `{}`: contains forbidden character `{}`",
            name,
            c
        );
    }

    Ok(name)
}

/// Installs a skill into the project-local skills directory for the given harness.
///
/// The caller is responsible for detecting the harness (typically via
/// `Harness::detect()`). Skill files follow the agentskills.io standard:
/// `{skills_dir}/{skill_name}/SKILL.md`.
pub async fn install_to_harness(harness: &Harness, skill: &SkillContent) -> Result<()> {
    info!(
        "Installing `{}` skill to {} for {} harness",
        skill.name,
        harness.project_skills_dir().display(),
        harness
    );

    // Validate before joining onto a filesystem path to prevent traversal,
    // regardless of how the `SkillContent` was constructed.
    validate_skill_name(&skill.name)?;

    let skill_dir = Path::new(&harness.project_skills_dir()).join(&skill.name);

    // create_dir_all is idempotent and avoids TOCTOU races
    tokio::fs::create_dir_all(&skill_dir)
        .await
        .with_context(|| format!("Failed to create skill directory: {}", skill_dir.display()))?;

    let skill_file = skill_dir.join(SKILL_FILE_NAME);

    tokio::fs::write(&skill_file, &skill.content)
        .await
        .with_context(|| format!("Failed to write skill file: {}", skill_file.display()))?;

    info!(
        "Installed `{}` skill to {} for {} harness",
        skill.name,
        skill_dir.display(),
        harness
    );
    Ok(())
}

/// Installs a skill using the detected harness's skills directory.
///
/// Detects the active harness from the current working directory and writes
/// the skill to the appropriate location following the agentskills.io standard.
pub async fn auto_install_skill(skill: &SkillContent) -> Result<()> {
    let harness: Harness = Harness::detect()?;
    info!("Detected harness: {}", harness);
    install_to_harness(&harness, skill).await
}

/// Downloads a specific skill from the registry and installs it locally.
pub async fn add_skill<R: Registry>(registry: &R, skill_name: &str) -> Result<()> {
    validate_skill_name(skill_name)?;
    let skill = registry.fetch_skill(skill_name).await?;
    auto_install_skill(&skill).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_skill_name_rejects_traversal() {
        assert!(validate_skill_name("..").is_err());
        assert!(validate_skill_name("a/b").is_err());
        assert!(validate_skill_name("ok-name").is_ok());
    }
}
