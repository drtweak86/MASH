use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "mash-release")]
#[command(about = "Release helper for the MASH installer")]
struct Cli {
    /// Show planned changes without modifying files or running git commands
    #[arg(long)]
    dry_run: bool,

    /// Skip creating a git tag
    #[arg(long)]
    no_tag: bool,

    /// Skip interactive prompts and accept defaults
    #[arg(short = 'y', long)]
    yes: bool,

    /// Use a specific commit message
    #[arg(long)]
    message: Option<String>,

    /// Override the computed tag (must be valid SemVer)
    #[arg(long)]
    tag: Option<String>,
}

fn main() {
    let _cli = Cli::parse();
}
