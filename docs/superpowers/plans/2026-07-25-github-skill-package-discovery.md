# GitHub Skill Package Discovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Discover a skill anywhere in a GitHub repository by `SKILL.md` frontmatter name, download its immutable repository archive, and atomically install the complete safe package.

**Architecture:** Replace the single UTF-8 document DTO with a package of validated relative paths and raw bytes. `agm-registry` resolves the default branch to a commit SHA, buffers a size-bounded Octocrab tarball response in memory, and parses that immutable buffer twice on a blocking worker: once to discover the requested skill and once to collect its files. `agm-skills` stages the complete package beside its destination, then swaps directories with backup-and-rollback behavior.

**Tech Stack:** Rust 2024, Tokio, Octocrab, `http-body-util`, `flate2` with its default pure-Rust backend, `tar` 0.4.46, Serde, `yaml_serde`, and `tempfile`.

## Global Constraints

- Continue using GitHub APIs through Octocrab; never invoke a system `git` executable.
- Parse archives with the Rust `flate2` and `tar` crates; never invoke a system `tar` executable and never call a general archive `unpack` operation.
- Resolve the repository default branch to an immutable commit SHA before requesting the tarball.
- Keep the compressed `.tar.gz` in memory, enforce the 256 MiB compressed limit while streaming body frames, and perform both local parser passes over that same buffer.
- Run gzip/tar parsing inside `tokio::task::spawn_blocking`; do not block Tokio worker threads with archive parsing.
- Fixed limits are: 256 MiB compressed archive, 64 MiB per selected file, 256 MiB selected package bytes, 10,000 selected files, 1 MiB per inspected `SKILL.md`, 100,000 archive entries, and 1 GiB total declared regular-file bytes.
- Use checked arithmetic for every count and byte total. Every limit error must name the affected normalized repository path and the applicable limit.
- Accept only relative archive paths made from normal, non-empty components, with no `.`, `..`, absolute prefix, or backslash.
- Inside the selected package, accept only regular files and directories. Reject symlinks, hard links, devices, FIFOs, contiguous files, sparse entries, and unknown special entry types.
- Do not preserve or expand symlinks, hard links, submodules, devices, special entries, or Git LFS pointers.
- Do not add branch/tag/commit CLI selection or configurable package limits.
- Preserve the CLI shape `agm skill add github:vercel-labs/skills --skill find-skills`.
- Keep the existing outer error context in the form `Failed to download skill {name} from {owner}/{repo}`.
- Use current package-manager-selected releases. Pin `tar` to 0.4.46 because 0.4.45 is affected by the 2026 PAX-header desynchronization advisory; do not introduce `serde_yaml` or `serde_yml`.
- Preserve all pre-existing uncommitted work. In particular, do not replace or revert edits in `crates/agm-registry/src/github.rs`, `crates/agm-cli/tests/cli.rs`, `mise.toml`, `.agents/skills/find-skills/SKILL.md`, or the approved design document.
- Do not edit `mise.toml`, `.agents/skills/find-skills/SKILL.md`, or `docs/superpowers/specs/2026-07-25-github-skill-package-discovery-design.md` as part of implementation.
- Do not create commits as part of this plan.

For a Python-oriented reader, `SkillPackage` and `SkillFile` are the Rust equivalent of validated `@dataclass` values, while `Result` propagation replaces exception-driven control flow at each trust boundary.

---

## File Map

- Modify `Cargo.toml`: centralize the Cargo-selected dependency versions in `[workspace.dependencies]` and remove the now-unused `base64` entry.
- Modify `Cargo.lock`: let Cargo regenerate the lockfile after dependency additions/removal.
- Modify `crates/agm-core/src/skills.rs:1-89`: replace `SkillContent` with `SkillPackage`/`SkillFile` and add package/path validation.
- Modify `crates/agm-core/src/registry.rs:1-102,137-278`: migrate the `Registry` trait and its test double to `SkillPackage`.
- Modify `crates/agm-registry/Cargo.toml:6-13`: add inherited archive, body-streaming, Serde, and YAML dependencies; remove `base64`.
- Modify `crates/agm-registry/src/github.rs:1-344`: replace Contents API retrieval with immutable tarball retrieval, bounded in-memory response streaming, blocking parser dispatch, and focused transport tests.
- Create `crates/agm-registry/src/github/archive.rs`: own two-pass discovery, frontmatter parsing, path normalization, package construction, limits, and synthetic archive tests.
- Modify `crates/agm-skills/Cargo.toml:6-11`: add inherited `tempfile`.
- Modify `crates/agm-skills/src/lib.rs:1-114`: stage and atomically replace complete packages, preserve binaries and executable state, and test rollback/cleanup.
- Modify `crates/agm-cli/tests/cli.rs:153-160`: append a documented-command-shape regression without disturbing the existing uncommitted validation tests.
- No production changes are expected in `crates/agm-cli/src/app.rs`, `crates/agm-cli/src/args.rs`, `crates/agm-harness/src/lib.rs`, or `crates/agm-registry/src/lib.rs`.

### Task 1: Migrate the Domain Model and Registry Trait

**Files:**

- Modify: `crates/agm-core/src/skills.rs:1-89`
- Modify: `crates/agm-core/src/registry.rs:1-102,137-278`
- Modify: `crates/agm-registry/src/github.rs:1-150,318-342`
- Modify: `crates/agm-skills/src/lib.rs:1-73,75-114`

**Interfaces:**

