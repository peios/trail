mod build;
mod cli;
mod export;
mod highlight;
mod images;
mod links;
mod markdown;
mod refs;
mod render;
mod search;
mod serve;
mod site;

use clap::Parser;

use crate::cli::{Cli, Command};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Build(args) => build::run(&args),
        Command::Serve(args) => serve::run(&args),
    }
}
