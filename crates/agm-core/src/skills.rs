use color_eyre::eyre::{Report, Result, bail};
use std::{fmt, path::PathBuf, str::FromStr};

/// A validated skill name that is safe to use as a single path component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillName(String);

impl SkillName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for SkillName {
    type Err = Report;

    fn from_str(name: &str) -> Result<Self> {
        if name.is_empty() {
            bail!("Skill name must not be empty");
        }
        if name == "." || name == ".." {
            bail!("Invalid skill name `{name}`: must not be `.` or `..`");
        }

        const FORBIDDEN: &[char] = &['/', '\\', ':', '<', '>', '"', '|', '?', '*', '\0'];
        if let Some(character) = name.chars().find(|character| FORBIDDEN.contains(character)) {
            bail!("Invalid skill name `{name}`: contains forbidden character `{character}`");
        }

        Ok(Self(name.to_string()))
    }
}

impl fmt::Display for SkillName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Shared struct holding the project-local and global skill directory paths.
#[derive(Debug, Clone)]
pub struct SkillsDir {
    pub project: PathBuf,
    pub global: PathBuf,
}

/// Represents the content of a downloaded skill.
#[derive(Debug)]
pub struct SkillContent {
    pub name: SkillName,
    pub content: String,
    pub sha: String,
    pub encoding: Option<String>,
    pub size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_name_preserves_valid_input() {
        let name = "My-skill_1".parse::<SkillName>().expect("valid skill name");

        assert_eq!(name.as_str(), "My-skill_1");
        assert_eq!(name.to_string(), "My-skill_1");
    }

    #[test]
    fn skill_name_rejects_empty_and_relative_components() {
        for name in ["", ".", ".."] {
            assert!(
                name.parse::<SkillName>().is_err(),
                "{name:?} should be invalid"
            );
        }
    }

    #[test]
    fn skill_name_rejects_every_cross_platform_forbidden_character() {
        for forbidden in ['/', '\\', ':', '<', '>', '"', '|', '?', '*', '\0'] {
            let name = format!("skill{forbidden}name");
            assert!(
                name.parse::<SkillName>().is_err(),
                "{name:?} should be invalid"
            );
        }
    }
}
