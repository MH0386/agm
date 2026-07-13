use clap::{ArgAction, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    version,
    about,
    long_about = "The open-source agent manager built in Rust 🦀"
)]
pub struct Cli {
    /// Turn debugging information on
    #[arg(short, long, action = ArgAction::SetTrue)]
    pub debug: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Initialize the AGM configuration
    Init,
    /// Skill commands
    Skill {
        #[command(subcommand)]
        action: SkillAction,
    },
    /// MCP commands
    Mcp {
        #[command(subcommand)]
        action: McpAction,
    },
}

#[derive(Debug, Subcommand)]
pub enum SkillAction {
    /// List all managed skills
    List,
    /// Add a skill from a registry source
    Add {
        /// Registry source (e.g. github:owner/repo)
        source: String,
        /// Name of the specific skill to install
        #[arg(long)]
        skill: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum McpAction {
    /// List all managed MCPs
    List,
}
