use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

use crate::cli::Cli;
use crate::errors::MashError;

pub fn run(
    _cli: &Cli,
    image: &Path,
    disk: &str,
    uefi_dir: &Path,
    dry_run: bool,
    auto_unmount: bool,
    yes_i_know: bool,
) -> Result<()> {
    log::info!("🎮 Entering Phase 1B: Disk Flashing");

    let disk = normalize_disk(disk);
    log::info!("Target disk: {}", disk);
    log::info!("Image: {}", image.display());
    log::info!("UEFI dir: {}", uefi_dir.display());

    // Always show lsblk for the disk (helps avoid nuking wrong device)
    show_lsblk(&disk)?;

    // Safety latch: require --yes-i-know for destructive run (and also for unmount w/out prompt)
    if !yes_i_know && !dry_run {
        return Err(MashError::MissingYesIKnow.into());
    }

    // Unmount stage
    if auto_unmount {
        if dry_run {
            log::info!("(dry-run) would unmount anything mounted on {}", disk);
        } else {
            // Ask unless yes_i_know
            if !yes_i_know && !confirm(&format!("Unmount anything on {}?", disk))? {
                return Err(MashError::Aborted.into());
            }
            unmount_all(&disk)?;
        }
    }

    if dry_run {
        log::info!("(dry-run) stopping here (no changes made). 🫡");
        return Ok(());
    }

    // TODO: real flash pipeline (partition/format/copy/uefi/stage)
    log::warn!("🛠️ Real flash pipeline not wired yet. This is the safe scaffold.");
    Ok(())
}

fn normalize_disk(d: &str) -> String {
    if d.starts_with("/dev/") { d.to_string() } else { format!("/dev/{d}") }
}

fn show_lsblk(disk: &str) -> Result<()> {
    log::info!("🧾 lsblk for {}", disk);
    let cmd = format!("lsblk -o NAME,SIZE,TYPE,FSTYPE,MOUNTPOINTS,MODEL {} -r", disk);
    let out = Command::new("bash").args(["-lc", &cmd]).output().context("lsblk failed")?;
    log::info!("\n{}", String::from_utf8_lossy(&out.stdout));
    Ok(())
}

/// Best-effort unmount: find any mountpoints for children of disk and unmount deepest first.
fn unmount_all(disk: &str) -> Result<()> {
    log::info!("🧹 Unmounting anything on {}", disk);
    let cmd = format!(
        r#"lsblk -rno NAME,MOUNTPOINTS {d} | awk '$2 != "" {{print $2}}' | sort -r"#,
        d=disk
    );
    let out = Command::new("bash").args(["-lc", &cmd]).output().context("lsblk mount scan failed")?;
    let mounts = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();

    if mounts.is_empty() {
        log::info!("✅ Nothing mounted.");
        return Ok(());
    }

    for m in mounts {
        log::info!("umount {}", m);
        let status = Command::new("sudo").args(["umount", &m]).status().context("umount failed")?;
        if !status.success() {
            return Err(MashError::CommandFailed(format!("umount {m}")).into());
        }
    }
    log::info!("✅ Unmount complete.");
    Ok(())
}

fn confirm(prompt: &str) -> Result<bool> {
    use std::io::{self, Write};
    print!("{} [y/N]: ", prompt);
    io::stdout().flush().ok();
    let mut s = String::new();
    io::stdin().read_line(&mut s).ok();
    let s = s.trim().to_lowercase();
    Ok(matches!(s.as_str(), "y" | "yes"))
}
