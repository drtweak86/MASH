use clap::Parser;
mod cli;
mod preflight;
mod flash;

fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    match &cli.command {
        cli::Command::Preflight { dry_run } => preflight::run(*dry_run)?,
        cli::Command::Flash { image, disk, uefi_dir, dry_run } => {
            flash::run(image, disk, uefi_dir, *dry_run)?;
        }
    }
    Ok(())
}
