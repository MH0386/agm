use color_eyre::eyre::{Result, bail};
use std::path::PathBuf;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_skill_name_rejects_traversal() {
        assert!(validate_skill_name("..").is_err());
        assert!(validate_skill_name("a/b").is_err());
        assert!(validate_skill_name("ok-name").is_ok());
    }
}
