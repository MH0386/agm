use crate::registry::github::{GitHubOwner, GitHubRepoName};
use crate::skills::{SkillName, SkillPackage};
use color_eyre::eyre::{Result, bail};
pub mod github;

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
    async fn fetch_skill(&self, name: &SkillName) -> Result<SkillPackage>;
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
