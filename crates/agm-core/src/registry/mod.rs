use crate::registry::github::{GitHubOwner, GitHubRepoName};
use crate::skills::SkillPackage;
use color_eyre::eyre::{Result, bail};
pub mod github;

/// Represents a parsed registry source identifier.
#[derive(Debug, Clone)]
pub enum RegistrySource {
    GitHub {
        owner: GitHubOwner,
        repo_name: GitHubRepoName,
    },
}

#[allow(async_fn_in_trait)]
pub trait Registry {
    async fn fetch_skills(&self) -> Result<Vec<SkillPackage>>;
    async fn get_sha(&self) -> Result<String>;
}

/// Parses a source string like `github:owner/repo` or `https://github.com/owner/repo` into a `RegistrySource`.
pub fn parse_source(input: &str) -> Result<RegistrySource> {
    if input.is_empty() {
        bail!("Source cannot be empty")
    }

    let rest = if let Some(rest) = input.strip_prefix("github:") {
        rest.trim_end_matches('/')
    } else if let Some(rest) = input.strip_prefix("https://github.com/") {
        rest.trim_end_matches('/')
    } else {
        bail!(
            "Invalid source identifier: expected `github:owner/repo` or `https://github.com/owner/repo`, got `{}`",
            input
        );
    };

    let Some((owner, repo_name)) = rest
        .split_once('/')
        .filter(|(o, r)| !o.is_empty() && !r.is_empty() && !r.contains('/'))
    else {
        bail!(
            "Invalid source github repository format: expected `github:owner/repo` or `https://github.com/owner/repo`, got `{}`",
            input
        );
    };

    Ok(RegistrySource::GitHub {
        owner: owner.parse()?,
        repo_name: repo_name.parse()?,
    })
}
