//! Synthetic-archive tests for skill package discovery.

use super::{
    checked_total, limits::ArchiveLimits, parse_skill_package, parse_skill_package_with_limits,
};
use agm_core::skills::SkillName;
use std::path::PathBuf;

#[derive(Debug)]
struct TestEntry {
    path: String,
    bytes: Vec<u8>,
    mode: u32,
    entry_type: tar::EntryType,
}

impl TestEntry {
    fn file(path: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            path: path.into(),
            bytes: bytes.into(),
            mode: 0o644,
            entry_type: tar::EntryType::Regular,
        }
    }

    fn executable(path: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            mode: 0o755,
            ..Self::file(path, bytes)
        }
    }

    fn directory(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            bytes: Vec::new(),
            mode: 0o755,
            entry_type: tar::EntryType::Directory,
        }
    }
}

fn skill_name() -> SkillName {
    "find-skills".parse().expect("valid skill name")
}

fn skill_markdown(name: &str, description: &str) -> Vec<u8> {
    format!("---\nname: {name}\ndescription: {description}\n---\n# {name}\n").into_bytes()
}

fn tar_gz(entries: &[TestEntry]) -> Vec<u8> {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use tar::{Builder, Header};

    let encoder = GzEncoder::new(Vec::new(), Compression::default());
    let mut builder = Builder::new(encoder);
    for entry in entries {
        let mut header = Header::new_gnu();
        header
            .set_path(&entry.path)
            .expect("fixture path should fit");
        header.set_size(entry.bytes.len() as u64);
        header.set_mode(entry.mode);
        header.set_entry_type(entry.entry_type);
        header.set_cksum();
        builder
            .append(&header, entry.bytes.as_slice())
            .expect("fixture entry should append");
    }
    builder
        .into_inner()
        .expect("tar should finish")
        .finish()
        .expect("gzip should finish")
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).expect("gzip input should write");
    encoder.finish().expect("gzip should finish")
}

fn raw_header(path: &[u8], entry_type: u8, size: u64) -> tar::Header {
    assert!(
        path.len() <= 100,
        "fixture path must fit the GNU name field"
    );
    let mut header = tar::Header::new_gnu();
    header.set_size(size);
    header.set_mode(0o644);
    header.set_entry_type(tar::EntryType::new(entry_type));
    let name = &mut header.as_mut_bytes()[..100];
    name.fill(0);
    name[..path.len()].copy_from_slice(path);
    if matches!(entry_type, b'1' | b'2') {
        header
            .set_link_name("target")
            .expect("fixture link target should fit");
    }
    if entry_type == b'S' {
        header
            .as_gnu_mut()
            .expect("fixture uses a GNU header")
            .set_real_size(0);
    }
    header.set_cksum();
    header
}

fn raw_tar_gz(entries: Vec<(tar::Header, Vec<u8>)>) -> Vec<u8> {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use tar::Builder;

    let encoder = GzEncoder::new(Vec::new(), Compression::default());
    let mut builder = Builder::new(encoder);
    for (header, bytes) in entries {
        builder
            .append(&header, bytes.as_slice())
            .expect("raw fixture entry should append");
    }
    builder
        .into_inner()
        .expect("tar should finish")
        .finish()
        .expect("gzip should finish")
}

fn generous_limits() -> ArchiveLimits {
    ArchiveLimits {
        skill_markdown_bytes: 1_024,
        selected_file_bytes: 1_024,
        selected_package_bytes: 4_096,
        selected_package_files: 100,
        archive_entries: 100,
        declared_regular_bytes: 4_096,
    }
}

fn assert_limit_error(error: &color_eyre::Report, path: &str, limit: impl ToString) {
    let message = format!("{error:?}");
    assert!(message.contains(path), "{message}");
    assert!(message.contains(&limit.to_string()), "{message}");
}

#[test]
fn discovers_root_skill_and_returns_complete_sorted_package() {
    let archive = tar_gz(&[
        TestEntry::file("repo-sha/assets/icon.bin", vec![0, 1, 255]),
        TestEntry::file(
            "repo-sha/SKILL.md",
            skill_markdown("find-skills", "Find skills"),
        ),
        TestEntry::file("repo-sha/README.md", b"repository readme".to_vec()),
    ]);

    let package = parse_skill_package(&archive, skill_name(), "abc123".to_string())
        .expect("root skill should be discovered");

    assert_eq!(package.name.as_str(), "find-skills");
    assert_eq!(package.revision, "abc123");
    assert_eq!(
        package
            .files
            .iter()
            .map(|file| file.relative_path.clone())
            .collect::<Vec<_>>(),
        [
            PathBuf::from("README.md"),
            PathBuf::from("SKILL.md"),
            PathBuf::from("assets/icon.bin"),
        ]
    );
}

