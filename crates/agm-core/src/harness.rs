use std::path::PathBuf;

/// Shared struct holding the project-local and global skill directory paths.
#[derive(Debug, Clone)]
pub struct SkillsDir {
    /// Project-scoped skills directory (for example, `.agents/skills`).
    pub project: PathBuf,
    /// User-global skills directory (for example, `~/.agents/skills`).
    pub global: PathBuf,
}