- Produces: `SkillFile { relative_path: PathBuf, bytes: Vec<u8>, executable: bool }`
- Produces: `SkillPackage { name: SkillName, revision: String, files: Vec<SkillFile> }`
- Produces: `pub fn validate_skill_relative_path(path: &Path) -> Result<()>`
- Produces: `pub fn SkillPackage::validate(&self) -> Result<()>`
- Produces: `Registry::fetch_skill(&self, skill_name: &SkillName) -> Result<SkillPackage>`
- Preserves: `add_skill<R: Registry>(registry: &R, skill_name: &SkillName) -> Result<()>`

- [ ] **Step 1: Add failing package-invariant and trait tests**

Replace the `SkillContent`-based test double in `crates/agm-core/src/registry.rs` and add focused tests in `crates/agm-core/src/skills.rs`:

```rust
fn package_with_paths(paths: &[&str]) -> SkillPackage {
    SkillPackage {
        name: "test-skill".parse().expect("valid skill name"),
        revision: "abc123".to_string(),
        files: paths
            .iter()
            .map(|path| SkillFile {
                relative_path: PathBuf::from(path),
                bytes: b"content".to_vec(),
                executable: false,
            })
            .collect(),
    }
}

#[test]
fn skill_package_requires_exactly_one_root_skill_file() {
    assert!(package_with_paths(&[]).validate().is_err());
    assert!(package_with_paths(&["nested/SKILL.md"]).validate().is_err());
    assert!(package_with_paths(&["SKILL.md"]).validate().is_ok());
    assert!(package_with_paths(&["SKILL.md", "SKILL.md"]).validate().is_err());
}

#[test]
fn skill_package_rejects_unsafe_and_duplicate_file_paths() {
    for path in ["", "../escape", "/absolute", r"assets\escape"] {
        assert!(
            package_with_paths(&["SKILL.md", path]).validate().is_err(),
            "{path:?} should be rejected"
        );
    }
    assert!(
        package_with_paths(&["SKILL.md", "assets/icon.png", "assets/icon.png"])
            .validate()
            .is_err()
    );
}
```

The registry test double must return:

```rust
async fn fetch_skill(&self, skill_name: &SkillName) -> Result<SkillPackage> {
    Ok(SkillPackage {
        name: skill_name.clone(),
        revision: "abc123".to_string(),
        files: vec![SkillFile {
            relative_path: PathBuf::from("SKILL.md"),
            bytes: b"# Test Skill\n".to_vec(),
            executable: false,
        }],
    })
}
```

- [ ] **Step 2: Run the focused tests and confirm the red state**

Run: `cargo test -p agm-core skill_package -- --nocapture`

Expected: compilation fails because `SkillPackage`, `SkillFile`, and `SkillPackage::validate` do not exist.

- [ ] **Step 3: Replace `SkillContent` and implement domain validation**

