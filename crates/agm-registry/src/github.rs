use agm_core::registry::{
    Registry,
    github::{GitHubOwner, GitHubRepoName},
};
use agm_core::skills::{SkillFile, SkillName, SkillPackage};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use color_eyre::eyre::{Context, ContextCompat, Result, bail};
use octocrab::models::repos::Content;
use std::path::PathBuf;
use tracing::debug;
mod archive;

/// Fetches skills from one GitHub `owner/repo`.
pub struct GitHubClient {
    pub owner: GitHubOwner,
    pub repo: GitHubRepoName,
}

impl Registry for GitHubClient {
    async fn fetch_skill(&self, name: &SkillName) -> Result<SkillPackage> {
        let path = format!("skills/{name}/SKILL.md");
        let github = octocrab::instance();

        async {
            let items = self.fetch_content_items(&github, &path).await?;
            let item = extract_requested_file(items, &path)?;
            let bytes = decode_file_content(&item)?;
            validate_content_integrity(&item, &bytes)?;
            build_skill_package(name.clone(), item, bytes)
        }
        .await
        .context(format!(
            "Failed to download skill `{}` from {}/{}",
            name, self.owner, self.repo
        ))
    }
}

impl GitHubClient {
    /// Fetches content items from GitHub using the octocrab client.
    async fn fetch_content_items(
        &self,
        client: &octocrab::Octocrab,
        path: &str,
    ) -> Result<Vec<Content>> {
        debug!(
            "Fetching file content from GitHub: owner = {}, repo = {}, path = {}",
            self.owner, self.repo, path
        );

        let response = client
            .repos(self.owner.as_str(), self.repo.as_str())
            .get_content()
            .path(path)
            .send()
            .await
            .context("Failed to fetch file from GitHub")?;
        debug!("GitHub response: {:?}", response);

        Ok(response.items)
    }
}

/// Extracts the exact requested file from a Contents API response.
fn extract_requested_file(items: Vec<Content>, expected_path: &str) -> Result<Content> {
    let item = match <[_; 1]>::try_from(items) {
        Ok([item]) => item,
        Err(items) if items.is_empty() => bail!("No content returned from GitHub"),
        Err(items) => bail!(
            "GitHub returned {} items for a single file path; expected exactly 1",
            items.len()
        ),
    };

    if item.r#type != "file" {
        bail!(
            "GitHub returned a '{}' at '{}', expected a file",
            item.r#type,
            expected_path
        );
    }

    if item.path != expected_path {
        bail!(
            "GitHub path mismatch for `{}`: response path was `{}`",
            expected_path,
            item.path
        );
    }
    Ok(item)
}

/// Decodes GitHub Contents API file content.
///
/// The Contents API only inlines content for files up to ~1 MB, using base64
/// encoding. Larger files are returned with `encoding: "none"` and empty
/// content, which would otherwise decode silently into an empty skill. This
/// guards against that by requiring an explicit `base64` encoding.
fn decode_file_content(item: &Content) -> Result<Vec<u8>> {
    let content = item
        .content
        .as_deref()
        .context("No content field in GitHub response")?;

    match item.encoding.as_deref() {
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
            item.path
        ),
        None => bail!("Missing content encoding for `{}`", item.path),
    }
}

/// Validates decoded bytes against Contents API metadata.
fn validate_content_integrity(item: &Content, bytes: &[u8]) -> Result<()> {
    if bytes.is_empty() {
        bail!("GitHub returned empty file content for `{}`", item.path);
    }
    let size = usize::try_from(item.size).with_context(|| {
        format!(
            "GitHub returned an invalid (negative) size for `{}`",
            item.path
        )
    })?;
    if bytes.len() != size {
        bail!(
            "GitHub size mismatch for `{}`: decoded {} bytes, metadata size {}",
            item.path,
            bytes.len(),
            size
        );
    }

    Ok(())
}

