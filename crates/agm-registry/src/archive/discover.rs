//! Pass 1: locate the skill root by matching `SKILL.md` frontmatter `name`.

use super::{
    frontmatter::{SkillFrontmatter, frontmatter_yaml},
    limits::ArchiveLimits,
    walk::{ArchivePass, gzip_reader, read_entry_body},
};
use agm_core::skills::SkillName;
use color_eyre::eyre::{Context, ContextCompat, Result, bail};
use std::{ffi::OsStr, path::PathBuf};

/// Finds the unique skill root whose frontmatter `name` equals `requested_name`.
pub(super) fn discover_skill_root(
    archive_bytes: &[u8],
    requested_name: &SkillName,
    limits: ArchiveLimits,
) -> Result<PathBuf> {
    let decoder = gzip_reader(archive_bytes, "discovering a skill")?;
    let mut archive = tar::Archive::new(decoder);
    let entries = archive
        .entries()
        .context("Failed to initialize tar iteration during skill discovery")?;
    let mut pass = ArchivePass::default();
    let mut candidates = Vec::new();

    for entry in entries {
        let mut entry = entry.context("Failed to read tar header during skill discovery")?;
        let entry_type = entry.header().entry_type();
        let Some(repository_path) = pass.inspect_entry(&entry, limits, "discovering a skill")?
        else {
            continue;
        };

        if !entry_type.is_file() || repository_path.file_name() != Some(OsStr::new("SKILL.md")) {
            continue;
        }

        let declared_size = entry.size();
        if declared_size > limits.skill_markdown_bytes {
            bail!(
                "SKILL.md discovery size exceeds {} bytes at `{}`",
                limits.skill_markdown_bytes,
                repository_path.display()
            );
        }
        let bytes = read_entry_body(
            &mut entry,
            declared_size,
            limits.skill_markdown_bytes,
            "SKILL.md discovery",
            &repository_path,
        )?;
        let text = std::str::from_utf8(&bytes).with_context(|| {
            format!(
                "SKILL.md at `{}` is not valid UTF-8",
                repository_path.display()
            )
        })?;
        let yaml = frontmatter_yaml(text, &repository_path)?;
        let frontmatter: SkillFrontmatter = yaml_serde::from_str(yaml).with_context(|| {
            format!(
                "Invalid YAML frontmatter in SKILL.md at `{}`",
                repository_path.display()
            )
        })?;
        if frontmatter.description.trim().is_empty() {
            bail!(
                "SKILL.md at `{}` must have a non-empty description",
                repository_path.display()
            );
        }
        if frontmatter.name == requested_name.as_str() {
            candidates.push(repository_path);
        }
    }

    candidates.sort();
    match candidates.as_slice() {
        [] => bail!("Skill `{requested_name}` not found in repository archive"),
        [skill_path] => Ok(skill_path
            .parent()
            .context("Matched SKILL.md path had no parent")?
            .to_path_buf()),
        _ => {
            let paths = candidates
                .iter()
                .map(|path| format!("`{}`", path.display()))
                .collect::<Vec<_>>()
                .join(", ");
            bail!("Skill `{requested_name}` is ambiguous; matching SKILL.md paths: {paths}")
        }
    }
}