In `crates/agm-core/src/skills.rs`, replace `SkillContent` with:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillPackage {
    pub name: SkillName,
    pub revision: String,
    pub files: Vec<SkillFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillFile {
    pub relative_path: PathBuf,
    pub bytes: Vec<u8>,
    pub executable: bool,
}
```

Implement `validate_skill_relative_path` with these exact checks:

1. Reject an empty `Path`.
2. Reject `path.is_absolute()`.
3. Require every `path.components()` item to be `Component::Normal`.
4. Reject a normal component containing a literal backslash.

Implement `SkillPackage::validate` by:

1. Revalidating every `SkillFile::relative_path`.
2. Inserting each relative path into a `BTreeSet<PathBuf>` and rejecting duplicates.
3. Counting paths exactly equal to `Path::new("SKILL.md")`.
4. Requiring that count to equal one.

Use errors that include the offending path, for example:

```rust
bail!(
    "Unsafe skill package path `{}`: paths must be relative normal components",
    path.display()
);
```

- [ ] **Step 4: Mechanically migrate every current caller without changing behavior yet**

In `crates/agm-core/src/registry.rs`, change the trait return type and imports to `SkillPackage`.

In `crates/agm-registry/src/github.rs`, temporarily keep the current Contents API flow, rename `build_skill_content` to `build_skill_package`, validate that the downloaded `SKILL.md` is UTF-8, and return:

```rust
Ok(SkillPackage {
    name,
    revision: item.sha,
    files: vec![SkillFile {
        relative_path: PathBuf::from("SKILL.md"),
        bytes,
        executable: false,
    }],
})
```

In `crates/agm-skills/src/lib.rs`, change `install_to_harness` and `auto_install_skill` to accept `&SkillPackage`. For this migration task only, locate the unique root `SKILL.md` after `skill.validate()?` and write its raw bytes:

```rust
let skill_file = skill
    .files
    .iter()
    .find(|file| file.relative_path == Path::new(SKILL_FILE_NAME))
    .context("Validated package did not contain root SKILL.md")?;
tokio::fs::write(&destination_file, &skill_file.bytes).await?;
```

Update all unit fixtures to construct `SkillPackage` and `SkillFile`; remove every `sha`, `encoding`, `size`, and `content` field access.

- [ ] **Step 5: Verify the migration is green before archive work**

Run: `cargo test --workspace`

Expected: exit 0; all existing tests pass with `SkillPackage`, and `rg -n 'SkillContent' crates` returns no matches.

### Task 2: Add Secure Dependencies and Implement Arbitrary-Path Discovery

**Files:**

- Modify: `Cargo.toml:16-35`
- Modify: `Cargo.lock`
- Modify: `crates/agm-registry/Cargo.toml:6-13`
- Modify: `crates/agm-skills/Cargo.toml:6-11`
- Modify: `crates/agm-registry/src/github.rs:1-8`
- Create: `crates/agm-registry/src/github/archive.rs`

**Interfaces:**

- Consumes: `SkillName`, `SkillPackage`, `SkillFile`, and `SkillPackage::validate`
- Produces: `pub(super) fn parse_skill_package(archive_bytes: &[u8], requested_name: SkillName, revision: String) -> Result<SkillPackage>`
- Produces: `fn discover_skill_root(archive_bytes: &[u8], requested_name: &SkillName) -> Result<PathBuf>`
- Produces: `fn collect_skill_files(archive_bytes: &[u8], skill_root: &Path) -> Result<Vec<SkillFile>>`

- [ ] **Step 1: Add current dependencies through Cargo, then preserve workspace inheritance**

Run these package-manager commands from `/home/mohamed/Developments/agm`:

```text
cargo add --package agm-registry flate2@1.1.9 http-body-util@0.1.4 tar@0.4.46 yaml_serde@0.10.4
cargo add --package agm-registry serde
cargo add --package agm-skills tempfile@3.27.0
```

Expected: Cargo updates package manifests and `Cargo.lock`; `flate2` reports the default `miniz_oxide`/`rust_backend` features.

Move the Cargo-selected requirements into the existing workspace dependency pattern so the final manifests contain:

```toml
# Cargo.toml [workspace.dependencies]
flate2 = "1.1.9"
http-body-util = "0.1.4"
tar = "0.4.46"
tempfile = "3.27.0"
yaml_serde = "0.10.4"
```

```toml
# crates/agm-registry/Cargo.toml [dependencies]
flate2 = { workspace = true }
http-body-util = { workspace = true }
serde = { workspace = true }
tar = { workspace = true }
yaml_serde = { workspace = true }
```

```toml
# crates/agm-skills/Cargo.toml [dependencies]
tempfile = { workspace = true }
```

Keep `serde = { version = "1.0", features = ["derive"] }` in the workspace manifest. Do not add `serde_yaml` or `serde_yml`.

Run: `cargo tree -p agm-registry -i tar`

Expected: the first line is `tar v0.4.46`; no `tar` version at or below 0.4.45 appears.

- [ ] **Step 2: Add synthetic archive helpers and failing discovery tests**

Declare `mod archive;` in `crates/agm-registry/src/github.rs`. In the new module's test section, use an owned fixture type so binary bytes and generated paths are safe:

```rust
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
        header.set_path(&entry.path).expect("fixture path should fit");
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
```

Add these exact tests:

- `discovers_root_skill_and_returns_complete_sorted_package`
- `discovers_flat_categorized_hidden_and_monorepo_layouts`
- `collects_nested_resources_binary_bytes_and_executable_state`
- `rejects_missing_frontmatter`
- `rejects_malformed_yaml_frontmatter`
- `rejects_empty_description`
- `reports_not_found_for_mismatched_name`
- `reports_sorted_paths_for_duplicate_matching_names`
- `requires_one_common_github_top_level_directory`

The layout test must iterate over:

```rust
[
    "repo-sha/SKILL.md",
    "repo-sha/skills/find-skills/SKILL.md",
    "repo-sha/skills/category/find-skills/SKILL.md",
    "repo-sha/skills/.curated/find-skills/SKILL.md",
    "repo-sha/plugins/catalog/skills/find-skills/SKILL.md",
]
```

The duplicate test must assert that both normalized paths appear in lexical order, independent of insertion order.

- [ ] **Step 3: Run discovery tests and confirm the red state**

Run: `cargo test -p agm-registry github::archive::tests::discovers -- --nocapture`

Expected: compilation fails because `parse_skill_package` and its discovery helpers do not exist.

- [ ] **Step 4: Implement strict YAML frontmatter parsing**

Deserialize only the required typed fields:

```rust
#[derive(Debug, serde::Deserialize)]
struct SkillFrontmatter {
    name: String,
    description: String,
}
```

Implement:

```rust
fn frontmatter_yaml<'a>(text: &'a str, repository_path: &Path) -> Result<&'a str> {
    let mut lines = text.split_inclusive('\n');
    let opening = lines
        .next()
        .with_context(|| format!("Empty SKILL.md at `{}`", repository_path.display()))?;
    if opening.trim_end_matches(['\r', '\n']) != "---" {
        bail!(
            "SKILL.md at `{}` must begin with YAML frontmatter",
            repository_path.display()
        );
    }

    let start = opening.len();
    let mut end = start;
    for line in lines {
        if line.trim_end_matches(['\r', '\n']) == "---" {
            return Ok(&text[start..end]);
        }
        end = end
            .checked_add(line.len())
            .context("Frontmatter offset overflow")?;
    }

    bail!(
        "SKILL.md at `{}` has no closing `---` delimiter",
        repository_path.display()
    );
}
```

For every regular file with basename exactly `SKILL.md`:

1. Reject a declared size above the configured discovery limit before reading.
2. Read at most `limit + 1` bytes and reject an observed overrun.
3. Require UTF-8.
4. Parse `SkillFrontmatter` with `yaml_serde::from_str`.
5. Reject `description.trim().is_empty()`.
6. Compare `frontmatter.name` exactly and case-sensitively with `requested_name.as_str()`.
7. Treat malformed or incomplete frontmatter in any inspected `SKILL.md` as an error, not as a non-match.

- [ ] **Step 5: Implement the two-pass happy path**

Use these production signatures:

```rust
pub(super) fn parse_skill_package(
    archive_bytes: &[u8],
    requested_name: SkillName,
    revision: String,
) -> Result<SkillPackage> {
    let skill_root = discover_skill_root(archive_bytes, &requested_name)?;
    let files = collect_skill_files(archive_bytes, &skill_root)?;
    let package = SkillPackage {
        name: requested_name,
        revision,
        files,
    };
    package.validate()?;
    Ok(package)
}
```

For each pass, create a fresh reader:

```rust
let decoder = flate2::read::GzDecoder::new(std::io::Cursor::new(archive_bytes));
let mut archive = tar::Archive::new(decoder);
let entries = archive.entries().context("Malformed tar archive")?;
```

Path handling in both passes must:

1. Read the effective tar path and raw path bytes.
2. Require a first normal component.
3. Record the first entry's top-level component and require the same component for every later entry.
4. Strip exactly that one top-level component.
5. Permit the stripped empty path only for the top-level directory entry.
6. Return normalized repository-relative `PathBuf` values for all other entries.

Pass 1 collects all exact-name candidates, sorts their normalized `SKILL.md` paths, returns not-found for zero, and returns an ambiguity error listing every path for more than one. The selected skill root is the parent of the one matching `SKILL.md`; an empty parent means repository root.

Pass 2 revisits the same buffer, includes entries only when `repository_path.strip_prefix(skill_root)` succeeds, ignores accepted directory entries, records regular files as `SkillFile`, computes `executable` with `header.mode()? & 0o111 != 0`, and sorts files by `relative_path`.

- [ ] **Step 6: Run the discovery/package tests**

Run: `cargo test -p agm-registry github::archive::tests -- --nocapture`

Expected: exit 0; root, flat, categorized, hidden, monorepo, binary, nested-resource, executable, malformed-frontmatter, not-found, and ambiguity tests pass.

### Task 3: Enforce Archive Safety and Every Fixed Limit

**Files:**

- Modify: `crates/agm-registry/src/github/archive.rs`

**Interfaces:**

- Extends: `parse_skill_package`, `discover_skill_root`, and `collect_skill_files`
- Produces: `ArchiveLimits::PRODUCTION`
- Produces: `fn parse_skill_package_with_limits(archive_bytes: &[u8], requested_name: SkillName, revision: String, limits: ArchiveLimits) -> Result<SkillPackage>`
- Produces: `fn validate_raw_archive_path(raw_path: &[u8], is_directory: bool, repository_path: &Path) -> Result<()>`
- Produces: `fn checked_total(current: u64, increment: u64, limit: u64, label: &str, repository_path: &Path) -> Result<u64>`

- [ ] **Step 1: Add failing raw-path, special-entry, malformed-stream, and limit tests**

Add a test-only raw header helper that writes short unsafe names directly into the GNU name field, then recalculates the checksum:

```rust
fn raw_header(path: &[u8], entry_type: u8, size: u64) -> tar::Header {
    assert!(path.len() <= 100, "fixture path must fit the GNU name field");
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
    header.set_cksum();
    header
}
```

Add these exact table-driven tests:

- `rejects_absolute_dot_parent_empty_and_backslash_path_components` using `/repo/skill/file`, `repo/./skill/file`, `repo/skill/../file`, `repo//skill/file`, and `repo/skill\file`.
- `rejects_every_special_entry_inside_selected_package` using type bytes `b'1'`, `b'2'`, `b'3'`, `b'4'`, `b'6'`, `b'7'`, `b'S'`, and `b'Z'`.
- `accepts_regular_files_and_directory_entries`.
- `rejects_duplicate_normalized_package_paths`.
- `rejects_malformed_gzip`.
- `rejects_malformed_tar_inside_valid_gzip`.
- `enforces_skill_markdown_discovery_limit`.
- `enforces_archive_entry_count_limit`.
- `enforces_repository_declared_regular_bytes_limit_even_outside_package`.
- `enforces_selected_file_size_limit`.
- `enforces_selected_package_total_bytes_limit`.
- `enforces_selected_package_file_count_limit`.
- `checked_total_rejects_integer_overflow`.