#[test]
fn discovers_flat_categorized_hidden_and_monorepo_layouts() {
    for skill_path in [
        "repo-sha/SKILL.md",
        "repo-sha/skills/find-skills/SKILL.md",
        "repo-sha/skills/category/find-skills/SKILL.md",
        "repo-sha/skills/.curated/find-skills/SKILL.md",
        "repo-sha/plugins/catalog/skills/find-skills/SKILL.md",
    ] {
        let archive = tar_gz(&[TestEntry::file(
            skill_path,
            skill_markdown("find-skills", "Find skills"),
        )]);

        let package = parse_skill_package(&archive, skill_name(), "abc123".to_string())
            .unwrap_or_else(|error| panic!("{skill_path} should be discovered: {error:?}"));

        assert_eq!(package.files.len(), 1, "{skill_path}");
        assert_eq!(
            package.files[0].relative_path,
            PathBuf::from("SKILL.md"),
            "{skill_path}"
        );
    }
}

#[test]
fn collects_nested_resources_binary_bytes_and_executable_state() {
    let archive = tar_gz(&[
        TestEntry::executable(
            "repo-sha/skills/find-skills/scripts/run.sh",
            b"#!/bin/sh\nexit 0\n".to_vec(),
        ),
        TestEntry::file(
            "repo-sha/skills/find-skills/SKILL.md",
            skill_markdown("find-skills", "Find skills"),
        ),
        TestEntry::file(
            "repo-sha/skills/find-skills/assets/data.bin",
            vec![0, 159, 146, 150, 255],
        ),
        TestEntry::file("repo-sha/other.txt", b"outside package".to_vec()),
    ]);

    let package = parse_skill_package(&archive, skill_name(), "abc123".to_string())
        .expect("complete package should parse");

    assert_eq!(
        package
            .files
            .iter()
            .map(|file| file.relative_path.clone())
            .collect::<Vec<_>>(),
        [
            PathBuf::from("SKILL.md"),
            PathBuf::from("assets/data.bin"),
            PathBuf::from("scripts/run.sh"),
        ]
    );
    assert_eq!(package.files[1].bytes, vec![0, 159, 146, 150, 255]);
    assert!(!package.files[1].executable);
    assert!(package.files[2].executable);
}

#[test]
fn rejects_missing_frontmatter() {
    let archive = tar_gz(&[TestEntry::file(
        "repo-sha/skills/find-skills/SKILL.md",
        b"# Find skills\n".to_vec(),
    )]);

    let error = parse_skill_package(&archive, skill_name(), "abc123".to_string())
        .expect_err("missing frontmatter should fail");

    assert!(
        error
            .to_string()
            .contains("must begin with YAML frontmatter")
    );
    assert!(error.to_string().contains("skills/find-skills/SKILL.md"));
}

#[test]
fn rejects_malformed_yaml_frontmatter() {
    let archive = tar_gz(&[TestEntry::file(
        "repo-sha/skills/find-skills/SKILL.md",
        b"---\nname: [unterminated\ndescription: broken\n---\n".to_vec(),
    )]);

    let error = parse_skill_package(&archive, skill_name(), "abc123".to_string())
        .expect_err("malformed YAML should fail");

    assert!(error.to_string().contains("YAML frontmatter"));
    assert!(error.to_string().contains("skills/find-skills/SKILL.md"));
}

#[test]
fn rejects_empty_description() {
    let archive = tar_gz(&[TestEntry::file(
        "repo-sha/skills/find-skills/SKILL.md",
        skill_markdown("find-skills", "\"   \""),
    )]);

    let error = parse_skill_package(&archive, skill_name(), "abc123".to_string())
        .expect_err("empty description should fail");

    assert!(error.to_string().contains("non-empty description"));
    assert!(error.to_string().contains("skills/find-skills/SKILL.md"));
}

#[test]
fn reports_not_found_for_mismatched_name() {
    let archive = tar_gz(&[TestEntry::file(
        "repo-sha/skills/other/SKILL.md",
        skill_markdown("other", "Another skill"),
    )]);

    let error = parse_skill_package(&archive, skill_name(), "abc123".to_string())
        .expect_err("mismatched name should not be selected");

    assert!(error.to_string().contains("find-skills"));
    assert!(error.to_string().contains("not found"));
}

#[test]
fn reports_sorted_paths_for_duplicate_matching_names() {
    let archive = tar_gz(&[
        TestEntry::file(
            "repo-sha/skills/zeta/SKILL.md",
            skill_markdown("find-skills", "Duplicate zeta"),
        ),
        TestEntry::file(
            "repo-sha/skills/alpha/SKILL.md",
            skill_markdown("find-skills", "Duplicate alpha"),
        ),
    ]);

    let error = parse_skill_package(&archive, skill_name(), "abc123".to_string())
        .expect_err("duplicate names should be ambiguous")
        .to_string();
    let alpha = error
        .find("skills/alpha/SKILL.md")
        .expect("alpha path should be reported");
    let zeta = error
        .find("skills/zeta/SKILL.md")
        .expect("zeta path should be reported");

    assert!(error.contains("ambiguous"));
    assert!(alpha < zeta, "paths should be sorted: {error}");
}

