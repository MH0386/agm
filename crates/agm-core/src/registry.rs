use crate::skills::{SkillName, SkillPackage};
use color_eyre::eyre::{Report, Result, bail};
use std::{fmt, str::FromStr};

/// A validated GitHub user or organization name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubOwner(String);

impl GitHubOwner {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for GitHubOwner {
    type Err = Report;

    fn from_str(owner: &str) -> Result<Self> {
        if !(1..=39).contains(&owner.len()) || !owner.is_ascii() {
            bail!("Invalid GitHub owner: must be 1 to 39 ASCII characters");
        }

        let mut parts = owner.split('_');
        let standard = parts.next().expect("split always yields one part");
        let shortcode = parts.next();
        if parts.next().is_some()
            || standard.is_empty()
            || standard.starts_with('-')
            || standard.ends_with('-')
            || standard.contains("--")
            || !standard
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '-')
            || shortcode.is_some_and(|shortcode| {
                !(3..=8).contains(&shortcode.len())
                    || !shortcode
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric())
            })
        {
            bail!("Invalid GitHub owner `{owner}`");
        }

        Ok(Self(owner.to_string()))
    }
}

impl fmt::Display for GitHubOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A validated GitHub repository name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubRepoName(String);

impl GitHubRepoName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for GitHubRepoName {
    type Err = Report;

    fn from_str(repo: &str) -> Result<Self> {
        if !(1..=100).contains(&repo.len()) || !repo.is_ascii() {
            bail!("Invalid GitHub repository name: must be 1 to 100 ASCII characters");
        }
        if !repo.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        }) {
            bail!("Invalid GitHub repository name `{repo}`");
        }
        if matches!(repo, "." | "..") || repo.to_ascii_lowercase().ends_with(".git") {
            bail!("Invalid GitHub repository name `{repo}`");
        }

        Ok(Self(repo.to_string()))
    }
}

impl fmt::Display for GitHubRepoName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Represents a parsed registry source identifier.
#[derive(Debug, Clone)]
pub enum RegistrySource {
    GitHub {
        owner: GitHubOwner,
        repo: GitHubRepoName,
    },
}

#[allow(async_fn_in_trait)]
pub trait Registry {
    async fn fetch_skill(&self, skill_name: &SkillName) -> Result<SkillPackage>;
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

    let Some((owner, repo)) = rest
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
        repo: repo.parse()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::{SkillFile, SkillName, SkillPackage};
    use std::path::PathBuf;

    #[test]
    fn github_owner_accepts_standard_boundaries_and_preserves_case() {
        for owner in ["a".to_string(), "A".repeat(39)] {
            let parsed = owner.parse::<GitHubOwner>().expect("valid GitHub owner");
            assert_eq!(parsed.as_str(), owner);
            assert_eq!(parsed.to_string(), owner);
        }
    }

    #[test]
    fn github_owner_accepts_managed_user_suffix_boundaries() {
        for owner in ["octo_abc", "octo_A1b2C3d4", "octo_admin"] {
            assert!(
                owner.parse::<GitHubOwner>().is_ok(),
                "{owner:?} should be valid"
            );
        }
    }

    #[test]
    fn github_owner_rejects_empty_and_overlong_values() {
        for owner in ["".to_string(), "a".repeat(40)] {
            assert!(
                owner.parse::<GitHubOwner>().is_err(),
                "{owner:?} should be invalid"
            );
        }
    }

    #[test]
    fn github_owner_rejects_malformed_standard_names() {
        for owner in [
            "-octo",
            "octo-",
            "octo--cat",
            "octo.cat",
            "octo cat",
            "octocat!",
            "octöcat",
        ] {
            assert!(
                owner.parse::<GitHubOwner>().is_err(),
                "{owner:?} should be invalid"
            );
        }
    }

    #[test]
    fn github_owner_rejects_malformed_managed_names() {
        for owner in [
            "_admin",
            "octo_ab",
            "octo_abcdefghi",
            "octo_ab-c",
            "octo__admin",
            "octo_admin_extra",
            "-octo_admin",
            "octo-_admin",
        ] {
            assert!(
                owner.parse::<GitHubOwner>().is_err(),
                "{owner:?} should be invalid"
            );
        }
    }

    #[test]
    fn github_repo_name_accepts_boundaries_and_preserves_case() {
        for repo in [
            "a".to_string(),
            "Repo.Name-1_test".to_string(),
            "r".repeat(100),
        ] {
            let parsed = repo
                .parse::<GitHubRepoName>()
                .expect("valid GitHub repository name");
            assert_eq!(parsed.as_str(), repo);
            assert_eq!(parsed.to_string(), repo);
        }
    }

