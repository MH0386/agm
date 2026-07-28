# GitHub Skill Package Discovery Design

## Status

Approved in conversation on 2026-07-25.

## Context

AGM currently assumes every remote skill lives at
`skills/{name}/SKILL.md` and downloads only that file. This works for flat
collections such as `anthropics/skills`, but it does not support:

- a single skill at repository root;
- categorized collections such as `skills/{category}/{name}/SKILL.md`;
- hidden catalog groups such as `skills/.curated/{name}/SKILL.md`;
- skills nested inside plugin or monorepo directories; or
- bundled resources such as `scripts/`, `references/`, and `assets/`.

The implementation must continue using GitHub APIs through Octocrab. It must
not require a system `git` executable.

## Goals

1. Discover a requested skill by its `SKILL.md` frontmatter `name`, regardless
  of where the skill directory appears in a GitHub repository.
2. Download and install the complete skill directory, including binary files.
3. Preserve executable file mode on Unix.
4. Reject ambiguous, malformed, oversized, or unsafe packages.
5. Avoid partial installs and stale files when replacing an existing skill.



## Non-goals

- Supporting Git hosts other than GitHub.
- Selecting a branch, tag, or commit from the CLI.
- Adding configurable archive or package limits in this iteration.
- Preserving symlinks, hard links, submodules, devices, or other special
archive entries.
- Expanding Git LFS pointer files.



## Retrieval

`GitHubRegistry::fetch_skill` will:

1. Fetch repository metadata to obtain the default branch.
2. Resolve that branch to an immutable commit SHA.
3. Request the repository tarball at that SHA with Octocrab's
  `download_tarball` API.
4. Validate the HTTP response status.
5. Stream compressed body frames into a byte buffer while enforcing a
  256 MiB compressed-size limit.
6. Move archive parsing to `tokio::task::spawn_blocking`.

Using the commit SHA prevents files from different revisions being combined if
the default branch advances during installation.

## Archive Discovery

The parser performs two local passes over the buffered `.tar.gz`:

### Pass 1: locate the skill

1. Decompress the archive with `flate2` and iterate it with `tar`.
2. Require one common GitHub-generated top-level directory component and
  strip it from every repository path.
3. Inspect every regular file whose basename is exactly `SKILL.md`.
4. Require the file to be UTF-8 and begin with `---`-delimited YAML
  frontmatter containing:
  - a string `name`;
  - a non-empty string `description`.
5. Select candidates whose `name` exactly equals the requested `SkillName`.

No candidate produces a not-found error. More than one candidate produces an
ambiguity error listing each normalized repository path. AGM must never pick a
candidate based on archive order.

### Pass 2: build the package

The parser iterates the archive again and collects every regular file under
the selected `SKILL.md` parent directory. For a root-level skill, the
repository root is the package root.

Each file records:

- its path relative to the skill root;
- raw bytes;
- whether its Unix executable bits are set.

Files are sorted by relative path before returning the package so behavior and
tests are deterministic.

## Domain Model

Replace the single-file `SkillContent` representation with:

```rust
pub struct SkillPackage {
    pub name: SkillName,
    pub revision: String,
    pub files: Vec<SkillFile>,
}

pub struct SkillFile {
    pub relative_path: PathBuf,
    pub bytes: Vec<u8>,
    pub executable: bool,
}
```

`Registry::fetch_skill` returns `SkillPackage`. The commit SHA replaces the
old `sha` field; GitHub's response encoding and a single-file size are no
longer meaningful package metadata.

The package must contain exactly one root-relative `SKILL.md`.

## Archive and Path Safety

AGM will never call a general archive `unpack` operation. It reads entries and
writes only validated package files during the later installation phase.

After removing GitHub's top-level directory, accepted paths must:

- be relative;
- contain only normal, non-empty components;
- contain neither `.` nor `..`;
- contain no backslash, so a path cannot change meaning on Windows; and
- remain beneath the selected skill root after prefix removal.

Only regular files and directories are accepted within the selected package.
Symlinks, hard links, devices, FIFOs, and other special entry types produce an
error instead of being ignored.

Fixed limits for this iteration are:

- compressed repository archive: 256 MiB;
- individual selected file: 64 MiB;
- total selected package bytes: 256 MiB;
- selected package file count: 10,000; and
- individual `SKILL.md` inspected during discovery: 1 MiB;
- total archive entries inspected: 100,000; and
- total declared regular-file bytes across the repository: 1 GiB.

All arithmetic uses checked operations. Limit violations identify the relevant
repository path and limit.

## Installation

The installer will:

1. Create the harness skills directory if needed.
2. Create a secure temporary staging directory beside the destination.
3. Revalidate every relative path before joining it to the staging root.
4. Create parent directories and write every file.
5. Apply executable permissions on Unix.
6. Verify the staged package contains `SKILL.md`.
7. Replace the destination:
  - if no destination exists, rename the staged directory into place;
  - otherwise, rename the old destination into a temporary backup, rename
  the staged directory into place, and remove the backup;
  - if the second rename fails, restore the backup before returning an error.

After a successful swap, failure to remove the backup is logged as a warning
with its path. The command still succeeds and does not roll back the newly
installed package.

Staging and destination remain on the same filesystem so renames do not cross
mount boundaries. Replacing the whole directory also removes stale files from
older package versions.

## Dependencies

Add current package-manager-selected releases of:

- `flate2`, using its default pure-Rust backend;
- `tar` 0.4.45 or newer, excluding versions affected by the 2026 path and PAX
header advisories;
- `http-body-util`, already present transitively through Octocrab, for bounded
response-body streaming;
- `yaml_serde`, the maintained official YAML organization fork, for typed
frontmatter deserialization; and
- `tempfile`, for secure staging and backup directories.

The archived `serde_yaml` crate and deprecated `serde_yml` crate must not be
introduced.

## Error Handling

Errors retain the existing outer context:

`Failed to download skill <name> from <owner>/<repo>`

Inner errors distinguish:

- repository metadata, branch resolution, HTTP, and streaming failures;
- compressed or extracted size-limit violations;
- malformed gzip or tar data;
- unsafe archive entries;
- invalid or incomplete frontmatter;
- no matching skill;
- duplicate matching skills; and
- staging, permission, replacement, or rollback failures.

Network and archive failures occur before destination mutation. Installation
failures must leave the previous destination unchanged whenever rollback is
possible.

## Testing

Pure parser tests will construct synthetic tarballs for:

- root `SKILL.md`;
- `skills/{name}/SKILL.md`;
- categorized and hidden catalog layouts;
- plugin- or monorepo-nested layouts;
- bundled nested resources and binary bytes;
- executable files;
- missing, malformed, and mismatched frontmatter;
- duplicate matching names;
- unsafe traversal paths and every rejected special entry type;
- malformed gzip and tar streams; and
- each entry-count, file-count, and size limit.

Installer tests will verify:

- nested resource creation;
- binary content preservation;
- executable permissions on Unix;
- stale-file removal when updating;
- rollback behavior when replacement fails; and
- cleanup of staging and backup directories.

Existing registry and CLI tests will be updated for `SkillPackage`. The
verification commands are:

```text
cargo fmt --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```



## Compatibility

The CLI remains:

```text
agm skill add github:vercel-labs/skills --skill find-skills
```

Source parsing and harness destination selection remain unchanged. The visible
behavioral change is that AGM discovers the skill anywhere in the repository
and installs its complete directory instead of only `SKILL.md`.