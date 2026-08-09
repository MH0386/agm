//! Helper functions for skill package discovery over an in-memory repository `.tar.gz`.

use agm_core::skills::{SkillFileInformation, SkillName, SkillPackage, SkillPackageFile};
use color_eyre::eyre::{Context, Result, bail};
use flate2::read::GzDecoder;
use std::io::{Cursor, Read};
use std::str::FromStr;
use std::{ffi::OsStr, path::PathBuf};
use tar::{Archive, Entries, EntryType};
use tokio::io;

/// Opens a gzip reader over the in-memory archive and rejects a missing gzip header.
fn gzip_reader<'a>(archive_bytes: &'a [u8]) -> Result<impl Read + 'a> {
    let decoder = GzDecoder::new(Cursor::new(archive_bytes));
    if decoder.header().is_none() {
        bail!("Missing gzip header");
    }
    Ok(decoder)
}

#[derive(Clone)]
struct SkillEntry {
    path: PathBuf,
    bytes: Vec<u8>,
    permissions: u32,
}

struct SkillArchivedEntries {
    entries: Vec<SkillEntry>,
}

impl SkillArchivedEntries {
    fn get_skill_root_entries(&self) -> Result<Vec<SkillEntry>> {
        Ok(self
            .entries
            .iter()
            .filter(|entry| entry.path.file_name() == Some(OsStr::new("SKILL.md")))
            .cloned()
            .collect::<Vec<_>>())
    }
    fn get_skill_files(&self, skill_entry: &SkillEntry) -> Result<Vec<SkillEntry>> {
        Ok(self
            .entries
            .iter()
            .filter(|entry| entry.path.starts_with(skill_entry.path.parent().unwrap()))
            .cloned()
            .collect::<Vec<_>>())
    }
}

impl<R: Read> From<Entries<'_, R>> for SkillArchivedEntries {
    fn from(entries: Entries<'_, R>) -> Self {
        let entries = entries
            .into_iter()
            .map(|entry| entry.unwrap())
            .filter_map(|mut entry| {
                // Read the bytes of the entry
                let bytes = (&mut entry)
                    .bytes()
                    .collect::<io::Result<Vec<u8>>>()
                    .unwrap();
                let permissions = entry.header().mode().unwrap(); // 0o644 for regular files, 0o755 for executable files
                // Strip the top-level archive prefix from the path
                let prefix: PathBuf = entry
                    .path()
                    .unwrap()
                    .components()
                    .next()
                    .unwrap()
                    .as_os_str()
                    .into();
                if entry.header().entry_type() != EntryType::Regular {
                    return None;
                }
                let entry_path: PathBuf = entry.path().unwrap().into();
                let path: PathBuf = entry_path.strip_prefix(prefix).unwrap().into();
                Some(SkillEntry {
                    path,
                    bytes,
                    permissions,
                })
            })
            .collect::<Vec<SkillEntry>>();
        Self { entries }
    }
}

/// Parses a skill package from a buffered repository tarball using production limits.
pub async fn parse_skill_package(
    archive_bytes: &[u8],
    requested_name: SkillName,
    revision: String,
) -> Result<SkillPackage> {
    let decoder = gzip_reader(archive_bytes)?;
    let mut archive = Archive::new(decoder);
    let entries = archive
        .entries()
        .context("Failed to initialize tar iteration while parsing skill package")?;
    let mut files = collect_skill_files(entries).await?;
    files.retain(|file| {
        SkillName::from_str(
            file.get_relative_path()
                .parent()
                .unwrap()
                .to_str()
                .unwrap()
                .trim_start_matches(
                    file.get_relative_path()
                        .parent()
                        .unwrap()
                        .parent()
                        .unwrap()
                        .to_str()
                        .unwrap(),
                )
                .trim_start_matches("/"),
        )
        .map_or(false, |name| name == requested_name)
    });
    Ok(SkillPackage {
        name: requested_name,
        revision,
        files,
    })
}

async fn collect_skill_files<R: Read>(entries: Entries<'_, R>) -> Result<Vec<SkillPackageFile>> {
    let skill_entries = SkillArchivedEntries::from(entries);
    let skill_root_entries = skill_entries.get_skill_root_entries()?;
    let mut skill_files = Vec::new();

    for root_entry in skill_root_entries {
        let skill_entries = skill_entries.get_skill_files(&root_entry)?;
        for skill_entry in skill_entries {
            let information = SkillFileInformation::new(
                &skill_entry.path,
                &skill_entry.bytes,
                skill_entry.permissions,
            );
            match information.relative_path.file_name() {
                Some(file_name) if file_name == OsStr::new("SKILL.md") => {
                    skill_files.push(SkillPackageFile::new_root(information)?)
                }
                Some(file_name) if file_name != OsStr::new("SKILL.md") => {
                    skill_files.push(SkillPackageFile::new_non_root(information))
                }
                _ => bail!(
                    "Unsupported file type: {}",
                    information.relative_path.display()
                ),
            }
        }
    }

    Ok(skill_files.into_iter().collect::<Vec<SkillPackageFile>>())
}
