//! Development tooling: fixtures, the local gate, the corpus and releases.
//!
//! Run with `cargo run -p xtask -- <command>`; see `--help`.

mod ci;
mod corpus;
mod fixtures;
mod msi;
mod release;
mod tool;
mod verify;

use std::error::Error;

use clap::{Parser, Subcommand};

/// Subcommand names, also used to keep the old bare-path invocation working.
const COMMANDS: &[&str] = &[
    "fixtures",
    "ci",
    "release",
    "regen-corpus",
    "verify",
    "help",
];

#[derive(Debug, Parser)]
#[command(name = "xtask", about = "Development tooling for Comical-RS")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Write synthetic archive fixtures.
    Fixtures {
        /// Output directory. Defaults to `target/fixtures`.
        directory: Option<String>,
    },
    /// Run the local gate: formatting, clippy and tests.
    Ci,
    /// Build release artifacts for this host and any cross targets.
    Release {
        /// Version to package. Defaults to the workspace version.
        version: Option<String>,
    },
    /// Rebuild the real-writer corpus with Info-ZIP and 7-Zip.
    #[command(name = "regen-corpus")]
    RegenCorpus,
    /// Check a finished release directory before publishing it.
    Verify {
        /// Release directory. Defaults to `target/release-dist/<version>`.
        directory: Option<String>,
    },
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args: Vec<String> = std::env::args().collect();
    // Fixture generation was `cargo run -p xtask -- target/fixtures` before
    // subcommands existed; keep that form working.
    if let Some(first) = args.get(1)
        && !first.starts_with('-')
        && !COMMANDS.contains(&first.as_str())
    {
        args.insert(1, "fixtures".to_owned());
    }

    match Cli::parse_from(args).command {
        Command::Fixtures { directory } => {
            fixtures::run(directory.unwrap_or_else(|| "target/fixtures".to_owned()))
        }
        Command::Ci => ci::run(),
        Command::Release { version } => release::run(version),
        Command::RegenCorpus => corpus::run(),
        Command::Verify { directory } => verify::run(directory),
    }
}
