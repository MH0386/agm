//! GitHub registry that fetches skills from GitHub repositories.

use crate::archive::parse_skill_archive_bytes;
use agm_core::registry::{
    Registry,
    github::{GitHubOwner, GitHubRepoName},
};
use agm_core::skills::SkillPackage;
use color_eyre::eyre::{Context, ContextCompat, Result, bail};
use http_body_util::{BodyExt, Limited};
use octocrab::models::repos::Object::{Commit, Tag};
use octocrab::{Octocrab, params::repos::Reference};
use tracing::debug;

/// Maximum compressed `.tar.gz` size accepted from GitHub.
const MAX_COMPRESSED_ARCHIVE_BYTES: usize = 64 * 1024 * 1024;

/// Fetches skills from one GitHub `owner/repo`.
pub struct GitHubRegistry {
    pub owner: GitHubOwner,
    pub repo_name: GitHubRepoName,
}

impl Registry for GitHubRegistry {
    async fn fetch_skills(&self) -> Result<Vec<SkillPackage>> {
        let github = octocrab::instance();
        let sha = self.get_repository_sha(&github).await?;
        let archive_bytes = self.download_repository_tarball(&github, &sha).await?;
        parse_skill_archive_bytes(&archive_bytes).await
    }
    async fn get_sha(&self) -> Result<String> {
        let github = octocrab::instance();
        self.get_repository_sha(&github).await
    }
}

impl GitHubRegistry {
    /// Resolves the commit SHA of the repository's default branch.
    ///
    /// look up the repo's default branch, then resolve
    /// `refs/heads/<branch>` to a commit SHA (like `git rev-parse origin/HEAD`).
    async fn get_repository_sha(&self, github: &Octocrab) -> Result<String> {
        let repo = github
            .repos(self.owner.as_str(), self.repo_name.as_str())
            .get()
            .await
            .context("Failed to fetch repository metadata")?;
        let default_branch = repo
            .default_branch
            .context("Repository metadata is missing default_branch")?;
        debug!(%default_branch, "Getting SHA for default branch");

        let git_ref = github
            .repos(self.owner.as_str(), self.repo_name.as_str())
            .get_ref(&Reference::Branch(default_branch))
            .await
            .context("Failed to get SHA for default branch")?;

        Ok(match git_ref.object {
            Commit { sha, .. } | Tag { sha, .. } => sha,
            other => bail!("Unexpected git ref object type: {other:?}"),
        })
    }

    /// Downloads the repository `.tar.gz` for `sha` with a compressed size ceiling.
    async fn download_repository_tarball(&self, github: &Octocrab, sha: &str) -> Result<Vec<u8>> {
        debug!(%sha, "Downloading repository tarball");
        let response = github
            .repos(self.owner.as_str(), self.repo_name.as_str())
            .download_tarball(sha.to_string())
            .await
            .context("Failed to download repository tarball")?;

        let status = response.status();
        if !status.is_success() {
            bail!("GitHub tarball download returned HTTP {status:?}");
        }
        let collected = Limited::new(response.into_body(), MAX_COMPRESSED_ARCHIVE_BYTES)
            .collect()
            .await
            .map_err(|err| {
                color_eyre::eyre::eyre!(
                    "Failed to read tarball response body (or exceeded size limit): {err}"
                )
            })?;
        Ok(collected.to_bytes().to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::Full;

    /// Collects a body with the same size ceiling used for GitHub tarballs.
    async fn collect_limited(body: Full<bytes::Bytes>, limit: usize) -> Result<Vec<u8>> {
        let collected = Limited::new(body, limit).collect().await.map_err(|err| {
            color_eyre::eyre::eyre!(
                "Failed to read tarball response body (or exceeded size limit): {err}"
            )
        })?;
        Ok(collected.to_bytes().to_vec())
    }

    #[tokio::test]
    async fn limited_body_rejects_oversized_stream() {
        let body = Full::new(bytes::Bytes::from(vec![0_u8; 6]));
        let error = collect_limited(body, 5)
            .await
            .expect_err("oversized body should fail");
        assert!(error.to_string().contains("exceeded size limit"));
    }

    #[tokio::test]
    async fn limited_body_accepts_body_within_limit() {
        let body = Full::new(bytes::Bytes::from_static(b"hello"));
        let bytes = collect_limited(body, 5)
            .await
            .expect("body within limit should succeed");
        assert_eq!(bytes, b"hello");
    }
}
