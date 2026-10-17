use std::net::SocketAddr;
use std::path::PathBuf;

use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand};

/// Build and check Peios documentation
#[derive(Debug, Parser)]
#[command(
    name = "trail",
    version,
    propagate_version = true,
    term_width = 80,
    subcommand_value_name = "command",
    after_help = "See https://learn.peios.org/trail/",
    disable_help_subcommand = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    /// Keep clap's generated entries while spelling usage placeholders as PSC
    /// requires. The invocation name follows argv[0], including symlinks.
    pub fn try_parse_from(arguments: Vec<std::ffi::OsString>) -> Result<Self, clap::Error> {
        let program = arguments
            .first()
            .and_then(|name| std::path::Path::new(name).file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "trail".into());
        let mut parser = Self::command()
            .bin_name(program.clone())
            .override_usage(format!("{program} [<options>] <command>"));
        for command in parser.get_subcommands_mut() {
            let name = command.get_name().to_string();
            let argument = if name == "help" { "command" } else { "root" };
            *command = command
                .clone()
                .override_usage(format!("{program} {name} [<options>] [<{argument}>]"));
        }
        Self::from_arg_matches(&parser.try_get_matches_from(arguments)?)
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Build the site into the output directory
    #[command(after_help = "See https://learn.peios.org/trail/building/build-and-serve/")]
    Build(BuildArgs),
    /// Build the site and serve it over local HTTP
    #[command(after_help = "See https://learn.peios.org/trail/building/build-and-serve/")]
    Serve(ServeArgs),
    /// Check the site's configured writing rules
    #[command(after_help = "See https://learn.peios.org/trail/writing/checking-style/")]
    Lint(LintArgs),
    /// Print help for Trail or one of its commands
    #[command(after_help = "See https://learn.peios.org/trail/")]
    Help {
        /// Show help for this command
        #[arg(value_name = "<command>", value_parser = ["build", "serve", "lint", "help"], hide_possible_values = true)]
        command: Option<String>,
    },
}

#[derive(Debug, Args)]
pub struct LintArgs {
    /// Read the site from this directory (default: .)
    #[arg(value_name = "<root>", default_value = ".", hide_default_value = true)]
    pub root: PathBuf,
}

#[derive(Debug, Args)]
pub struct BuildArgs {
    /// Read the site from this directory (default: .)
    #[arg(value_name = "<root>", default_value = ".", hide_default_value = true)]
    pub root: PathBuf,

    /// Set output directory (default: <root>/dist)
    #[arg(long, value_name = "directory", display_order = 3)]
    pub out: Option<PathBuf>,

    /// Warn about missing link targets
    #[arg(long, display_order = 0)]
    pub allow_dangling_links: bool,

    /// Require descriptions; warn about absolute links
    #[arg(long, display_order = 5)]
    pub strict: bool,

    /// Rebuild without reading or writing build caches
    #[arg(long, display_order = 2)]
    pub no_cache: bool,

    /// Also write each print.md bundle as llms-full.txt
    #[arg(long, display_order = 4)]
    pub render_llms_full: bool,

    /// Publish a second copy under this path
    #[arg(long, value_name = "path", display_order = 1)]
    pub cbpath: Option<String>,
}

impl BuildArgs {
    pub fn out_dir(&self) -> PathBuf {
        self.out.clone().unwrap_or_else(|| self.root.join("dist"))
    }
}

#[derive(Debug, Args)]
pub struct ServeArgs {
    #[command(flatten)]
    pub build: BuildArgs,

    /// Listen on this address (default: 127.0.0.1:8724)
    #[arg(
        long,
        value_name = "address",
        default_value = "127.0.0.1:8724",
        hide_default_value = true,
        display_order = 0
    )]
    pub addr: SocketAddr,
}
