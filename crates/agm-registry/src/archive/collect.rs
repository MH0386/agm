//! Pass 2: collect every regular file under the selected skill root.

use super::{
    limits::{ArchiveLimits, checked_count, checked_total},
    walk::{ArchivePass, gzip_reader, read_entry_body},
};
use agm_core::skills::{SkillFile, validate_skill_relative_path};
use color_eyre::eyre::{Context, Result, bail};
use std::{collections::BTreeSet, path::Path};

/// Collects sorted package files beneath `skill_root`, including binary and executable files.
pub(super) fn collect_skill_files(
    archive_bytes: &[u8],
    skill_root: &Path,
    limits: ArchiveLimits,
) -> Result<Vec<SkillFile>> {
    let decoder = gzip_reader(archive_bytes, "collecting a skill package")?;
    let mut archive = tar::Archive::new(decoder);
    let entries = archive
        .entries()
        .context("Failed to initialize tar iteration while collecting package")?;
    let mut pass = ArchivePass::default();
    let mut files = Vec::new();
    let mut paths = BTreeSet::new();
    let mut selected_file_count = 0;
    let mut selected_package_bytes = 0;

    for entry in entries {
        let mut entry = entry.context("Failed to read tar header while collecting package")?;
        let entry_type = entry.header().entry_type();
        let Some(repository_path) =
            pass.inspect_entry(&entry, limits, "collecting a skill package")?
        else {
            continue;
        };
        let Ok(relative_path) = repository_path.strip_prefix(skill_root) else {
            continue;
        };
        if entry_type.is_dir() {
            continue;
        }
        if !entry_type.is_file() {
            bail!(
                "Unsupported archive entry type {:?} at `{}` inside selected package",
                entry_type,
                repository_path.display()
            );
        }
        if relative_path.as_os_str().is_empty() {
            bail!(
                "Selected regular file `{}` has an empty package-relative path",
                repository_path.display()
            );
        }
        validate_skill_relative_path(relative_path)?;

        selected_file_count = checked_count(
            selected_file_count,
            1,
            limits.selected_package_files,
            "Selected package file count",
            &repository_path,
        )?;
        let declared_size = entry.size();
        if declared_size > limits.selected_file_bytes {
            bail!(
                "Selected file size exceeds {} bytes at `{}`",
                limits.selected_file_bytes,
                repository_path.display()
            );
        }
        selected_package_bytes = checked_total(
            selected_package_bytes,
            declared_size,
            limits.selected_package_bytes,
            "Selected package bytes",
            &repository_path,
        )?;

        let relative_path = relative_path.to_path_buf();
        if !paths.insert(relative_path.clone()) {
            bail!(
                "Duplicate normalized package path `{}` at repository path `{}`",
                relative_path.display(),
                repository_path.display()
            );
        }

        let executable = entry.header().mode().with_context(|| {
            format!(
                "Failed to read mode for archive entry `{}`",
                repository_path.display()
            )
        })? & 0o111
            != 0;
        let bytes = read_entry_body(
            &mut entry,
            declared_size,
            limits.selected_file_bytes,
            "selected archive entry",
            &repository_path,
        )?;
        files.push(SkillFile {
            relative_path,
            bytes,
            executable,
        });
    }

    if !paths.contains(Path::new("SKILL.md")) {
        bail!("Selected package must contain exactly one root `SKILL.md`");
    }
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(files)
}
