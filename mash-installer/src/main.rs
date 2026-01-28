mod cli;
mod errors;
mod flash;
mod locale;
mod logging;
mod preflight;
mod tui;

use clap::Parser;
use crate::cli::{Cli, Command};
use crate::errors::Result;

fn main() -> Result<()> {
    let cli = Cli::parse();
    logging::init();

    match &cli.cmd {
        Command::Preflight { dry_run } => preflight::run(&cli, *dry_run),
        Command::Flash {
            image,
            disk,
            uefi_dir,
            dry_run,
            auto_unmount,
            yes_i_know,
            watch,
        } => flash::run(
            &cli,
            image,
            disk,
            uefi_dir,
            *dry_run,
            *auto_unmount,
            *yes_i_know,
            *watch,
        ),
        Command::Tui { watch, dry_run } => tui::run(&cli, *watch, *dry_run),
    }
}
