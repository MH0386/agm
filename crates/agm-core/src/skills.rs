use color_eyre::eyre::{Report, Result, bail};
use std::{
    collections::BTreeSet,
    fmt::{self, Display, Formatter},
    path::{Component, Path, PathBuf},
    str::FromStr,
};

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

impl Display for SkillName {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Shared struct holding the project-local and global skill directory paths.
#[derive(Debug, Clone)]
pub struct SkillsDir {
    /// Project-scoped skills directory (for example, `.agents/skills`).
    pub project: PathBuf,
    /// User-global skills directory (for example, `~/.agents/skills`).
    pub global: PathBuf,
}

/// A complete skill package downloaded from a registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillPackage {
    /// Validated skill identifier; must match the frontmatter `name`.
    pub name: SkillName,
    /// Immutable registry revision, such as a Git commit SHA.
    pub revision: String,
    /// Files that make up the skill, including `SKILL.md` and any resources.
    pub files: Vec<SkillFile>,
}

/// A file contained in a downloaded skill package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillFile {
    /// Path relative to the skill root (e.g. `SKILL.md`, `scripts/run.py`).
    pub relative_path: PathBuf,
    /// Raw file contents; may be binary.
    pub bytes: Vec<u8>,
    /// Whether the archive recorded any Unix execute bits for this file.
    pub executable: bool,
}

/// Validates a package-relative file path before it is joined to a destination.
pub fn validate_skill_relative_path(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| match component {
            Component::Normal(value) => value.to_string_lossy().contains('\\'),
            _ => true,
        })
    {
        bail!(
            "Unsafe skill package path `{}`: paths must be relative normal components",
            path.display()
        );
    }

    Ok(())
}

impl SkillPackage {
    /// Revalidates package invariants at the registry/installer boundary.
    pub fn validate(&self) -> Result<()> {
        let mut paths = BTreeSet::new();
        let mut root_skill_files = 0_u8;

        for file in &self.files {
            validate_skill_relative_path(&file.relative_path)?;
            if !paths.insert(file.relative_path.clone()) {
                bail!(
                    "Duplicate skill package path `{}`",
                    file.relative_path.display()
                );
            }
            if file.relative_path == Path::new("SKILL.md") {
                root_skill_files = root_skill_files
                    .checked_add(1)
                    .expect("file count cannot overflow before duplicate detection");
            }
        }

        if root_skill_files != 1 {
            bail!(
                "Skill package must contain exactly one root `SKILL.md`; found {root_skill_files}"
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package_with_paths(paths: &[&str]) -> SkillPackage {
        SkillPackage {
            name: "test-skill".parse().expect("valid skill name"),
            revision: "abc123".to_string(),
            files: paths
                .iter()
                .map(|path| SkillFile {
                    relative_path: PathBuf::from(path),
                    bytes: b"content".to_vec(),
                    executable: false,
                })
                .collect(),
        }
    }

    #[test]
    fn skill_package_requires_exactly_one_root_skill_file() {
        assert!(package_with_paths(&[]).validate().is_err());
        assert!(package_with_paths(&["nested/SKILL.md"]).validate().is_err());
        assert!(package_with_paths(&["SKILL.md"]).validate().is_ok());
        assert!(
            package_with_paths(&["SKILL.md", "SKILL.md"])
                .validate()
                .is_err()
        );
    }

    #[test]
    fn skill_package_rejects_unsafe_and_duplicate_file_paths() {
        for path in ["", "../escape", "/absolute", r"assets\escape"] {
            assert!(
                package_with_paths(&["SKILL.md", path]).validate().is_err(),
                "{path:?} should be rejected"
            );
        }
        assert!(
            package_with_paths(&["SKILL.md", "assets/icon.png", "assets/icon.png"])
                .validate()
                .is_err()
        );
    }

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
