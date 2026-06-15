use anyhow::Result;

use crate::cli::args::{Cli, Commands, McpAction, SkillAction};
use crate::core::config::{AgmConfig, init_config};

/// Executes the parsed CLI command.
pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Some(Commands::Init) => {
            init_config(&AgmConfig::new())?;
        }
        Some(Commands::Skill {
            action: SkillAction::List,
        }) => todo!(),
        Some(Commands::Mcp {
            action: McpAction::List,
        }) => todo!(),
        None => {
            println!("No command provided. Use --help to see available commands.");
        }
    }

    Ok(())
}