Use small per-test `ArchiveLimits` values through `parse_skill_package_with_limits` rather than constructing multi-gigabyte fixtures. Each error assertion must include both the normalized repository path and the configured numeric limit.

- [ ] **Step 2: Run the hardening tests and confirm the red state**

Run: `cargo test -p agm-registry github::archive::tests -- --nocapture`

Expected: compilation fails because `ArchiveLimits` and `parse_skill_package_with_limits` do not exist yet.

- [ ] **Step 3: Add fixed production limits with test-only override values**

Define:

```rust
#[derive(Debug, Clone, Copy)]
struct ArchiveLimits {
    skill_markdown_bytes: u64,
    selected_file_bytes: u64,
    selected_package_bytes: u64,
    selected_package_files: usize,
    archive_entries: usize,
    declared_regular_bytes: u64,
}

impl ArchiveLimits {
    const PRODUCTION: Self = Self {
        skill_markdown_bytes: 1024 * 1024,
        selected_file_bytes: 64 * 1024 * 1024,
        selected_package_bytes: 256 * 1024 * 1024,
        selected_package_files: 10_000,
        archive_entries: 100_000,
        declared_regular_bytes: 1024 * 1024 * 1024,
    };
}
```

Keep this type private. Tests may construct smaller values directly; no CLI or configuration surface may expose it.

Change `discover_skill_root` and `collect_skill_files` to accept an `ArchiveLimits` argument. Make `parse_skill_package` delegate to `parse_skill_package_with_limits` with `ArchiveLimits::PRODUCTION`; tests call the private limited form directly.

- [ ] **Step 4: Validate raw and normalized paths before any selection**

`validate_raw_archive_path` must:

