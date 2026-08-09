//! Helper functions for skill package discovery over an in-memory repository `.tar.gz`.

use agm_core::skills::{SkillFileInformation, SkillName, SkillPackage, SkillPackageFile};
use color_eyre::eyre::{Context, Result, bail};
use flate2::read::GzDecoder;
use std::io::{Cursor, Read};
use std::{ffi::OsStr, path::PathBuf};
use tar::{Archive, Entries, EntryType};
use tokio::io;
use tracing::debug;

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

/// Parses skill packages from an in-memory repository tarball bytes.
pub async fn parse_skill_archive_bytes(archive_bytes: &[u8]) -> Result<Vec<SkillPackage>> {
    let decoder = gzip_reader(archive_bytes)?;
    let mut archive = Archive::new(decoder);
    let entries = archive
        .entries()
        .context("Failed to initialize tar iteration while parsing skill package")?;
    let skill_files = collect_skill_files(entries).await?;
    let skill_names = skill_files
        .iter()
        .filter_map(|file| match file {
            SkillPackageFile::Root(root) => Some(root.frontmatter.name.clone()),
            SkillPackageFile::NonRoot(_) => None,
        })
        .collect::<Vec<SkillName>>();
    debug!("Skill names: {:?}", skill_names);
    debug!(
        "Files: {:?}",
        skill_files
            .iter()
            .map(|file| file.get_relative_path())
            .collect::<Vec<_>>()
    );

    Ok(skill_names
        .into_iter()
        .map(|name| SkillPackage {
            name: name.clone(),
            files: skill_files
                .to_vec()
                .into_iter()
                .filter(|file| match file {
                    SkillPackageFile::Root(root) => root.frontmatter.name == name,
                    SkillPackageFile::NonRoot(non_root) => non_root
                        .information
                        .relative_path
                        .components()
                        .into_iter()
                        .any(|component| component.as_os_str() == name.as_str()),
                })
                .collect::<Vec<_>>(),
        })
        .collect::<Vec<_>>())
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
