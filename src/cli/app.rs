use crate::cli::args::{Cli, Commands, McpAction, SkillAction};
use crate::core::config::{AgmConfig, init_config};
use crate::core::registry::parse_source;
use crate::core::skills::add_skill;
use color_eyre::eyre::{Result, bail};

/// Executes the parsed CLI command.
pub async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Some(Commands::Init) => {
            init_config(&AgmConfig::new())?;
        }
        Some(Commands::Skill {
            action: SkillAction::List,
        }) => todo!(),
        Some(Commands::Skill {
            action: SkillAction::Add { source, skill },
        }) => add_skill(parse_source(&source)?, &skill).await?,
        Some(Commands::Mcp {
            action: McpAction::List,
        }) => todo!(),
        None => {
            bail!("No command provided. Use --help to see available commands.");
        }
    }

    Ok(())
}
