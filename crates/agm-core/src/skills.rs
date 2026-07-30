use color_eyre::eyre::{Context, Report, Result, bail, eyre};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt::{self, Display, Formatter},
    ops::Deref,
    path::{Path, PathBuf},
    str::FromStr,
};
use yaml_serde::from_str;

/// A validated skill name that is safe to use as a single path component.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SkillName(String);

impl AsRef<Path> for SkillName {
    fn as_ref(&self) -> &Path {
        Path::new(&self.0)
    }
}

impl Deref for SkillName {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for SkillName {
    type Err = Report;

    fn from_str(name: &str) -> Result<Self> {
        // Must be 1-64 characters.
        if name.is_empty() || name.trim().is_empty() {
            bail!("Skill name must not be empty");
        } else if name.len() > 64 {
            bail!(
                "Skill name must be 1-64 characters, got {:?} ({})",
                name,
                name.len()
            );
        }

        // May only contain unicode lowercase alphanumeric characters (a-z, 0-9) and hyphens (-).
        if !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            bail!(
                "Invalid skill name `{name}`: must only contain lowercase alphanumeric characters and hyphens"
            );
        }

        // Must not start or end with a hyphen (-).
        if name.starts_with('-') || name.ends_with('-') {
            bail!("Invalid skill name `{name}`: must not start or end with a hyphen");
        }

        // Must not contain consecutive hyphens (--).
        if name.contains("--") {
            bail!("Invalid skill name `{name}`: must not contain consecutive hyphens");
        }

        Ok(Self(name.to_string()))
    }
}

impl Display for SkillName {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SkillDescription(String);

impl Deref for SkillDescription {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for SkillDescription {
    type Err = Report;

