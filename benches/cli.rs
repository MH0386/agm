use agm::cli::args::Cli;
use clap::{CommandFactory, Parser};
use divan::{Bencher, black_box};

fn main() {
    divan::main();
}

/// Builds the clap command tree, as done on every invocation.
#[divan::bench]
fn build_command() -> clap::Command {
    Cli::command()
}

/// Parses the argument sets accepted by the CLI.
#[divan::bench(args = ["", "--debug", "init", "skill list", "mcp list"])]
fn parse(bencher: Bencher, arguments: &str) {
    let argv: Vec<&str> = std::iter::once("agm")
        .chain(arguments.split_whitespace())
        .collect();

    bencher.bench(|| Cli::parse_from(black_box(argv.iter().copied())));
}

/// Renders the long help output, the fallback path of a bare invocation.
#[divan::bench]
fn render_long_help() -> String {
    Cli::command().render_long_help().to_string()
}
