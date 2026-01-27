mod cli;
mod config;
mod errors;
mod logging;
mod preflight;
mod flash;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    logging::init();
    let cli = cli::Cli::parse();

    match &cli.command {
        cli::Commands::Preflight { dry_run } => {
            preflight::run(&cli, *dry_run)?;
        }
        cli::Commands::Flash {
            image,
            disk,
            uefi_dir,
            dry_run,
            auto_unmount,
            yes_i_know,
        } => {
            flash::run(&cli, image, disk, uefi_dir, *dry_run, *auto_unmount, *yes_i_know)?;
        }
        cli::Commands::Dojo { .. } => {
            log::warn!("🥋 Dojo is Phase 1C — stub for now. (We’ll build it next.)");
        }
    }

    Ok(())
}
