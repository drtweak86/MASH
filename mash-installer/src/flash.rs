use anyhow::{Context, Result, bail};
use log::info;
use std::path::Path;
use std::process::Command;

use crate::errors::MashError;

pub fn run(
    image: &Path,
    disk: &str,
    uefi_dir: &Path,
    dry_run: bool,
    _auto_unmount: bool,  // Prefix unused with _
    yes_i_know: bool,
) -> Result<()> {
    info!("🎮 MASH Full-Loop Installer: Fedora KDE + UEFI Boot for RPi4");
    
    let disk = normalize_disk(disk);
    info!("Target disk: {}", disk);
    info!("Image: {}", image.display());
    info!("UEFI dir: {}", uefi_dir.display());

    // Verify image exists
    if !image.exists() {
        bail!("Image file not found: {}", image.display());
    }

    // Show disk info
    show_lsblk(&disk)?;

    // Safety check
    if !yes_i_know && !dry_run {
        return Err(MashError::MissingYesIKnow.into());
    }

    if dry_run {
        info!("(dry-run) Would perform full installation");
        return Ok(());
    }

    info!("✅ Installation pipeline ready");
    info!("Full implementation coming in next update");
    
    Ok(())
}

fn normalize_disk(d: &str) -> String {
    if d.starts_with("/dev/") { 
        d.to_string() 
    } else { 
        format!("/dev/{}", d) 
    }
}

fn show_lsblk(disk: &str) -> Result<()> {
    info!("🧾 Current disk layout for {}", disk);
    let output = Command::new("lsblk")
        .args(["-o", "NAME,SIZE,TYPE,FSTYPE,MOUNTPOINTS,MODEL", disk])
        .output()
        .context("Failed to run lsblk")?;
    
    info!("\n{}", String::from_utf8_lossy(&output.stdout));
    Ok(())
}
