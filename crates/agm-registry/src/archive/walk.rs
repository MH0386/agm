//! Shared gzip/tar entry walking helpers used by both archive passes.

use super::{
    limits::{ArchiveLimits, checked_count, checked_total},
    path::normalize_repository_path,
};
use color_eyre::eyre::{Context, ContextCompat, Result, bail};
use flate2::read::GzDecoder;
use std::{
    ffi::OsString,
    io::{self, Cursor, Read},
    path::{Path, PathBuf},
};

/// Wraps a gzip decoder so read failures carry a clearer context string.
struct GzipReadContext<R>(R);

impl<R: Read> Read for GzipReadContext<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.0.read(buffer).map_err(|error| {
            io::Error::new(error.kind(), format!("Failed to read gzip stream: {error}"))
        })
    }
}

/// Per-pass state for top-level directory tracking and repository-wide counters.
#[derive(Debug, Default)]
pub(super) struct ArchivePass {
    top_level: Option<OsString>,
    entry_count: usize,
    declared_regular_bytes: u64,
}

impl ArchivePass {
    /// Normalizes the entry path and updates repository-wide entry/byte counters.
    pub(super) fn inspect_entry<R: Read>(
        &mut self,
        entry: &tar::Entry<'_, R>,
        limits: ArchiveLimits,
        phase: &str,
    ) -> Result<Option<PathBuf>> {
        let entry_type = entry.header().entry_type();
        let effective_path = entry
            .path()
            .with_context(|| format!("Failed to read tar path while {phase}"))?
            .into_owned();
        let raw_path = entry.path_bytes().into_owned();
        let repository_path = normalize_repository_path(
            &effective_path,
            &raw_path,
            entry_type.is_dir(),
            &mut self.top_level,
        )?;
        let counter_path = repository_path.as_deref().unwrap_or_else(|| Path::new("."));

        self.entry_count = checked_count(
            self.entry_count,
            1,
            limits.archive_entries,
            "Archive entry count",
            counter_path,
        )?;
        if entry_type.is_file() {
            self.declared_regular_bytes = checked_total(
                self.declared_regular_bytes,
                entry.size(),
                limits.declared_regular_bytes,
                "Repository declared regular-file bytes",
                counter_path,
            )?;
        }

        Ok(repository_path)
    }
}

/// Opens a gzip reader over the in-memory archive and rejects a missing gzip header.
pub(super) fn gzip_reader<'a>(archive_bytes: &'a [u8], phase: &str) -> Result<impl Read + 'a> {
    let decoder = GzDecoder::new(Cursor::new(archive_bytes));
    if decoder.header().is_none() {
        bail!("Malformed gzip header while {phase}");
    }
    Ok(GzipReadContext(decoder))
}

/// Reads an entry body, enforcing declared size and the supplied byte ceiling.
pub(super) fn read_entry_body<R: Read>(
    entry: &mut tar::Entry<'_, R>,
    declared_size: u64,
    limit: u64,
    label: &str,
    repository_path: &Path,
) -> Result<Vec<u8>> {
    let read_limit = declared_size.checked_add(1).with_context(|| {
        format!(
            "{label} read limit overflow at `{}`; limit is {limit} bytes",
            repository_path.display()
        )
    })?;
    let capacity = usize::try_from(declared_size).with_context(|| {
        format!(
            "{label} size cannot fit memory at `{}`; limit is {limit} bytes",
            repository_path.display()
        )
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    entry
        .take(read_limit)
        .read_to_end(&mut bytes)
        .with_context(|| {
            format!(
                "Failed to read {label} body at `{}`",
                repository_path.display()
            )
        })?;
    let observed_size =
        u64::try_from(bytes.len()).context("Observed archive entry size overflow")?;
    if observed_size > limit {
        bail!(
            "{label} exceeds {limit} bytes at `{}`",
            repository_path.display()
        );
    }
    if observed_size != declared_size {
        bail!(
            "{label} body size mismatch at `{}`: declared {declared_size} bytes, read \
             {observed_size} bytes",
            repository_path.display()
        );
    }
    Ok(bytes)
}
