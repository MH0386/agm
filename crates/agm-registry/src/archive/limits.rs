//! Fixed archive resource limits and checked arithmetic helpers.

use color_eyre::eyre::{ContextCompat, Result, bail};
use std::path::Path;

/// Resource ceilings applied while scanning a repository archive.
#[derive(Debug, Clone, Copy)]
pub(super) struct ArchiveLimits {
    /// Maximum bytes read from any inspected `SKILL.md` during discovery.
    pub(super) skill_markdown_bytes: u64,
    /// Maximum bytes for one selected package file.
    pub(super) selected_file_bytes: u64,
    /// Maximum total bytes for the selected package.
    pub(super) selected_package_bytes: u64,
    /// Maximum number of regular files in the selected package.
    pub(super) selected_package_files: usize,
    /// Maximum number of archive entries inspected per pass.
    pub(super) archive_entries: usize,
    /// Maximum total declared regular-file bytes across the whole repository.
    pub(super) declared_regular_bytes: u64,
}

impl ArchiveLimits {
    /// Production limits from the approved skill-package discovery design.
    pub(super) const PRODUCTION: Self = Self {
        skill_markdown_bytes: 1024 * 1024,
        selected_file_bytes: 64 * 1024 * 1024,
        selected_package_bytes: 256 * 1024 * 1024,
        selected_package_files: 10_000,
        archive_entries: 100_000,
        declared_regular_bytes: 1024 * 1024 * 1024,
    };
}

/// Adds `increment` to an entry/file count with overflow and ceiling checks.
pub(super) fn checked_count(
    current: usize,
    increment: usize,
    limit: usize,
    label: &str,
    repository_path: &Path,
) -> Result<usize> {
    let total = current.checked_add(increment).with_context(|| {
        format!(
            "{label} overflow while inspecting `{}`; limit is {limit}",
            repository_path.display()
        )
    })?;
    if total > limit {
        bail!("{label} exceeds {limit} at `{}`", repository_path.display());
    }
    Ok(total)
}

/// Adds `increment` to a byte total with overflow and ceiling checks.
pub(super) fn checked_total(
    current: u64,
    increment: u64,
    limit: u64,
    label: &str,
    repository_path: &Path,
) -> Result<u64> {
    let total = current.checked_add(increment).with_context(|| {
        format!(
            "{label} overflow while inspecting `{}`; limit is {limit} bytes",
            repository_path.display()
        )
    })?;
    if total > limit {
        bail!(
            "{label} exceeds {limit} bytes at `{}`",
            repository_path.display()
        );
    }
    Ok(total)
}
