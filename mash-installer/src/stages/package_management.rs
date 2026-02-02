use anyhow::{Context, Result};
use log::warn;
use std::env;
use std::process::Command;

const DNF_BIN_ENV: &str = "MASH_DNF_BIN";

pub fn install_packages(package_list: &[&str]) -> Result<()> {
    let dnf_bin = env::var(DNF_BIN_ENV).unwrap_or_else(|_| "dnf".to_string());
    let mut cmd = Command::new(dnf_bin);
    cmd.args([
        "install",
        "-y",
        "--skip-unavailable",
        "--setopt=install_weak_deps=True",
    ]);
    cmd.args(package_list);

    let status = cmd
        .status()
        .context("failed to run package install command")?;
    if !status.success() {
        warn!("package install command exited with status {status}");
    }
    Ok(())
}
