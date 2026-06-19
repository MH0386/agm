use crate::core::skills::SkillsDir;
use color_eyre::eyre::{Context, ContextCompat, Result};
use dirs;
use std::env;
use std::path::Path;

/// Supported AI harness types that AGM can install skills into.
///
/// Detection examines the current working directory for known marker
/// directories and returns the first matching harness (or `Standard` as the
/// fallback).
#[derive(Debug)]
pub enum Harness {
    /// Standard agentskills.io layout (`.agents/skills/`).
    Standard { skills_dir: SkillsDir },
    /// Pi agent (`.pi/skills/`).
    Pi { skills_dir: SkillsDir },
    /// OpenCode IDE (`.opencode/skills/`).
    OpenCode { skills_dir: SkillsDir },
}

impl Harness {
    /// Detects the active harness by checking for marker directories in the
    /// current working directory.
    ///
    /// Priority order: OpenCode > Pi > Standard (default).
    ///
    /// # Errors
    ///
    /// Returns an error if the current directory or home directory cannot be determined.
    pub fn detect() -> Result<Self> {
        let current_dir = env::current_dir().context("Failed to get current directory")?;
        let home_dir = dirs::home_dir().context("Failed to determine home directory")?;

        if Path::new(".opencode").is_dir() {
            Ok(Self::OpenCode {
                skills_dir: SkillsDir {
                    project: current_dir.join(".opencode").join("skills"),
                    global: home_dir.join(".config").join("opencode").join("skills"),
                },
            })
        } else if Path::new(".pi").is_dir() {
            Ok(Self::Pi {
                skills_dir: SkillsDir {
                    project: current_dir.join(".pi").join("skills"),
                    global: home_dir.join(".pi").join("agent").join("skills"),
                },
            })
        } else {
            Ok(Self::Standard {
                skills_dir: SkillsDir {
                    project: current_dir.join(".agents").join("skills"),
                    global: home_dir.join(".agents").join("skills"),
                },
            })
        }
    }

    /// Returns the project-local skills directory path.
    pub fn project_skills_dir(&self) -> &Path {
        match self {
            Self::Standard { skills_dir } => &skills_dir.project,
            Self::Pi { skills_dir } => &skills_dir.project,
            Self::OpenCode { skills_dir } => &skills_dir.project,
        }
    }

    /// Returns the global skills directory path.
    pub fn global_skills_dir(&self) -> &Path {
        match self {
            Self::Standard { skills_dir } => &skills_dir.global,
            Self::Pi { skills_dir } => &skills_dir.global,
            Self::OpenCode { skills_dir } => &skills_dir.global,
        }
    }
}

impl std::fmt::Display for Harness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Standard { .. } => write!(f, "Standard"),
            Self::Pi { .. } => write!(f, "Pi"),
            Self::OpenCode { .. } => write!(f, "OpenCode"),
        }
    }
}
