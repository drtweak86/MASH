use anyhow::{anyhow, Result};
use mash_hal::ProcessOps;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct FormatOptions {
    pub dry_run: bool,
    pub confirmed: bool,
    pub extra_args: Vec<String>,
}

impl FormatOptions {
    pub fn new(dry_run: bool, confirmed: bool) -> Self {
        Self {
            dry_run,
            confirmed,
            extra_args: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
}

pub fn format_ext4(device: &Path, opts: &FormatOptions) -> Result<()> {
    ensure_confirmed(opts)?;
    if opts.dry_run {
        log::info!("DRY RUN: mkfs.ext4 {}", device.display());
        return Ok(());
    }
    let spec = ext4_command_spec(device, opts);
    run_command(&spec)
}

pub fn format_btrfs(device: &Path, opts: &FormatOptions) -> Result<()> {
    ensure_confirmed(opts)?;
    if opts.dry_run {
        log::info!("DRY RUN: mkfs.btrfs {}", device.display());
        return Ok(());
    }
    let spec = btrfs_command_spec(device, opts);
    run_command(&spec)
}

pub fn ext4_command_spec(device: &Path, opts: &FormatOptions) -> CommandSpec {
    let mut args = opts.extra_args.clone();
    args.push(device.display().to_string());
    CommandSpec {
        program: "mkfs.ext4".to_string(),
        args,
    }
}

pub fn btrfs_command_spec(device: &Path, opts: &FormatOptions) -> CommandSpec {
    let mut args = opts.extra_args.clone();
    args.push(device.display().to_string());
    CommandSpec {
        program: "mkfs.btrfs".to_string(),
        args,
    }
}

fn ensure_confirmed(opts: &FormatOptions) -> Result<()> {
    if !opts.confirmed {
        return Err(anyhow!("Formatting requires explicit confirmation"));
    }
    Ok(())
}

fn run_command(spec: &CommandSpec) -> Result<()> {
    let hal = mash_hal::LinuxHal::new();
    let args: Vec<&str> = spec.args.iter().map(String::as_str).collect();
    if let Err(_err) = hal.command_status(&spec.program, &args, Duration::from_secs(10 * 60)) {
        return Err(anyhow!("Command failed: {}", spec.program));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ext4_command_is_constructed() {
        let opts = FormatOptions::new(true, true);
        let spec = ext4_command_spec(Path::new("/dev/sda1"), &opts);
        assert_eq!(spec.program, "mkfs.ext4");
        assert_eq!(spec.args, vec!["/dev/sda1".to_string()]);
    }

    #[test]
    fn btrfs_command_is_constructed() {
        let opts = FormatOptions::new(true, true);
        let spec = btrfs_command_spec(Path::new("/dev/sda2"), &opts);
        assert_eq!(spec.program, "mkfs.btrfs");
        assert_eq!(spec.args, vec!["/dev/sda2".to_string()]);
    }

    #[test]
    fn format_requires_confirmation() {
        let opts = FormatOptions::new(true, false);
        let err = format_ext4(Path::new("/dev/sda1"), &opts).unwrap_err();
        assert!(err.to_string().contains("explicit confirmation"));
    }
}
