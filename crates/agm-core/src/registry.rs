use crate::skills::SkillContent;
use color_eyre::eyre::{Result, bail};

/// Represents a parsed registry source identifier.
#[derive(Debug, Clone)]
pub enum RegistrySource {
    GitHub { owner: String, repo: String },
}

#[allow(async_fn_in_trait)]
pub trait Registry {
    async fn fetch_skill(&self, skill_name: &str) -> Result<SkillContent>;
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
        owner: owner.to_string(),
        repo: repo.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::SkillContent;

    struct StaticRegistry;

    impl Registry for StaticRegistry {
        async fn fetch_skill(&self, skill_name: &str) -> Result<SkillContent> {
            Ok(SkillContent {
                name: skill_name.to_string(),
                content: "# Test Skill\n".to_string(),
                sha: "abc123".to_string(),
                encoding: Some("utf-8".to_string()),
                size: 13,
            })
        }
    }

    #[tokio::test]
    async fn registry_trait_fetches_skill_content() {
        let skill = StaticRegistry
            .fetch_skill("test-skill")
            .await
            .expect("registry should fetch skill content");

        assert_eq!(skill.name, "test-skill");
        assert_eq!(skill.content, "# Test Skill\n");
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
                if owner == "owner" && repo == "repo"
        ));
    }

    #[test]
    fn parse_source_accepts_https_url() {
        let source = parse_source("https://github.com/owner/repo").expect("source should parse");

        assert!(matches!(
            source,
            RegistrySource::GitHub { owner, repo }
                if owner == "owner" && repo == "repo"
        ));
    }

    #[test]
    fn parse_source_accepts_trailing_slash() {
        let source = parse_source("github:owner/repo/").expect("source should parse");

        assert!(matches!(
            source,
            RegistrySource::GitHub { owner, repo }
                if owner == "owner" && repo == "repo"
        ));
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
