mod cli;
mod core;
use crate::cli::app::run;
use crate::cli::args::Cli;
use clap::Parser;
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    // Initialize the default formatter and logger layer
    tracing_subscriber::fmt::init();

    let cli: Cli = Cli::parse();

    match run(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}
