use anyhow::{anyhow, Result};
use std::fs;
use std::process::Command;

pub fn run(args: &[String]) -> Result<()> {
    let root = args
        .first()
        .map(String::as_str)
        .ok_or_else(|| anyhow!("need target root path"))?;
    let stage = args
        .get(1)
        .map(String::as_str)
        .ok_or_else(|| anyhow!("need staging dir path"))?;

    let systemd_dir = format!("{root}/etc/systemd/system");
    let mash_system_dir = format!("{root}/usr/local/lib/mash/system");
    let wants_dir = format!("{root}/etc/systemd/system/multi-user.target.wants");

    fs::create_dir_all(&systemd_dir)?;
    fs::create_dir_all(&mash_system_dir)?;
    fs::create_dir_all(&wants_dir)?;

    let _ = Command::new("install")
        .args([
            "-m",
            "0644",
            &format!("{stage}/systemd/mash-early-ssh.service"),
            &format!("{systemd_dir}/mash-early-ssh.service"),
        ])
        .status();
    let _ = Command::new("install")
        .args([
            "-m",
            "0755",
            &format!("{stage}/systemd/early-ssh.sh"),
            &format!("{mash_system_dir}/early-ssh.sh"),
        ])
        .status();

    let _ = Command::new("ln")
        .args([
            "-sf",
            "../mash-early-ssh.service",
            &format!("{wants_dir}/mash-early-ssh.service"),
        ])
        .status();

    Ok(())
}
