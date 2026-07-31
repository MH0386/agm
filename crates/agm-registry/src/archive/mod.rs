//! Two-pass skill package discovery over an in-memory repository `.tar.gz`.

mod collect;
mod discover;
mod frontmatter;
mod limits;
mod path;
mod walk;

#[cfg(test)]
mod tests;

use collect::collect_skill_files;
use discover::discover_skill_root;
use limits::ArchiveLimits;

use agm_core::skills::{SkillName, SkillPackage};
use color_eyre::eyre::Result;

#[cfg(test)]
use limits::checked_total;

/// Parses a skill package from a buffered repository tarball using production limits.
pub fn parse_skill_package(
    archive_bytes: &[u8],
    requested_name: SkillName,
    revision: String,
) -> Result<SkillPackage> {
    parse_skill_package_with_limits(
        archive_bytes,
        requested_name,
        revision,
        ArchiveLimits::PRODUCTION,
    )
}

/// Same as [`parse_skill_package`], with injectable limits for focused tests.
fn parse_skill_package_with_limits(
    archive_bytes: &[u8],
    requested_name: SkillName,
    revision: String,
    limits: ArchiveLimits,
) -> Result<SkillPackage> {
    let skill_root = discover_skill_root(archive_bytes, &requested_name, limits)?;
    let files = collect_skill_files(archive_bytes, &skill_root, limits)?;
    let package = SkillPackage {
        name: requested_name,
        revision,
        files,
    };
    package.validate()?;
    Ok(package)
}
