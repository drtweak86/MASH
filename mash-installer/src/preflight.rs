use crate::{cli::Cli, errors::Result};
use std::process::Command;

pub fn run(_cli: &Cli, dry_run: bool) -> Result<()> {
    log::info!("🧪 Phase 1A: Preflight");
    log::info!("MASH root: {}", _cli.mash_root.display());

    let tools = ["rsync", "pv", "parted", "losetup", "wipefs", "mkfs.vfat", "mkfs.ext4", "mkfs.btrfs", "mount", "umount", "lsblk", "findmnt"];
    for t in tools {
        if which(t) {
            log::info!("✅· found {}", t);
        } else {
            log::warn!("⚠️· missing {}", t);
        }
    }

    if dry_run {
        log::info!("(dry-run) no changes made. ✅·");
    }
    Ok(())
}

fn which(cmd: &str) -> bool {
    Command::new("sh")
        .arg("-lc")
        .arg(format!("command -v {} >/dev/null 2>&1", cmd))
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
