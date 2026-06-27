use crate::harness::Harness;
use crate::registry::RegistrySource;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use color_eyre::eyre::{Context, ContextCompat, Result, bail};
use std::path::{Path, PathBuf};
use tracing::{debug, info};

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

/// Downloads file content from GitHub using the octocrab client.
/// The GitHub contents API returns base64-encoded content, which is decoded here.
pub async fn fetch_skill(
    client: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    path: &str,
) -> Result<SkillContent> {
    debug!(
        "Fetching file content from GitHub: owner = {}, repo = {}, path = {}",
        owner, repo, path
    );

    let response = client
        .repos(owner, repo)
        .get_content()
        .path(path)
        .send()
        .await
        .context("Failed to fetch file from GitHub")?;
    debug!("GitHub response: {:?}", response);

    // Ensure we got exactly one item and it's a file (not a dir/symlink/submodule)
    if response.items.is_empty() {
        bail!("No content returned from GitHub");
    }
    if response.items.len() > 1 {
        bail!(
            "GitHub returned {} items for a single file path; expected exactly 1",
            response.items.len()
        );
    }
    let item = &response.items[0];
    if item.r#type != "file" {
        bail!(
            "GitHub returned a '{}' at '{}', expected a file",
            item.r#type,
            path
        );
    }
    debug!("GitHub item: {:?}", item);

    let encoded = item
        .content
        .as_deref()
        .context("No content field in GitHub response")?;
    // GitHub inserts newlines into base64 content; strip before decoding
    let cleaned: String = encoded.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = STANDARD
        .decode(&cleaned)
        .context("Failed to decode base64 content")?;
    // Extract skill name from path, handling edge cases
    let name = item
        .path
        .trim_end_matches("/SKILL.md")
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .context("Could not extract skill name from path (empty name after trimming)")?;

    Ok(SkillContent {
        name,
        content: String::from_utf8(bytes).context("File content is not valid UTF-8")?,
        sha: item.sha.clone(),
        encoding: item.encoding.clone(),
        size: item.size as usize,
    })
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

/// Downloads a specific skill from the repository.
///
/// According to Registry-style, looks for `skills/{skill_name}/SKILL.md` for repositories.
pub async fn add_skill(source: RegistrySource, skill_name: &str) -> Result<()> {
    // Validate skill name before any network I/O (defense-in-depth)
    validate_skill_name(skill_name)?;

    // First, try to fetch from skills/ directory (registry style)
    let skill_file_path = format!("skills/{}/SKILL.md", skill_name);
    match source {
        RegistrySource::GitHub { owner, repo } => {
            let github = octocrab::instance();
            let skill: SkillContent = fetch_skill(&github, &owner, &repo, &skill_file_path)
                .await
                .context(format!(
                    "Failed to download skill `{}` from {}/{}",
                    skill_name, owner, repo
                ))?;
            auto_install_skill(&skill).await?;
            Ok(())
        }
    }
}
