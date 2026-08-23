use std::net::SocketAddr;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// The static site builder for the Peios learn documentation.
#[derive(Debug, Parser)]
#[command(name = "trail", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Build the site into the output directory.
    Build(BuildArgs),
    /// Build the site, then serve it over local HTTP.
    Serve(ServeArgs),
}

#[derive(Debug, Args)]
pub struct BuildArgs {
    /// Site root directory (the one containing trail.toml).
    #[arg(value_name = "ROOT", default_value = ".")]
    pub root: PathBuf,

    /// Directory the built site is written into [default: ROOT/dist].
    #[arg(long, value_name = "DIR")]
    pub out: Option<PathBuf>,

    /// Warn instead of failing when a ~link's target page does not exist.
    /// Ambiguous links still fail: they need the author to pick a candidate.
    #[arg(long)]
    pub allow_dangling_links: bool,

    /// Fail the build when an article has no `description:` frontmatter.
    /// Descriptions become each page's meta description and social
    /// preview text, and its standfirst in the markdown mirrors.
    #[arg(long)]
    pub strict: bool,

    /// Also write each unit's print.md as llms-full.txt — a discovery
    /// fallback for agents that probe for that name instead of reading
    /// llms.txt (which always points at print.md).
    #[arg(long)]
    pub render_llms_full: bool,

    /// Publish a second copy of the whole site under this path, with every
    /// link inside it rewritten to stay in the copy. Give it a name that
    /// changes on every deploy — a commit hash — and the copy's URLs have
    /// never been requested before, so nothing can serve them from a cache.
    /// The copy is `noindex` and stays out of sitemap.xml; the real site is
    /// unchanged apart from learning the path, which its /cb command uses.
    #[arg(long, value_name = "PATH")]
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

    /// Address to serve on.
    #[arg(long, value_name = "ADDR", default_value = "127.0.0.1:8724")]
    pub addr: SocketAddr,
}
