//! Skill install and add flows.

use agm_core::registry::Registry;
use agm_core::skills::{SkillName, SkillPackage};
use agm_harness::Harness;
use color_eyre::eyre::{Context, ContextCompat, Result};
use std::path::Path;
use tracing::info;

/// Canonical skill file name per the agentskills.io standard.
pub const SKILL_FILE_NAME: &str = "SKILL.md";

/// Installs a skill into the project-local skills directory for the given harness.
///
/// The caller is responsible for detecting the harness (typically via
/// `Harness::detect()`). Skill files follow the agentskills.io standard:
/// `{skills_dir}/{skill_name}/SKILL.md`.
pub async fn install_to_harness(harness: &Harness, skill: &SkillPackage) -> Result<()> {
    info!(
        "Installing `{}` skill to {} for {} harness",
        skill.name,
        harness.project_skills_dir().display(),
        harness
    );

    skill.validate()?;
    let package_skill_file = skill
        .files
        .iter()
        .find(|file| file.relative_path == Path::new(SKILL_FILE_NAME))
        .context("Validated package did not contain root SKILL.md")?;
    let skill_dir = Path::new(&harness.project_skills_dir()).join(skill.name.as_str());

    // create_dir_all is idempotent and avoids TOCTOU races
    tokio::fs::create_dir_all(&skill_dir)
        .await
        .with_context(|| format!("Failed to create skill directory: {}", skill_dir.display()))?;

    let skill_file = skill_dir.join(SKILL_FILE_NAME);

    tokio::fs::write(&skill_file, &package_skill_file.bytes)
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
pub async fn auto_install_skill(skill: &SkillPackage) -> Result<()> {
    let harness: Harness = Harness::detect()?;
    info!("Detected harness: {}", harness);
    install_to_harness(&harness, skill).await
}

/// Downloads a specific skill from the registry and installs it locally.
///
/// Raw strings are rejected so callers cannot bypass `SkillName` validation:
///
/// ```compile_fail
/// use agm_core::registry::Registry;
/// use agm_skills::add_skill;
///
/// async fn add_raw_name<R: Registry>(registry: &R) {
///     add_skill(registry, "raw-name").await.unwrap();
/// }
/// ```
pub async fn add_skill<R: Registry>(registry: &R, skill_name: &SkillName) -> Result<()> {
    let skill = registry.fetch_skill(skill_name).await?;
    auto_install_skill(&skill).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use agm_core::skills::{SkillFile, SkillName, SkillPackage, SkillsDir};
    use agm_harness::Harness;
    use std::path::PathBuf;

    #[tokio::test]
    async fn install_to_harness_writes_skill_file() {
        let project_dir =
            std::env::temp_dir().join(format!("agm-skills-install-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&project_dir);

        let harness = Harness::Standard {
            skills_dir: SkillsDir {
                project: project_dir.clone(),
                global: project_dir.join("global"),
            },
        };
        let skill = SkillPackage {
            name: "test-skill".parse::<SkillName>().expect("valid skill name"),
            revision: "abc123".to_string(),
            files: vec![SkillFile {
                relative_path: PathBuf::from("SKILL.md"),
                bytes: b"# Test Skill\n".to_vec(),
                executable: false,
            }],
        };

        install_to_harness(&harness, &skill)
            .await
            .expect("skill should install");

        assert_eq!(
            tokio::fs::read_to_string(project_dir.join("test-skill").join("SKILL.md"))
                .await
                .expect("skill file should exist"),
            "# Test Skill\n"
        );

        std::fs::remove_dir_all(project_dir).expect("test directory should be removable");
    }
}
