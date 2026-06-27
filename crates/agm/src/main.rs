mod app;
mod args;
use crate::app::run;
use crate::args::Cli;
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