#[test]
fn requires_one_common_top_level_directory() {
    let archive = tar_gz(&[
        TestEntry::file(
            "repo-one/skills/find-skills/SKILL.md",
            skill_markdown("find-skills", "Find skills"),
        ),
        TestEntry::file("repo-two/README.md", b"other root".to_vec()),
    ]);

    let error = parse_skill_package(&archive, skill_name(), "abc123".to_string())
        .expect_err("multiple top-level directories should fail");

    assert!(error.to_string().contains("top-level"));
    assert!(error.to_string().contains("repo-two/README.md"));
}

#[test]
fn rejects_absolute_dot_parent_empty_and_backslash_path_components() {
    for raw_path in [
        b"/repo/skill/file".as_slice(),
        b"repo/./skill/file".as_slice(),
        b"repo/skill/../file".as_slice(),
        b"repo//skill/file".as_slice(),
        b"repo/skill\\file".as_slice(),
    ] {
        let archive = raw_tar_gz(vec![(raw_header(raw_path, b'0', 1), b"x".to_vec())]);

        let error = parse_skill_package(&archive, skill_name(), "abc123".to_string())
            .expect_err("unsafe raw path should fail");
        let message = format!("{error:?}");
        let displayed_path = String::from_utf8_lossy(raw_path);

        assert!(message.contains(displayed_path.as_ref()), "{message}");
    }
}

#[test]
fn rejects_every_special_entry_inside_selected_package() {
    for entry_type in *b"123467SZ" {
        let skill = skill_markdown("find-skills", "Find skills");
        let special_path = b"repo-sha/skills/find-skills/special";
        let archive = raw_tar_gz(vec![
            (
                raw_header(
                    b"repo-sha/skills/find-skills/SKILL.md",
                    b'0',
                    skill.len() as u64,
                ),
                skill,
            ),
            (raw_header(special_path, entry_type, 0), Vec::new()),
        ]);

        let error = parse_skill_package(&archive, skill_name(), "abc123".to_string())
            .expect_err("special entry type should fail");
        let message = format!("{error:?}");

        assert!(
            message.contains("skills/find-skills/special"),
            "entry type {entry_type:?}: {message}"
        );
        assert!(
            message.contains("Unsupported archive entry type"),
            "entry type {entry_type:?}: {message}"
        );
    }
}

#[test]
fn accepts_regular_files_and_directory_entries() {
    let archive = tar_gz(&[
        TestEntry::directory("repo-sha/skills/find-skills/"),
        TestEntry::file(
            "repo-sha/skills/find-skills/SKILL.md",
            skill_markdown("find-skills", "Find skills"),
        ),
        TestEntry::directory("repo-sha/skills/find-skills/assets/"),
        TestEntry::file(
            "repo-sha/skills/find-skills/assets/icon.bin",
            vec![0, 1, 255],
        ),
    ]);

    let package = parse_skill_package(&archive, skill_name(), "abc123".to_string())
        .expect("regular files and directories should be accepted");

    assert_eq!(
        package
            .files
            .iter()
            .map(|file| file.relative_path.clone())
            .collect::<Vec<_>>(),
        [PathBuf::from("SKILL.md"), PathBuf::from("assets/icon.bin"),]
    );
}

#[test]
fn rejects_duplicate_normalized_package_paths() {
    let archive = tar_gz(&[
        TestEntry::file(
            "repo-sha/skills/find-skills/SKILL.md",
            skill_markdown("find-skills", "Find skills"),
        ),
        TestEntry::file(
            "repo-sha/skills/find-skills/assets/icon.bin",
            b"first".to_vec(),
        ),
        TestEntry::file(
            "repo-sha/skills/find-skills/assets/icon.bin",
            b"second".to_vec(),
        ),
    ]);

    let error = parse_skill_package(&archive, skill_name(), "abc123".to_string())
        .expect_err("duplicate normalized path should fail");

    assert!(error.to_string().contains("Duplicate"));
    assert!(error.to_string().contains("assets/icon.bin"));
}

#[test]
fn rejects_malformed_gzip() {
    let error = parse_skill_package(b"not gzip", skill_name(), "abc123".to_string())
        .expect_err("malformed gzip should fail");

    assert!(format!("{error:?}").to_lowercase().contains("gzip"));
}

