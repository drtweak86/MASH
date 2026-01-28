use clap::Parser;
mod cli;
mod preflight;
mod flash;
mod logging;
mod errors;

fn main() -> anyhow::Result<()> {
    logging::init();
    let cli = cli::Cli::parse();

    match &cli.command {
        cli::Command::Preflight { dry_run } => {
            preflight::run(*dry_run)?;
        }
        cli::Command::Flash { image, disk, uefi_dir, dry_run, auto_unmount, yes_i_know } => {
            flash::run(image, disk, uefi_dir, *dry_run, *auto_unmount, *yes_i_know)?;
        }
    }

    Ok(())
}
