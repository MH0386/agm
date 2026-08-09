//! Skill install and add flows.

use agm_core::registry::Registry;
use agm_core::skills::SkillName;
use agm_harness::Harness;
use color_eyre::eyre::{Result, bail};
use std::path::PathBuf;
use tokio::fs::{OpenOptions, create_dir_all};
use tokio::io::AsyncWriteExt;
use tracing::{debug, info};

/// Downloads a skill from the registry and installs it into the detected harness.
///
/// Detects the active harness from the current working directory, then writes
/// the package following the agentskills.io standard.
pub async fn add_skill<R: Registry>(registry: &R, skill_name: &SkillName) -> Result<()> {
    let skill = registry.fetch_skill(skill_name).await?;
    let harness = Harness::detect()?;
    info!("Detected harness: {}", harness);

    let skill_dir = harness.project_skills_dir().join(&skill.name);
    debug!(
        "Installing `{}` skill to `{}` for `{}` harness",
        &skill.name,
        skill_dir.display(),
        harness
    );
    for file in &skill.files {
        let mut path_components = file.get_relative_path().components();
        let trimmed_path = path_components.by_ref().skip(2).collect::<PathBuf>();
        let dest = skill_dir.join(&trimmed_path);
        if let Some(parent) = dest.parent() {
            match create_dir_all(parent).await {
                Ok(_) => (),
                Err(_) => bail!("Failed to create skill directory: {}", parent.display()),
            }
        }
        let mut dest_file = OpenOptions::new()
            .write(true) // TODO: check if this is needed
            .create(true) // TODO: check if this is needed
            .truncate(true) // TODO: check if this is needed
            .mode(file.get_permissions())
            .open(&dest)
            .await?;
        match dest_file.write_all(file.get_bytes()).await {
            Ok(_) => debug!("Wrote skill file: {}", dest.display()),
            Err(_) => bail!("Failed to write skill file: {}", dest.display()),
        }
    }

    info!("The installation is complete.");
    Ok(())
}
