use anyhow::Result;
use blur::cli::{Cli, Command};
use clap::Parser;

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Doctor(args) => blur::runtime::doctor(args),
        Command::Play(args) => blur::runtime::play(args),
        Command::Profile(args) => blur::profile::run(args),
        Command::Unlock(args) => blur::unlock::run(args),
        Command::Lan(args) => blur::lan::run(args),
    }
}
