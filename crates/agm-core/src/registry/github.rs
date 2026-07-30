use color_eyre::eyre::{Report, Result, bail};
use std::{
    fmt::{self, Display, Formatter},
    str::FromStr,
};

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

impl Display for GitHubOwner {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
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

impl Display for GitHubRepoName {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
