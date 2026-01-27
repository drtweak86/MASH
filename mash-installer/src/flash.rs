use crate::errors::MashError;
use crate::logging;
use anyhow::{Context, Result};
use log::info;
use std::path::Path;
use std::process::Command;

pub fn run(
    image: &Path,
    disk: &str,
    uefi_dir: &Path,
    dry_run: bool,
    auto_unmount: bool,
    yes_i_know: bool,
) -> Result<()> {
    info!("🎮 Phase 1B: Disk Flashing");
    info!("Target disk : /dev/{}", disk);
    info!("Image       : {}", image.display());
    info!("UEFI dir    : {}", uefi_dir.display());

    confirm_gate(disk, yes_i_know)?;

    if auto_unmount {
        unmount_disk(disk, dry_run)?;
    }

    if dry_run {
        info!("🧪 Dry-run enabled — no changes will be made.");
        return Ok(());
    }

    wipe_partition_table(disk)?;
    create_partitions(disk)?;
    format_partitions(disk)?;

    mount_and_copy(image, disk)?;
    stage_uefi(disk, uefi_dir)?;

    info!("✅ Phase 1B complete.");
    Ok(())
}

fn confirm_gate(disk: &str, yes_i_know: bool) -> Result<()> {
    if yes_i_know {
        info!("⚠️  --yes-i-know supplied. Skipping confirmation.");
        return Ok(());
    }

    println!();
    println!("⚠️  WARNING ⚠️");
    println!("You are about to ERASE /dev/{}", disk);
    println!("This action is IRREVERSIBLE.");
    println!("Type the disk name ({}) to continue:", disk);

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    if input.trim() != disk {
        return Err(MashError::Abort("Disk confirmation failed".into()).into());
    }

    Ok(())
}

fn unmount_disk(disk: &str, dry_run: bool) -> Result<()> {
    info!("🔌 Unmounting any mounts on /dev/{}", disk);

    let cmd = format!("lsblk -ln /dev/{} | awk '{{print $1}}'", disk);

    let output = Command::new("bash")
        .arg("-c")
        .arg(&cmd)
        .output()
        .context("Failed to list partitions")?;

    let parts = String::from_utf8_lossy(&output.stdout);

    for part in parts.lines().skip(1) {
        let dev = format!("/dev/{}", part);
        if dry_run {
            info!("(dry-run) would umount {}", dev);
        } else {
            Command::new("umount")
                .arg("-f")
                .arg(&dev)
                .status()
                .ok();
        }
    }

    Ok(())
}

fn wipe_partition_table(disk: &str) -> Result<()> {
    info!("🧹 Wiping partition table");

    Command::new("wipefs")
        .args(["-a", &format!("/dev/{}", disk)])
        .status()
        .context("wipefs failed")?;

    Ok(())
}

fn create_partitions(disk: &str) -> Result<()> {
    info!("📐 Creating MBR partitions");

    let script = r#"
        mklabel msdos
        mkpart primary fat32 4MiB 1024MiB
        set 1 boot on
        mkpart primary ext4 1024MiB 3072MiB
        mkpart primary btrfs 3072MiB 1800GiB
        mkpart primary ext4 1800GiB 100%
    "#;

    Command::new("parted")
        .args(["/dev/{}", disk])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut p| {
            use std::io::Write;
            p.stdin.as_mut().unwrap().write_all(script.as_bytes())?;
            p.wait()?;
            Ok(())
        })
        .context("partitioning failed")?;

    Ok(())
}

fn format_partitions(disk: &str) -> Result<()> {
    info!("🧪 Formatting filesystems");

    let dev = |n| format!("/dev/{}{}", disk, n);

    Command::new("mkfs.vfat").arg(dev("1")).status()?;
    Command::new("mkfs.ext4").arg("-F").arg(dev("2")).status()?;
    Command::new("mkfs.btrfs").arg("-f").arg(dev("3")).status()?;
    Command::new("mkfs.ext4").arg("-F").arg(dev("4")).status()?;

    Ok(())
}

fn mount_and_copy(image: &Path, disk: &str) -> Result<()> {
    info!("📦 Copying Fedora image (rsync)");

    // This is intentionally stubbed for now.
    // We will wire rsync + pv + Pac-Man animation next.

    Ok(())
}

fn stage_uefi(disk: &str, uefi_dir: &Path) -> Result<()> {
    info!("🧠 Staging Pi UEFI firmware");

    // Stub — real implementation next step
    Ok(())
}
