use color_eyre::eyre::{Result, bail};
use serde::{Deserialize, Serialize};
use std::fs::{File, read_to_string};
use std::io::BufWriter;
use std::path::Path;

const CONFIG_FILE: &str = "agm.json";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Serialize, Deserialize, Debug)]
pub struct AgmConfig {
    pub version: String,
    pub skills: Vec<String>,
    pub mcps: Vec<String>, // TODO: Add MCP config
}

impl AgmConfig {
    /// Creates a new default AGM config.
    pub fn new() -> Self {
        Self {
            version: APP_VERSION.to_string(),
            skills: Vec::new(),
            mcps: Vec::new(),
        }
    }
}

/// Initializes the config file at the given path.
pub fn init_config(config: &AgmConfig) -> Result<()> {
    // Check if config already exists to prevent silent overwrite
    if Path::new(CONFIG_FILE).exists() {
        bail!("{} already exists.", CONFIG_FILE);
    }

    // Create the config file
    let file = File::create(CONFIG_FILE)?;
    let writer = BufWriter::new(file);

    // Serialize and write to the file stream
    serde_json::to_writer_pretty(writer, &config)?;

    println!("Created {}.", CONFIG_FILE);
    Ok(())
}

/// Loads the agm config file.
pub fn load_config() -> Result<AgmConfig> {
    let content = read_to_string(CONFIG_FILE)?;
    let config: AgmConfig = serde_json::from_str(&content)?;

    Ok(config)
}
