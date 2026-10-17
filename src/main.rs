mod build;
mod cache;
mod cli;
mod diagnostic;
mod export;
mod highlight;
mod images;
mod links;
mod lint;
mod markdown;
mod math;
mod memo;
mod parsed;
mod plugins;
mod refs;
mod render;
mod search;
mod serve;
mod site;
mod titlecase;
mod typography;

use crate::cli::{Cli, Command};

fn main() -> std::process::ExitCode {
    let cli = match Cli::try_parse_from(std::env::args_os().collect()) {
        Ok(cli) => cli,
        Err(error) => return diagnostic::arguments(error),
    };
    let result = match cli.command {
        Command::Build(args) => build::run(&args),
        Command::Serve(args) => serve::run(&args),
        Command::Lint(args) => match lint::run(&args.root) {
            Ok(true) => return std::process::ExitCode::FAILURE,
            Ok(false) => Ok(()),
            Err(error) => Err(error),
        },
        Command::Help { command } => {
            let mut arguments = vec![std::ffi::OsString::from(diagnostic::program())];
            if let Some(name) = command {
                arguments.push(name.into());
            }
            arguments.push("--help".into());
            // The explicit help flag always produces clap's display result.
            return diagnostic::arguments(Cli::try_parse_from(arguments).unwrap_err());
        }
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            diagnostic::error(&error);
            std::process::ExitCode::FAILURE
        }
    }
}
