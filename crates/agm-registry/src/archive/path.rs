//! Archive path normalization and raw-path safety checks.

use agm_core::skills::validate_skill_relative_path;
use color_eyre::eyre::{ContextCompat, Result, bail};
use std::{
    ffi::OsString,
    path::{Component, Path, PathBuf},
};

/// Strips the archive's common top-level directory and returns a repository-relative path.
///
/// Returns `Ok(None)` for the top-level directory entry itself.
pub(super) fn normalize_repository_path(
    effective_path: &Path,
    raw_path: &[u8],
    is_directory: bool,
    top_level: &mut Option<OsString>,
) -> Result<Option<PathBuf>> {
    validate_raw_archive_path(raw_path, is_directory, effective_path)?;
    let mut components = effective_path.components();
    let first = components.next().with_context(|| {
        format!(
            "Archive entry has an empty path: `{}`",
            effective_path.display()
        )
    })?;
    let Component::Normal(first) = first else {
        bail!(
            "Archive entry path `{}` has no normal top-level component",
            effective_path.display()
        );
    };

    match top_level {
        Some(expected) if expected != first => bail!(
            "Archive entry `{}` does not share top-level directory `{}`",
            effective_path.display(),
            expected.to_string_lossy()
        ),
        Some(_) => {}
        None => *top_level = Some(first.to_os_string()),
    }

    let repository_path = components.collect::<PathBuf>();
    if repository_path.as_os_str().is_empty() {
        if is_directory {
            return Ok(None);
        }
        bail!(
            "Archive entry `{}` must be beneath one top-level directory",
            effective_path.display()
        );
    }
    validate_skill_relative_path(&repository_path)?;
    Ok(Some(repository_path))
}

/// Rejects absolute, empty, dot, parent, and backslash path components in raw tar bytes.
fn validate_raw_archive_path(
    raw_path: &[u8],
    is_directory: bool,
    repository_path: &Path,
) -> Result<()> {
    let displayed_path = String::from_utf8_lossy(raw_path);
    if raw_path.is_empty() {
        bail!(
            "Unsafe archive path `{displayed_path}` (effective `{}`): path is empty",
            repository_path.display()
        );
    }
    if raw_path.starts_with(b"/") {
        bail!(
            "Unsafe archive path `{displayed_path}` (effective `{}`): absolute paths are not \
             allowed",
            repository_path.display()
        );
    }
    if raw_path.contains(&b'\\') {
        bail!(
            "Unsafe archive path `{displayed_path}` (effective `{}`): backslashes are not allowed",
            repository_path.display()
        );
    }

    let components = raw_path.split(|byte| *byte == b'/').collect::<Vec<_>>();
    for (index, component) in components.iter().enumerate() {
        if component.is_empty() {
            let is_allowed_directory_suffix =
                is_directory && index + 1 == components.len() && index > 0;
            if !is_allowed_directory_suffix {
                bail!(
                    "Unsafe archive path `{displayed_path}` (effective `{}`): empty path \
                     components are not allowed",
                    repository_path.display()
                );
            }
        } else if *component == b"." || *component == b".." {
            bail!(
                "Unsafe archive path `{displayed_path}` (effective `{}`): `.` and `..` components \
                 are not allowed",
                repository_path.display()
            );
        }
    }

    Ok(())
}
