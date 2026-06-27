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

    let encoded = item
        .content
        .as_deref()
        .context("No content field in GitHub response")?;
    let bytes = decode_github_content(item.encoding.as_deref(), encoded, path)?;

    // Extract and validate the skill name from the path (defense-in-depth: the
    // name is derived from the API response, not a trusted local input).
    let name = extract_skill_name(&item.path)?;
    validate_skill_name(&name)?;

    let size = usize::try_from(item.size)
        .with_context(|| format!("GitHub returned an invalid (negative) size for `{}`", path))?;

    Ok(SkillContent {
        name,
        content: String::from_utf8(bytes).context("File content is not valid UTF-8")?,
        sha: item.sha.clone(),
        encoding: item.encoding.clone(),
        size,
    })
}

/// Decodes GitHub Contents API file content.
///
/// The Contents API only inlines content for files up to ~1 MB, using base64
/// encoding. Larger files are returned with `encoding: "none"` and empty
/// content, which would otherwise decode silently into an empty skill. This
/// guards against that by requiring an explicit `base64` encoding.
fn decode_github_content(encoding: Option<&str>, content: &str, path: &str) -> Result<Vec<u8>> {
    match encoding {
        Some("base64") => {
            // GitHub inserts newlines into base64 content; strip before decoding.
            let cleaned: String = content.chars().filter(|c| !c.is_whitespace()).collect();
            STANDARD
                .decode(&cleaned)
                .context("Failed to decode base64 content")
        }
        Some(other) => bail!(
            "Unsupported content encoding `{}` for `{}` (the file may exceed GitHub's inline content size limit)",
            other,
            path
        ),
        None => bail!("Missing content encoding for `{}`", path),
    }
}

/// Extracts the skill name from a `.../{skill_name}/SKILL.md` path.
///
/// Strips a single trailing `/SKILL.md` segment (unlike `trim_end_matches`,
/// which would repeatedly strip the pattern) and returns the final path
/// component.
fn extract_skill_name(path: &str) -> Result<String> {
    path.strip_suffix("/SKILL.md")
        .unwrap_or(path)
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .context("Could not extract skill name from path (empty name after trimming)")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_base64_content_stripping_whitespace() {
        // "hello" base64-encoded, with GitHub-style embedded newlines.
        let bytes = decode_github_content(Some("base64"), "aGVs\nbG8=\n", "skills/x/SKILL.md")
            .expect("should decode");
        assert_eq!(bytes, b"hello");
    }

    #[test]
    fn rejects_non_base64_encoding() {
        // Large files come back with `encoding: "none"` and empty content;
        // this must error instead of silently producing an empty skill.
        let err = decode_github_content(Some("none"), "", "skills/x/SKILL.md")
            .expect_err("non-base64 encoding should error");
        assert!(err.to_string().contains("Unsupported content encoding"));
    }

    #[test]
    fn rejects_missing_encoding() {
        assert!(decode_github_content(None, "", "skills/x/SKILL.md").is_err());
    }

    #[test]
    fn extracts_skill_name_from_path() {
        assert_eq!(
            extract_skill_name("skills/my-skill/SKILL.md").unwrap(),
            "my-skill"
        );
    }

    #[test]
    fn extracts_skill_name_strips_only_one_suffix() {
        // `trim_end_matches` would strip both segments; `strip_suffix` strips one.
        assert_eq!(
            extract_skill_name("skills/SKILL.md/SKILL.md").unwrap(),
            "SKILL.md"
        );
    }

    #[test]
    fn extract_skill_name_rejects_empty_component() {
        assert!(extract_skill_name("/SKILL.md").is_err());
    }

    #[test]
    fn validate_skill_name_rejects_traversal() {
        assert!(validate_skill_name("..").is_err());
        assert!(validate_skill_name("a/b").is_err());
        assert!(validate_skill_name("ok-name").is_ok());
    }
}
