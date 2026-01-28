use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about="MASH Phase 1 Installer")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Preflight { #[arg(long)] dry_run: bool },
    Flash {
        #[arg(long)] image: String,
        #[arg(long)] disk: String,
        #[arg(long)] uefi_dir: String,
        #[arg(long)] dry_run: bool,
    },
}
