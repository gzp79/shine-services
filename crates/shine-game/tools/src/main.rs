//! Developer tools of `shine-game`: code generators and similar tasks, run on a desktop.

use clap::{Parser, Subcommand};

mod commands;

#[derive(Parser)]
#[command(name = "shine-game-tools", about = "Developer tools of shine-game")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generates the TypeScript loaders of the stream snapshots (chat, pixel map).
    TsBindings(commands::ts_bindings::Args),
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::TsBindings(args) => commands::ts_bindings::run(args),
    }
}