#[test]
fn rejects_malformed_tar_inside_valid_gzip() {
    let archive = gzip(&vec![b'x'; 512]);

    let error = parse_skill_package(&archive, skill_name(), "abc123".to_string())
        .expect_err("malformed tar should fail");

    assert!(format!("{error:?}").to_lowercase().contains("tar"));
}

#[test]
fn enforces_skill_markdown_discovery_limit() {
    let limit = 16;
    let archive = tar_gz(&[TestEntry::file(
        "repo-sha/skills/find-skills/SKILL.md",
        skill_markdown("find-skills", "Find skills"),
    )]);
    let mut limits = generous_limits();
    limits.skill_markdown_bytes = limit;

    let error =
        parse_skill_package_with_limits(&archive, skill_name(), "abc123".to_string(), limits)
            .expect_err("oversized SKILL.md should fail");

    assert_limit_error(&error, "skills/find-skills/SKILL.md", limit);
}

#[test]
fn enforces_archive_entry_count_limit() {
    let limit = 1;
    let archive = tar_gz(&[
        TestEntry::file(
            "repo-sha/skills/find-skills/SKILL.md",
            skill_markdown("find-skills", "Find skills"),
        ),
        TestEntry::file("repo-sha/README.md", b"readme".to_vec()),
    ]);
    let mut limits = generous_limits();
    limits.archive_entries = limit;

    let error =
        parse_skill_package_with_limits(&archive, skill_name(), "abc123".to_string(), limits)
            .expect_err("too many archive entries should fail");

    assert_limit_error(&error, "README.md", limit);
}

#[test]
fn enforces_repository_declared_regular_bytes_limit_even_outside_package() {
    let skill = skill_markdown("find-skills", "Find skills");
    let limit = skill.len() as u64 + 2;
    let archive = tar_gz(&[
        TestEntry::file("repo-sha/skills/find-skills/SKILL.md", skill),
        TestEntry::file("repo-sha/outside.bin", vec![1, 2, 3]),
    ]);
    let mut limits = generous_limits();
    limits.declared_regular_bytes = limit;

    let error =
        parse_skill_package_with_limits(&archive, skill_name(), "abc123".to_string(), limits)
            .expect_err("repository declared bytes should include outside files");

    assert_limit_error(&error, "outside.bin", limit);
}

#[test]
fn enforces_selected_file_size_limit() {
    let skill = skill_markdown("find-skills", "Find skills");
    let limit = skill.len() as u64;
    let archive = tar_gz(&[
        TestEntry::file("repo-sha/skills/find-skills/SKILL.md", skill),
        TestEntry::file(
            "repo-sha/skills/find-skills/assets/large.bin",
            vec![0; limit as usize + 1],
        ),
    ]);
    let mut limits = generous_limits();
    limits.selected_file_bytes = limit;

    let error =
        parse_skill_package_with_limits(&archive, skill_name(), "abc123".to_string(), limits)
            .expect_err("oversized selected file should fail");

    assert_limit_error(&error, "skills/find-skills/assets/large.bin", limit);
}

#[test]
fn enforces_selected_package_total_bytes_limit() {
    let skill = skill_markdown("find-skills", "Find skills");
    let limit = skill.len() as u64 + 2;
    let archive = tar_gz(&[
        TestEntry::file("repo-sha/skills/find-skills/SKILL.md", skill),
        TestEntry::file("repo-sha/skills/find-skills/assets/data.bin", vec![1, 2, 3]),
    ]);
    let mut limits = generous_limits();
    limits.selected_package_bytes = limit;

    let error =
        parse_skill_package_with_limits(&archive, skill_name(), "abc123".to_string(), limits)
            .expect_err("oversized selected package should fail");

    assert_limit_error(&error, "skills/find-skills/assets/data.bin", limit);
}

#[test]
fn enforces_selected_package_file_count_limit() {
    let limit = 1;
    let archive = tar_gz(&[
        TestEntry::file(
            "repo-sha/skills/find-skills/SKILL.md",
            skill_markdown("find-skills", "Find skills"),
        ),
        TestEntry::file("repo-sha/skills/find-skills/assets/data.bin", vec![1, 2, 3]),
    ]);
    let mut limits = generous_limits();
    limits.selected_package_files = limit;

    let error =
        parse_skill_package_with_limits(&archive, skill_name(), "abc123".to_string(), limits)
            .expect_err("too many selected files should fail");

    assert_limit_error(&error, "skills/find-skills/assets/data.bin", limit);
}

#[test]
fn checked_total_rejects_integer_overflow() {
    let limit = u64::MAX;
    let path = std::path::Path::new("skills/find-skills/assets/data.bin");

    let error = checked_total(u64::MAX, 1, limit, "Selected package bytes", path)
        .expect_err("integer overflow should fail");

    assert_limit_error(&error, "skills/find-skills/assets/data.bin", limit);
    assert!(format!("{error:?}").contains("overflow"));
}
