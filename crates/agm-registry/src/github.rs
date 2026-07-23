use agm_core::registry::{GitHubOwner, GitHubRepoName, Registry};
use agm_core::skills::{SkillContent, SkillName};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use color_eyre::eyre::{Context, ContextCompat, Result, bail};
use tracing::debug;

pub struct GitHubRegistry {
    pub owner: GitHubOwner,
    pub repo: GitHubRepoName,
}

impl Registry for GitHubRegistry {
    async fn fetch_skill(&self, skill_name: &SkillName) -> Result<SkillContent> {
        let path = format!("skills/{skill_name}/SKILL.md");
        let github = octocrab::instance();

        fetch_from_github(&github, &self.owner, &self.repo, &path)
            .await
            .context(format!(
                "Failed to download skill `{}` from {}/{}",
                skill_name, self.owner, self.repo
            ))
    }
}

/// Downloads file content from GitHub using the octocrab client.
/// The GitHub contents API returns base64-encoded content, which is decoded here.
async fn fetch_from_github(
    client: &octocrab::Octocrab,
    owner: &GitHubOwner,
    repo: &GitHubRepoName,
    path: &str,
) -> Result<SkillContent> {
    debug!(
        "Fetching file content from GitHub: owner = {}, repo = {}, path = {}",
        owner, repo, path
    );

    let response = client
        .repos(owner.as_str(), repo.as_str())
        .get_content()
        .path(path)
        .send()
        .await
        .context("Failed to fetch file from GitHub")?;
    debug!("GitHub response: {:?}", response);

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
fn extract_skill_name(path: &str) -> Result<SkillName> {
    let name = path
        .strip_suffix("/SKILL.md")
        .unwrap_or(path)
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .context("Could not extract skill name from path (empty name after trimming)")?;
    name.parse()
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
}
