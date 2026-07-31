//! `SKILL.md` YAML frontmatter extraction and typed fields.

use color_eyre::eyre::{ContextCompat, Result, bail};
use serde::Deserialize;
use std::path::Path;

/// Required frontmatter fields from the Agent Skills format.
#[derive(Debug, Deserialize)]
pub(super) struct SkillFrontmatter {
    pub(super) name: String,
    pub(super) description: String,
}

/// Returns the YAML block between the opening and closing `---` delimiters.
pub(super) fn frontmatter_yaml<'a>(text: &'a str, repository_path: &Path) -> Result<&'a str> {
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