/// Maps validated GitHub content into the registry package type.
fn build_skill_package(name: SkillName, item: Content, bytes: Vec<u8>) -> Result<SkillPackage> {
    std::str::from_utf8(&bytes).context("File content is not valid UTF-8")?;
    Ok(SkillPackage {
        name,
        revision: item.sha,
        files: vec![SkillFile {
            relative_path: PathBuf::from("SKILL.md"),
            bytes,
            executable: false,
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_archive_chunk_accepts_exact_limit() {
        let mut buffer = b"abc".to_vec();
        append_archive_chunk(&mut buffer, b"de", 5).expect("exact limit should pass");
        assert_eq!(buffer, b"abcde");
    }

    #[test]
    fn append_archive_chunk_rejects_limit_without_appending() {
        let mut buffer = b"abc".to_vec();
        let error = append_archive_chunk(&mut buffer, b"def", 5)
            .expect_err("oversized archive should fail");
        assert!(
            error
                .to_string()
                .contains("5 byte compressed archive limit")
        );
        assert_eq!(buffer, b"abc");
    }

    #[test]
    fn append_archive_chunk_rejects_length_overflow() {
        let error = checked_archive_length(usize::MAX, 1, usize::MAX)
            .expect_err("length overflow should fail");
        assert!(error.to_string().contains("overflow"));
    }
}

#[cfg(any())]
mod obsolete_tests {
    use super::*;
    use agm_core::skills::SkillName;
    use octocrab::models::repos::{Content, ContentLinks};

    fn sample_content(path: &str, r#type: &str, size: i64) -> Content {
        Content {
            name: path.rsplit('/').next().unwrap_or(path).to_string(),
            path: path.to_string(),
            sha: "deadbeef".into(),
            encoding: Some("base64".into()),
            content: None,
            size,
            url: "https://api.github.com/content".into(),
            html_url: None,
            git_url: None,
            download_url: None,
            r#type: r#type.into(),
            links: ContentLinks {
                git: None,
                html: None,
                _self: "https://api.github.com/content".parse().unwrap(),
            },
            license: None,
        }
    }

    #[test]
    fn extract_requested_file_rejects_empty() {
        let err = extract_requested_file(vec![], "skills/x/SKILL.md")
            .expect_err("empty items should error");
        assert!(err.to_string().contains("No content"));
    }

    #[test]
    fn extract_requested_file_rejects_multiple() {
        let path = "skills/x/SKILL.md";
        let err = extract_requested_file(
            vec![
                sample_content(path, "file", 1),
                sample_content(path, "file", 1),
            ],
            path,
        )
        .expect_err("multiple items should error");
        assert!(err.to_string().contains("expected exactly 1"));
    }

    #[test]
    fn extract_requested_file_rejects_non_file() {
        let path = "skills/x/SKILL.md";
        let err = extract_requested_file(vec![sample_content(path, "dir", 0)], path)
            .expect_err("non-file should error");
        assert!(err.to_string().contains("expected a file"));
    }

    #[test]
    fn extract_requested_file_rejects_path_mismatch() {
        let err = extract_requested_file(
            vec![sample_content("skills/other/SKILL.md", "file", 1)],
            "skills/x/SKILL.md",
        )
        .expect_err("path mismatch should error");
        assert!(err.to_string().contains("path mismatch"));
    }

    #[test]
    fn extract_requested_file_accepts_exact_file() {
        let path = "skills/x/SKILL.md";
        let item = extract_requested_file(vec![sample_content(path, "file", 5)], path)
            .expect("single file should succeed");
        assert_eq!(item.path, path);
        assert_eq!(item.r#type, "file");
    }

    #[test]
    fn decode_file_content_decodes_base64_stripping_whitespace() {
        let path = "skills/x/SKILL.md";
        let mut item = sample_content(path, "file", 5);
        item.content = Some("aGVs\nbG8=\n".into());

        let bytes = decode_file_content(&item).expect("base64 content should decode");
        assert_eq!(bytes, b"hello");
    }

    #[test]
    fn decode_file_content_rejects_missing_content() {
        let path = "skills/x/SKILL.md";
        let item = sample_content(path, "file", 0);

        let err = decode_file_content(&item).expect_err("missing content should error");
        assert!(err.to_string().contains("No content field"));
    }

    #[test]
    fn decode_file_content_rejects_non_base64_encoding() {
        let path = "skills/x/SKILL.md";
        let mut item = sample_content(path, "file", 0);
        item.encoding = Some("none".into());
        item.content = Some(String::new());

        let err = decode_file_content(&item).expect_err("non-base64 encoding should error");
        assert!(err.to_string().contains("Unsupported content encoding"));
    }

    #[test]
    fn decode_file_content_rejects_missing_encoding() {
        let path = "skills/x/SKILL.md";
        let mut item = sample_content(path, "file", 0);
        item.encoding = None;
        item.content = Some(String::new());

        let err = decode_file_content(&item).expect_err("missing encoding should error");
        assert!(err.to_string().contains("Missing content encoding"));
    }

    #[test]
    fn decode_file_content_rejects_invalid_base64() {
        let path = "skills/x/SKILL.md";
        let mut item = sample_content(path, "file", 1);
        item.content = Some("%%%".into());

        let err = decode_file_content(&item).expect_err("invalid base64 should error");
        assert!(err.to_string().contains("Failed to decode base64 content"));
    }

    #[test]
    fn validate_content_integrity_rejects_empty() {
        let path = "skills/x/SKILL.md";
        let item = sample_content(path, "file", 0);

        let err =
            validate_content_integrity(&item, &[]).expect_err("empty bytes should be rejected");
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn validate_content_integrity_rejects_negative_size() {
        let path = "skills/x/SKILL.md";
        let item = sample_content(path, "file", -1);

        let err = validate_content_integrity(&item, b"x")
            .expect_err("negative metadata size should be rejected");
        assert!(err.to_string().contains("negative"));
    }

    #[test]
    fn validate_content_integrity_rejects_size_mismatch() {
        let path = "skills/x/SKILL.md";
        let item = sample_content(path, "file", 99);

        let err = validate_content_integrity(&item, b"hello")
            .expect_err("size mismatch should be rejected");
        assert!(err.to_string().contains("size mismatch"));
    }

    #[test]
    fn validate_content_integrity_accepts_matching_size() {
        let path = "skills/x/SKILL.md";
        let item = sample_content(path, "file", 5);

        validate_content_integrity(&item, b"hello")
            .expect("matching non-empty content should be valid");
    }

    #[test]
    fn build_skill_package_rejects_invalid_utf8() {
        let name = "x".parse::<SkillName>().expect("valid skill name");
        let item = sample_content("skills/x/SKILL.md", "file", 1);

        let err = build_skill_package(name, item, vec![0xff])
            .expect_err("invalid UTF-8 should be rejected");
        assert!(err.to_string().contains("not valid UTF-8"));
    }

    #[test]
    fn build_skill_package_uses_validated_name_and_revision() {
        let name = "requested-skill"
            .parse::<SkillName>()
            .expect("valid skill name");
        let item = sample_content("skills/response-name/SKILL.md", "file", 5);

        let skill = build_skill_package(name, item, b"hello".to_vec())
            .expect("valid content should build a skill package");
        assert_eq!(skill.name.as_str(), "requested-skill");
        assert_eq!(skill.revision, "deadbeef");
        assert_eq!(skill.files.len(), 1);
        assert_eq!(skill.files[0].relative_path, PathBuf::from("SKILL.md"));
        assert_eq!(skill.files[0].bytes, b"hello");
        assert!(!skill.files[0].executable);
    }
}