    #[test]
    fn github_repo_name_rejects_empty_and_overlong_values() {
        for repo in ["".to_string(), "r".repeat(101)] {
            assert!(
                repo.parse::<GitHubRepoName>().is_err(),
                "{repo:?} should be invalid"
            );
        }
    }

    #[test]
    fn github_repo_name_rejects_relative_names_and_git_suffix() {
        for repo in [".", "..", "repo.git", "repo.GIT", ".git"] {
            assert!(
                repo.parse::<GitHubRepoName>().is_err(),
                "{repo:?} should be invalid"
            );
        }
        assert!("repo.gitignore".parse::<GitHubRepoName>().is_ok());
    }

    #[test]
    fn github_repo_name_rejects_forbidden_or_non_ascii_characters() {
        for repo in ["repo/name", "repo name", "repo@name", "répo"] {
            assert!(
                repo.parse::<GitHubRepoName>().is_err(),
                "{repo:?} should be invalid"
            );
        }
    }

    struct StaticRegistry;

    impl Registry for StaticRegistry {
        async fn fetch_skill(&self, skill_name: &SkillName) -> Result<SkillPackage> {
            Ok(SkillPackage {
                name: skill_name.clone(),
                revision: "abc123".to_string(),
                files: vec![SkillFile {
                    relative_path: PathBuf::from("SKILL.md"),
                    bytes: b"# Test Skill\n".to_vec(),
                    executable: false,
                }],
            })
        }
    }

    #[tokio::test]
    async fn registry_trait_fetches_skill_package() {
        let name = "test-skill".parse::<SkillName>().expect("valid skill name");
        let skill = StaticRegistry
            .fetch_skill(&name)
            .await
            .expect("registry should fetch skill package");

        assert_eq!(skill.name.as_str(), "test-skill");
        assert_eq!(skill.revision, "abc123");
        assert_eq!(skill.files[0].bytes, b"# Test Skill\n");
    }

    #[test]
    fn parse_source_rejects_empty_input() {
        assert!(parse_source("").is_err());
    }

    #[test]
    fn parse_source_accepts_github_shorthand() {
        let source = parse_source("github:owner/repo").expect("source should parse");

        assert!(matches!(
            source,
            RegistrySource::GitHub { owner, repo }
                if owner.as_str() == "owner" && repo.as_str() == "repo"
        ));
    }

    #[test]
    fn parse_source_accepts_managed_github_owner() {
        let source = parse_source("github:octo_admin/repo").expect("source should parse");

        assert!(matches!(
            source,
            RegistrySource::GitHub { owner, repo }
                if owner.as_str() == "octo_admin" && repo.as_str() == "repo"
        ));
    }

    #[test]
    fn parse_source_accepts_https_url() {
        let source = parse_source("https://github.com/owner/repo").expect("source should parse");

        assert!(matches!(
            source,
            RegistrySource::GitHub { owner, repo }
                if owner.as_str() == "owner" && repo.as_str() == "repo"
        ));
    }

    #[test]
    fn parse_source_accepts_trailing_slash() {
        let source = parse_source("github:owner/repo/").expect("source should parse");

        assert!(matches!(
            source,
            RegistrySource::GitHub { owner, repo }
                if owner.as_str() == "owner" && repo.as_str() == "repo"
        ));
    }

    #[test]
    fn parse_source_rejects_malformed_github_owners() {
        for source in [
            "github:-owner/repo",
            "github:owner-/repo",
            "github:own--er/repo",
            "github:owner.name/repo",
        ] {
            assert!(parse_source(source).is_err(), "{source} should be invalid");
        }
    }

    #[test]
    fn parse_source_rejects_malformed_managed_github_owners() {
        for source in [
            "github:owner_ab/repo",
            "github:owner_abcdefghi/repo",
            "github:owner_ab-c/repo",
            "github:owner__admin/repo",
        ] {
            assert!(parse_source(source).is_err(), "{source} should be invalid");
        }
    }

    #[test]
    fn parse_source_rejects_invalid_github_repository_names() {
        for source in [
            "github:owner/.",
            "github:owner/..",
            "github:owner/repo.git",
            "github:owner/repo@name",
            "github:owner/répo",
        ] {
            assert!(parse_source(source).is_err(), "{source} should be invalid");
        }
    }

    #[test]
    fn parse_source_rejects_excessive_github_identifier_lengths() {
        let owner = "o".repeat(40);
        let repo = "r".repeat(101);

        assert!(parse_source(&format!("github:{owner}/repo")).is_err());
        assert!(parse_source(&format!("github:owner/{repo}")).is_err());
    }

    #[test]
    fn parse_source_rejects_invalid_formats() {
        for source in [
            "owner/repo",
            "github:owner",
            "github:/repo",
            "github:owner/",
            "github:owner/repo/extra",
            "https://gitlab.com/owner/repo",
        ] {
            assert!(parse_source(source).is_err(), "{source} should be invalid");
        }
    }
}