1. Reject empty input.
2. Reject a leading `/`.
3. Reject any `\` byte.
4. Split on `/` and reject empty, `.` and `..` components.
5. Allow one final empty component only for a directory entry whose preceding components are valid.
6. Include the lossy archive path in every error.

After converting to `PathBuf` and stripping the common top-level component, call `validate_skill_relative_path` for every non-empty repository path. This second check is defense in depth for platform-specific `Path` semantics.

- [ ] **Step 5: Apply checked repository-wide counters in both passes**

For each yielded tar entry:

1. Increment the entry count with `checked_add(1)` and enforce 100,000.
2. If `entry.header().entry_type().is_file()`, read the effective declared size, add it with `checked_total`, and enforce 1 GiB even when the file is outside the selected package.
3. Perform these checks before reading a file body.

Implement the shared arithmetic helper:

```rust
fn checked_total(
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
```

- [ ] **Step 6: Enforce selected-package types, counts, sizes, and exact root file**

Within the selected root:

1. Accept directories without adding them to `SkillPackage::files`.
2. Reject every entry type for which neither `is_file()` nor `is_dir()` is true.
3. Increment selected regular-file count with checked arithmetic before reading and enforce 10,000.
4. Enforce 64 MiB against the effective declared size before reading.
5. Add declared size to selected-package bytes with checked arithmetic and enforce 256 MiB.
6. Read no more than declared size plus one byte, reject mismatch/overrun, and retain raw bytes unchanged.
7. Reject duplicate relative paths before pushing.
8. Require exactly one relative path equal to `SKILL.md`.
9. Sort by relative path before returning.

Malformed gzip initialization, gzip reads, tar iteration, tar headers, paths, modes, and body reads must each have distinct context strings.

- [ ] **Step 7: Run all parser tests**

Run: `cargo test -p agm-registry github::archive::tests -- --nocapture`

Expected: exit 0; every layout, frontmatter, binary, executable, path, special-entry, malformed-stream, duplicate, count, and byte-limit test passes.

### Task 4: Retrieve and Stream the Immutable Octocrab Tarball

**Files:**

- Modify: `Cargo.toml:16-35`
- Modify: `Cargo.lock`
- Modify: `crates/agm-registry/Cargo.toml:6-18`
- Modify: `crates/agm-registry/src/github.rs:1-344`

**Interfaces:**

- Consumes: `archive::parse_skill_package`
- Produces: `async fn GitHubRegistry::resolve_default_revision(&self, client: &octocrab::Octocrab) -> Result<String>`
- Produces: `async fn GitHubRegistry::download_archive(&self, client: &octocrab::Octocrab, revision: &str) -> Result<Vec<u8>>`
- Produces: `fn checked_archive_length(current: usize, increment: usize, limit: usize) -> Result<usize>`
- Produces: `fn append_archive_chunk(buffer: &mut Vec<u8>, chunk: &[u8], limit: usize) -> Result<()>`
- Returns: `Registry::fetch_skill(...) -> Result<SkillPackage>` populated with the resolved commit SHA in `revision`

- [ ] **Step 1: Replace Contents API tests with failing bounded-buffer tests**

Delete tests for `Content`, base64 decoding, Contents API path matching, response encoding, and single-file metadata. Add:

```rust
#[test]
fn append_archive_chunk_accepts_exact_limit() {
    let mut buffer = b"abc".to_vec();
    append_archive_chunk(&mut buffer, b"de", 5).expect("exact limit should pass");
    assert_eq!(buffer, b"abcde");
}

#[test]
fn append_archive_chunk_rejects_limit_without_appending() {
    let mut buffer = b"abc".to_vec();
    let error = append_archive_chunk(&mut buffer, b"def", 5)
        .expect_err("oversized archive should fail");
    assert!(error.to_string().contains("5 byte compressed archive limit"));
    assert_eq!(buffer, b"abc");
}

#[test]
fn append_archive_chunk_rejects_length_overflow() {
    let error = checked_archive_length(usize::MAX, 1, usize::MAX)
        .expect_err("length overflow should fail");
    assert!(error.to_string().contains("overflow"));
}
```

- [ ] **Step 2: Run the transport tests and confirm the red state**

Run: `cargo test -p agm-registry append_archive_chunk -- --nocapture`

Expected: compilation fails because `append_archive_chunk` and `checked_archive_length` do not exist.

- [ ] **Step 3: Resolve repository metadata and the default branch ref**

Use Octocrab's typed APIs:

```rust
use octocrab::models::repos::Object;
use octocrab::params::repos::Reference;

async fn resolve_default_revision(&self, client: &octocrab::Octocrab) -> Result<String> {
    let repository = client
        .repos(self.owner.as_str(), self.repo.as_str())
        .get()
        .await
        .context("Failed to fetch repository metadata")?;
    let default_branch = repository
        .default_branch
        .context("GitHub repository metadata did not include a default branch")?;
    let branch_ref = client
        .repos(self.owner.as_str(), self.repo.as_str())
        .get_ref(&Reference::Branch(default_branch.clone()))
        .await
        .with_context(|| format!("Failed to resolve default branch `{default_branch}`"))?;

    match branch_ref.object {
        Object::Commit { sha, .. } => Ok(sha),
        Object::Tag { .. } => bail!("Default branch `{default_branch}` resolved to a tag"),
        _ => bail!("Default branch `{default_branch}` resolved to an unsupported object"),
    }
}
```

This metadata request must occur once per `fetch_skill`; the SHA returned here is both the tarball reference and `SkillPackage::revision`.

- [ ] **Step 4: Stream response frames into one bounded in-memory buffer**

Define:

```rust
const MAX_COMPRESSED_ARCHIVE_BYTES: usize = 256 * 1024 * 1024;
```

Use `http_body_util::BodyExt` and Octocrab's `download_tarball` response:

```rust
async fn download_archive(
    &self,
    client: &octocrab::Octocrab,
    revision: &str,
) -> Result<Vec<u8>> {
    let response = client
        .repos(self.owner.as_str(), self.repo.as_str())
        .download_tarball(revision.to_owned())
        .await
        .with_context(|| format!("Failed to request repository tarball at `{revision}`"))?;
    let status = response.status();
    if !status.is_success() {
        bail!("GitHub tarball request returned HTTP {status}");
    }

    let mut body = response.into_body();
    let mut archive_bytes = Vec::new();
    while let Some(frame) = body.frame().await {
        let frame = frame.context("Failed while streaming repository tarball")?;
        if let Ok(data) = frame.into_data() {
            append_archive_chunk(
                &mut archive_bytes,
                data.as_ref(),
                MAX_COMPRESSED_ARCHIVE_BYTES,
            )?;
        }
    }
    Ok(archive_bytes)
}
```

`checked_archive_length` must call `checked_add`, report overflow, and reject totals above `limit` with an error containing the numeric limit. `append_archive_chunk` calls it before `extend_from_slice` and leaves the existing buffer unchanged on failure. The production call uses `MAX_COMPRESSED_ARCHIVE_BYTES`, so its error identifies the 268,435,456-byte (256 MiB) limit.

- [ ] **Step 5: Dispatch both parser passes to one blocking task**

Replace the Contents API body of `Registry::fetch_skill` with:

```rust
async fn fetch_skill(&self, name: &SkillName) -> Result<SkillPackage> {
    let github = octocrab::instance();

    async {
        let revision = self.resolve_default_revision(&github).await?;
        let archive_bytes = self.download_archive(&github, &revision).await?;
        let requested_name = name.clone();
        tokio::task::spawn_blocking(move || {
            archive::parse_skill_package(&archive_bytes, requested_name, revision)
        })
        .await
        .context("Archive parsing task failed")?
    }
    .await
    .context(format!(
        "Failed to download skill `{}` from {}/{}",
        name, self.owner, self.repo
    ))
}
```

Remove the old Contents API helpers, `octocrab::models::repos::Content`, and all base64 imports.

- [ ] **Step 6: Remove the obsolete base64 dependency and verify transport tests**

Run: `cargo remove --package agm-registry base64`

After `rg -n 'base64' crates` returns no source usage, remove `base64 = "0.22"` from the root `[workspace.dependencies]`; Cargo will drop it from the lockfile if no transitive dependency needs that version.

Run: `cargo test -p agm-registry -- --nocapture`

Expected: exit 0; bounded-buffer and complete archive-parser tests pass.

### Task 5: Install the Full Package with Atomic Replacement and Rollback

**Files:**

- Modify: `crates/agm-skills/src/lib.rs:1-114`

**Interfaces:**

- Consumes: `SkillPackage::validate`, `SkillFile`, and `Harness::project_skills_dir`
- Preserves: `pub async fn install_to_harness(harness: &Harness, skill: &SkillPackage) -> Result<()>`
- Preserves: `pub async fn auto_install_skill(skill: &SkillPackage) -> Result<()>`
- Produces: `async fn replace_directory(staged: &Path, destination: &Path) -> Result<()>`
- Produces on Unix: `async fn apply_executable(path: &Path, executable: bool) -> Result<()>`

- [ ] **Step 1: Replace the single-file fixture with failing package-install tests**

Use `tempfile::TempDir` instead of PID-derived global temp paths. Add a package helper:

```rust
fn package(files: Vec<(&str, Vec<u8>, bool)>) -> SkillPackage {
    SkillPackage {
        name: "test-skill".parse().expect("valid skill name"),
        revision: "abc123".to_string(),
        files: files
            .into_iter()
            .map(|(path, bytes, executable)| SkillFile {
                relative_path: PathBuf::from(path),
                bytes,
                executable,
            })
            .collect(),
    }
}
```

Add these exact tests:

- `install_writes_nested_resources_and_preserves_binary_bytes`
- `install_rejects_package_without_root_skill_file`
- `install_revalidates_every_relative_path`
- `install_removes_stale_files_when_updating`
- `failed_staging_leaves_existing_destination_unchanged`
- `replace_directory_restores_backup_when_install_rename_fails`
- `successful_and_rollback_succeeded_installs_leave_no_staging_or_backup_directories`
- `install_applies_executable_permissions_on_unix` behind `#[cfg(unix)]`

The rollback test must create an existing destination and call `replace_directory` with a nonexistent staged path. This deterministically makes the second rename fail after the old destination has moved, then asserts the old file is restored at the destination.

- [ ] **Step 2: Run installer tests and confirm the red state**

Run: `cargo test -p agm-skills install_writes_nested_resources_and_preserves_binary_bytes -- --nocapture`

Expected: FAIL because the transitional installer writes only `SKILL.md`; the nested binary resource is absent.

Run: `cargo test -p agm-skills replace_directory_restores_backup_when_install_rename_fails -- --nocapture`

Expected: compilation fails because `replace_directory` does not exist.

- [ ] **Step 3: Stage every validated file beside the destination**

`install_to_harness` must execute in this order:

1. Call `skill.validate()` before filesystem mutation.
2. Create the harness skills directory with `tokio::fs::create_dir_all`.
3. Create a `tempfile::Builder::new().prefix(".agm-staging-").tempdir_in(skills_root)` directory. Use `spawn_blocking` for this synchronous filesystem call.
4. Re-run `validate_skill_relative_path` for every file immediately before `staging.path().join(relative_path)`.
5. Create each parent directory with `tokio::fs::create_dir_all`.
6. Write `SkillFile::bytes` with `tokio::fs::write`.
7. Apply executable state on Unix.
8. Verify `staging.path().join(SKILL_FILE_NAME)` is a regular file using `tokio::fs::metadata`.
9. Call `replace_directory(staging.path(), destination)`.

Keep the `TempDir` guard alive through staging and replacement. Before a successful rename it cleans failures automatically; after a successful rename its old random path no longer exists.

- [ ] **Step 4: Preserve executable state on Unix**

Implement:

```rust
#[cfg(unix)]
async fn apply_executable(path: &Path, executable: bool) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    if !executable {
        return Ok(());
    }
    let mut permissions = tokio::fs::metadata(path)
        .await
        .with_context(|| format!("Failed to read permissions for {}", path.display()))?
        .permissions();
    permissions.set_mode(permissions.mode() | 0o111);
    tokio::fs::set_permissions(path, permissions)
        .await
        .with_context(|| format!("Failed to set executable permissions on {}", path.display()))
}

#[cfg(not(unix))]
async fn apply_executable(_path: &Path, _executable: bool) -> Result<()> {
    Ok(())
}
```

This preserves normal file permissions and only adds executable bits when the archive recorded at least one Unix execute bit.

- [ ] **Step 5: Implement same-filesystem backup, swap, and rollback**

`replace_directory` must:

1. Rename staged directly to destination when destination does not exist.
2. Otherwise create `.agm-backup-*` with `tempfile::Builder::tempdir_in(destination.parent())`, using `spawn_blocking` for this synchronous filesystem call.
3. Call `backup.keep()` to obtain a path whose cleanup is explicitly controlled.
4. Rename destination to `backup_root.join("previous")`.
5. Rename staged to destination.
6. On step 5 failure, rename `previous` back to destination.
7. If rollback succeeds, remove the now-empty backup root and return the original replacement error.
8. If rollback fails, leave the old package under `previous` and return one error containing the replacement error, rollback error, and backup path.
9. After a successful swap, remove the backup root asynchronously. If removal fails, log `tracing::warn!` with the backup path and still return success.

If step 4 fails, remove the still-empty backup root before returning the rename error. Cleanup tests cover this first-rename failure, the successful swap, staging failure, and successful rollback; a rollback failure intentionally retains the backup because it contains the previous installation.

The critical branch should have this shape:

```rust
match tokio::fs::rename(staged, destination).await {
    Ok(()) => {
        if let Err(error) = tokio::fs::remove_dir_all(&backup_root).await {
            tracing::warn!(
                path = %backup_root.display(),
                error = %error,
                "Installed skill but failed to remove backup directory"
            );
        }
        Ok(())
    }
    Err(replacement_error) => {
        match tokio::fs::rename(&previous, destination).await {
            Ok(()) => {
                if let Err(error) = tokio::fs::remove_dir_all(&backup_root).await {
                    tracing::warn!(
                        path = %backup_root.display(),
                        error = %error,
                        "Rollback succeeded but backup container cleanup failed"
                    );
                }
                Err(replacement_error).context("Failed to replace installed skill; restored previous installation")
            }
            Err(rollback_error) => bail!(
                "Failed to replace installed skill: {replacement_error}; rollback failed: \
                 {rollback_error}; previous installation remains at {}",
                previous.display()
            ),
        }
    }
}
```

All download and archive parsing already finish before `auto_install_skill`, so this task must not move destination mutation earlier in the add flow.

- [ ] **Step 6: Run all installer tests**

Run: `cargo test -p agm-skills -- --nocapture`

Expected: exit 0; nested files, binary bytes, Unix executable bits, stale removal, pre-swap failure, rollback, and staging/backup cleanup tests pass.

### Task 6: Update Regressions and Run Full Verification

**Files:**

- Modify: `crates/agm-cli/tests/cli.rs:153-160`
- Review only: `crates/agm-cli/src/app.rs`
- Review only: `crates/agm-registry/src/github.rs`
- Review only: `crates/agm-registry/src/github/archive.rs`
- Review only: `crates/agm-skills/src/lib.rs`

**Interfaces:**

- Verifies: unchanged source parsing and CLI destination selection
- Verifies: exact documented command shape without making a network request
- Verifies: no system `git`, system `tar`, general unpack, archived YAML crate, or old `SkillContent`

- [ ] **Step 1: Append a CLI compatibility regression without disturbing existing tests**

Add:

```rust
#[test]
fn documented_github_skill_add_shape_reaches_domain_validation() {
    let mut cmd = agm();
    cmd.args([
        "skill",
        "add",
        "github:vercel-labs/skills",
        "--skill",
        "../find-skills",
    ]);
    cmd.assert()
        .failure()
        .stderr(contains("Invalid skill name `../find-skills`"))
        .stderr(contains("unexpected argument").not());
}
```

The intentionally invalid skill name stops before network access while proving that the documented source and `--skill` placement still parse. Do not rewrite the existing uncommitted CLI validation tests.

- [ ] **Step 2: Run focused regressions**

Run: `cargo test -p agm --test cli`

Expected: exit 0; all CLI integration tests pass.

Run: `cargo test -p agm-core -p agm-registry -p agm-skills`

Expected: exit 0; domain, transport, archive, and installer tests all pass.

- [ ] **Step 3: Prove prohibited implementations and dependencies are absent**

Run:

```text
if rg -n 'Command::new\("(git|tar)"' crates; then exit 1; else echo "PASS: no system git/tar invocation"; fi
if rg -n '\.unpack(_in)?\(' crates/agm-registry crates/agm-skills; then exit 1; else echo "PASS: no general archive unpack"; fi
if rg -n 'serde_yaml|serde_yml' Cargo.toml crates Cargo.lock; then exit 1; else echo "PASS: maintained yaml_serde only"; fi
if rg -n 'SkillContent' crates; then exit 1; else echo "PASS: package model fully migrated"; fi
```

Expected: four `PASS:` lines and exit 0.

Run: `cargo tree -p agm-registry -i tar`

Expected: `tar v0.4.46` is the only direct tar parser version selected for `agm-registry`.

Run: `cargo tree -p agm-registry -e features -i miniz_oxide`

Expected: the tree reaches `flate2` through its default pure-Rust `miniz_oxide` backend.

- [ ] **Step 4: Run repository-required formatting, tests, and linting**

Run: `cargo fmt --check`

Expected: exit 0 with no formatting diff.

Run: `cargo test --workspace`

Expected: exit 0; every workspace unit, integration, and doc test passes with zero failures.

Run: `cargo clippy --workspace --all-targets -- -D warnings`

Expected: exit 0 with no warnings.

- [ ] **Step 5: Review the final diff for scope and preserved user work**

Run: `git status --short`

Expected: implementation files plus the pre-existing uncommitted files are visible; no commit exists.

Run:

```text
git diff --check
git diff --stat
git diff -- mise.toml .agents/skills/find-skills/SKILL.md docs/superpowers/specs/2026-07-25-github-skill-package-discovery-design.md
```

Expected: `git diff --check` exits 0; the final command shows no implementation-caused edits to the three protected paths. The approved design document may remain untracked exactly as it was before implementation.

## Self-Review Against the Approved Design

- [x] **Context and goals:** Tasks 2-5 discover root, flat, categorized, hidden, plugin/monorepo layouts and install complete binary-capable packages.
- [x] **Non-goals:** Global constraints explicitly exclude other hosts, revision-selection CLI, configurable limits, special-entry preservation, and Git LFS expansion.
- [x] **Retrieval:** Task 4 fetches metadata, resolves the default branch to a commit SHA, calls Octocrab `download_tarball` at that SHA, checks HTTP status, streams bounded frames into memory, and uses `spawn_blocking`.
- [x] **Revision consistency:** One resolved SHA is used for both tarball retrieval and `SkillPackage::revision`; parsing uses one immutable in-memory buffer.
- [x] **Pass 1 discovery:** Task 2 strips one common GitHub top-level component, inspects every regular basename-exact `SKILL.md`, requires UTF-8 and typed delimited YAML, validates non-empty description, matches exact name, and reports sorted ambiguity paths.
- [x] **Pass 2 package build:** Tasks 2-3 revisit the archive, collect every regular file under the selected parent (or repository root), retain relative path/raw bytes/executable state, and sort deterministically.
- [x] **Domain model:** Task 1 defines the approved `SkillPackage` and `SkillFile` fields, migrates `Registry::fetch_skill`, removes obsolete single-file metadata, and requires exactly one root-relative `SKILL.md`.
- [x] **Path safety:** Task 3 rejects absolute, empty, dot, parent, and backslash paths before selection, revalidates normalized paths, and ensures prefix removal cannot escape the selected root.
- [x] **Entry safety:** Task 3 accepts only directories and regular files in the package and explicitly tests hard links, symlinks, devices, FIFOs, contiguous files, sparse entries, and unknown types.
- [x] **All limits:** Tasks 3-4 cover 256 MiB compressed, 64 MiB selected file, 256 MiB package, 10,000 selected files, 1 MiB inspected `SKILL.md`, 100,000 archive entries, and 1 GiB declared repository bytes with checked arithmetic and path-bearing errors.
- [x] **No general extraction:** Global constraints and Task 6 prohibit `unpack`; only validated bytes are written by the installer.
- [x] **Installation:** Task 5 creates the harness root, securely stages beside the destination, revalidates paths, creates parents, writes binary bytes, applies Unix execute bits, verifies staged `SKILL.md`, swaps whole directories, removes stale files, backs up, rolls back, and warns without undoing success on backup cleanup failure.
- [x] **Failure guarantees:** Download/archive errors precede destination mutation; staging failures retain the old destination; replacement failure restores backup whenever possible and reports the preserved backup path otherwise.
- [x] **Dependencies:** Task 2 uses Cargo-selected current releases, keeps `flate2` defaults, pins fixed `tar` 0.4.46, adds direct `http-body-util`, uses official `yaml_serde`, adds `tempfile`, and forbids archived/deprecated YAML crates.
- [x] **Error taxonomy:** Tasks 2-5 provide distinct contexts for metadata, branch resolution, HTTP/status/streaming, gzip/tar, paths/frontmatter/ambiguity/limits, staging/permissions/replacement/rollback while retaining the existing outer context.
- [x] **Testing:** Tasks 1-6 cover every parser, installer, registry migration, and CLI regression named in the design and use exact focused/full commands with expected red or green outcomes.
- [x] **Compatibility:** Task 6 proves the documented CLI shape still parses and leaves source parsing, harness detection, and destination selection unchanged.
- [x] **No external executables and in-memory buffering:** Tasks 4 and 6 explicitly require Octocrab plus Rust archive crates, prohibit system `git`/`tar`, and keep the bounded compressed archive in RAM for both passes.
- [x] **Scope preservation:** The plan names protected uncommitted files, avoids changes to the approved design and unrelated configuration, contains no commit steps, and ends with a diff review.
- [x] **Plan quality:** All task steps use checkboxes, paths and commands are exact, red/green expectations are stated, non-obvious Rust interfaces and algorithms are concrete, type names stay consistent across tasks, and no incomplete implementation markers remain.