    fn from_str(description: &str) -> Result<Self> {
        // Must be 1-1024 characters.
        if description.is_empty() || description.trim().is_empty() {
            bail!("Skill description must not be empty");
        } else if description.len() > 1024 {
            bail!(
                "Skill description must be 1-1024 characters, got {:?} ({})",
                description,
                description.len()
            );
        }

        Ok(Self(description.to_string()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SkillLicense(String);

impl Deref for SkillLicense {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for SkillLicense {
    type Err = Report;

    fn from_str(license: &str) -> Result<Self> {
        // Must be non-empty, if present.
        if license.is_empty() {
            bail!("Skill license must not be empty");
        }

        Ok(Self(license.to_string()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SkillCompatibility(String);

impl Deref for SkillCompatibility {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for SkillCompatibility {
    type Err = Report;

    fn from_str(compatibility: &str) -> Result<Self> {
        // Must be 1-500 characters if provided.
        if compatibility.is_empty() {
            bail!("Skill compatibility must not be empty");
        } else if compatibility.len() > 500 {
            bail!(
                "Skill compatibility must be 1-500 characters, got {:?} ({})",
                compatibility,
                compatibility.len()
            );
        }

        Ok(Self(compatibility.to_string()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)] // nice JSON representation
pub enum SkillMetadataValue {
    List(Vec<String>),
    Single(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SkillMetadata(HashMap<String, SkillMetadataValue>);

impl Deref for SkillMetadata {
    type Target = HashMap<String, SkillMetadataValue>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for SkillMetadata {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        for (key, value) in &self.0 {
            formatter.write_str(&format!("{key}: {value:?}"))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SkillAllowedTools(Vec<String>);

impl Deref for SkillAllowedTools {
    type Target = Vec<String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for SkillAllowedTools {
    type Err = Report;

    fn from_str(allowed_tools: &str) -> Result<Self> {
        // Must be non-empty, if present.
        if allowed_tools.is_empty() {
            bail!("Skill allowed tools must not be empty");
        }

        Ok(Self(
            allowed_tools
                .split_whitespace()
                .map(|s| s.trim().to_string())
                .collect(),
        ))
    }
}

impl Display for SkillAllowedTools {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.join(" "))
    }
}

/// A complete skill package downloaded from a registry.
#[derive(Debug, Clone)]
pub struct SkillPackage {
    /// Validated skill identifier; must match the frontmatter `name`.
    pub name: SkillName,
    /// Files that make up the skill, including `SKILL.md` and any resources.
    pub files: Vec<SkillPackageFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum SkillPackageFile {
    /// A root file `SKILL.md` file inside a downloaded skill package.
    Root(SkillRootFile),
    /// A non-root file inside a downloaded skill package.
    NonRoot(SkillNonRootFile),
}

impl SkillPackageFile {
    pub fn new_root(information: SkillFileInformation) -> Result<Self> {
        Ok(Self::Root(SkillRootFile::with_bytes(information)?))
    }
    pub fn new_non_root(information: SkillFileInformation) -> Self {
        Self::NonRoot(SkillNonRootFile::new(information))
    }
    pub fn get_relative_path(&self) -> &Path {
        match self {
            Self::Root(root) => &root.information.relative_path,
            Self::NonRoot(non_root) => &non_root.information.relative_path,
        }
    }
    pub fn get_bytes(&self) -> &[u8] {
        match self {
            Self::Root(root) => &root.information.bytes,
            Self::NonRoot(non_root) => &non_root.information.bytes,
        }
    }
    pub fn is_executable(&self) -> bool {
        match self {
            Self::NonRoot(non_root) => (non_root.information.permissions & 0o111) == 0o111,
            Self::Root(_) => false,
        }
    }
    pub fn get_permissions(&self) -> u32 {
        match self {
            Self::NonRoot(non_root) => non_root.information.permissions,
            Self::Root(_) => 0o644,
        }
    }
}

/// Common information about a file in a skill package.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SkillFileInformation {
    /// Path relative to the skill root (e.g. `SKILL.md`, `scripts/run.py`).
    pub relative_path: PathBuf,
    /// Raw contents; may be binary.
    pub bytes: Vec<u8>,
    /// Permissions of the file, if it executable.
    pub permissions: u32,
}

impl SkillFileInformation {
    pub fn new(relative_path: &Path, bytes: &[u8], permissions: u32) -> Self {
        Self {
            relative_path: relative_path.to_path_buf(),
            bytes: bytes.to_vec(),
            permissions,
        }
    }
}

/// A non-root file inside a downloaded skill package.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SkillNonRootFile {
    /// Information about the non-root file.
    pub information: SkillFileInformation,
}

impl SkillNonRootFile {
    pub fn new(information: SkillFileInformation) -> Self {
        Self { information }
    }
}

/// A root file `SKILL.md` file inside a downloaded skill package.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SkillRootFile {
    /// Information about the root file.
    pub information: SkillFileInformation,
    /// Parsed frontmatter from the `SKILL.md` file.
    pub frontmatter: SkillFrontmatter,
    /// Parsed body from the `SKILL.md` file.
    pub body: SkillBody,
}

impl SkillRootFile {
    pub fn new(
        information: SkillFileInformation,
        frontmatter: SkillFrontmatter,
        body: SkillBody,
    ) -> Self {
        Self {
            information,
            frontmatter,
            body,
        }
    }
    // Disassemble into frontmatter and body
    pub fn with_bytes(information: SkillFileInformation) -> Result<Self> {
        let text = String::from_utf8(information.clone().bytes).unwrap();
        let mut lines = text.split_inclusive('\n');
        let opening = lines.next().ok_or(eyre!("Empty SKILL.md"))?;
        if opening.trim_end_matches(['\r', '\n']) != "---" {
            bail!("SKILL.md must begin with YAML frontmatter")
        }
        let start = opening.len();
        let mut end = start;
        for line in lines {
            if line.trim_end_matches(['\r', '\n']) == "---" {
                return Ok(Self {
                    information,
                    frontmatter: from_str::<SkillFrontmatter>(&text[start..end])
                        .context("Failed to parse YAML frontmatter from SKILL.md")?,
                    body: SkillBody::from_str(
                        &text[end
                            .checked_add(line.len())
                            .ok_or(eyre!("Frontmatter offset overflow"))?..],
                    )
                    .context("Failed to parse skill body from SKILL.md")?,
                });
            }
            end = end
                .checked_add(line.len())
                .ok_or(eyre!("Frontmatter offset overflow"))?;
        }

        bail!("SKILL.md has no closing `---` delimiter")
    }
}

/// Required frontmatter fields from the Agent Skills format.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SkillFrontmatter {
    pub name: SkillName,
    pub description: SkillDescription,
    pub license: Option<SkillLicense>,
    pub compatibility: Option<SkillCompatibility>,
    pub metadata: Option<SkillMetadata>,
    pub allowed_tools: Option<SkillAllowedTools>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SkillBody(String);

impl Deref for SkillBody {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for SkillBody {
    type Err = Report;

    fn from_str(body: &str) -> Result<Self> {
        if body.is_empty() {
            bail!("Skill body must not be empty");
        }

        if body
            .lines()
            .next()
            .is_some_and(|line| line.trim_end_matches('\r') == "---")
        {
            bail!("Skill body must not begin with YAML frontmatter");
        }

        Ok(Self(body.to_string()))
    }
}
