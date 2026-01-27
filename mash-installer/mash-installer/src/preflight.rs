use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Command;

use crate::cli::Cli;

pub fn run(cli: &Cli, dry_run: bool) -> Result<()> {
    log::info!("🧪 Phase 1A: Preflight");
    let mash_root = expand_tilde(&cli.mash_root)?;
    log::info!("MASH root: {}", mash_root.display());

    // Check tools
    for bin in ["rsync", "pv", "parted", "losetup"] {
        which(bin).with_context(|| format!("Missing required tool: {bin}"))?;
        log::info!("✅ found {bin}");
    }

    if dry_run {
        log::info!("(dry-run) no changes made. ✅");
    }
    Ok(())
}

fn which(bin: &str) -> Result<PathBuf> {
    let out = Command::new("bash")
        .args(["-lc", &format!("command -v {bin}")])
        .output()
        .context("failed to run shell")?;

    if !out.status.success() {
        anyhow::bail!("not found");
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(PathBuf::from(s))
}

fn expand_tilde(p: &str) -> Result<PathBuf> {
    if p.starts_with("~/") {
        let home = std::env::var("HOME").context("HOME not set")?;
        Ok(PathBuf::from(home).join(p.trim_start_matches("~/")))
    } else if p == "~" {
        let home = std::env::var("HOME").context("HOME not set")?;
        Ok(PathBuf::from(home))
    } else {
        Ok(PathBuf::from(p))
    }
}
