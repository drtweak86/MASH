use anyhow::{anyhow, Result};
use std::path::Path;
use std::process::Command;

pub fn run(args: &[String]) -> Result<()> {
    let src = args
        .first()
        .map(String::as_str)
        .unwrap_or("/data/mash-staging");
    let src_path = Path::new(src);
    if !src_path.is_dir() {
        return Err(anyhow!("Usage: install_dojo <path to mash-staging>"));
    }

    println!("Installing MASH Dojo system files… 🥋");

    let _ = Command::new("mkdir")
        .args([
            "-p",
            "/usr/local/lib/mash/dojo",
            "/usr/local/lib/mash/system",
            "/usr/local/bin",
            "/etc/xdg/autostart",
            "/usr/local/lib/mash/dojo/assets",
        ])
        .status();

    let _ = Command::new("rsync")
        .args([
            "-a",
            &format!("{}/usr_local_lib_mash/dojo/", src),
            "/usr/local/lib/mash/dojo/",
        ])
        .status();
    let _ = Command::new("rsync")
        .args([
            "-a",
            &format!("{}/usr_local_lib_mash/system/", src),
            "/usr/local/lib/mash/system/",
        ])
        .status();

    let _ = Command::new("install")
        .args([
            "-m",
            "0755",
            &format!("{}/usr_local_bin/mash-dojo-launch", src),
            "/usr/local/bin/mash-dojo-launch",
        ])
        .status();

    let _ = Command::new("install")
        .args([
            "-m",
            "0644",
            &format!("{}/autostart/mash-dojo.desktop", src),
            "/etc/xdg/autostart/mash-dojo.desktop",
        ])
        .status();

    let starship = src_path.join("assets/starship.toml");
    if starship.is_file() {
        let _ = Command::new("install")
            .args([
                "-m",
                "0644",
                starship.to_string_lossy().as_ref(),
                "/usr/local/lib/mash/dojo/assets/starship.toml",
            ])
            .status();
    }

    let early_service = src_path.join("systemd/mash-early-ssh.service");
    if early_service.is_file() {
        let _ = Command::new("install")
            .args([
                "-m",
                "0644",
                early_service.to_string_lossy().as_ref(),
                "/etc/systemd/system/mash-early-ssh.service",
            ])
            .status();
        let _ = Command::new("install")
            .args([
                "-m",
                "0755",
                &format!("{}/systemd/early-ssh.sh", src),
                "/usr/local/lib/mash/system/early-ssh.sh",
            ])
            .status();
        let _ = Command::new("systemctl")
            .args(["enable", "mash-early-ssh.service"])
            .status();
    }

    let internet_service = src_path.join("systemd/mash-internet-wait.service");
    if internet_service.is_file() {
        let _ = Command::new("install")
            .args([
                "-m",
                "0644",
                internet_service.to_string_lossy().as_ref(),
                "/etc/systemd/system/mash-internet-wait.service",
            ])
            .status();
        let _ = Command::new("install")
            .args([
                "-m",
                "0755",
                &format!("{}/systemd/internet-wait.sh", src),
                "/usr/local/lib/mash/system/internet-wait.sh",
            ])
            .status();
        let _ = Command::new("systemctl")
            .args(["enable", "mash-internet-wait.service"])
            .status();
    }

    let _ = Command::new("systemctl").args(["daemon-reload"]).status();

    println!("✅ Dojo installed.");
    println!("➡️  Log out + back in (or reboot) and the Dojo should appear automatically.");
    println!("Manual launch:  /usr/local/bin/mash-dojo-launch");

    Ok(())
}
