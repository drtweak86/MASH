use anyhow::Result;
use std::process::Command;

pub fn run(_args: &[String]) -> Result<()> {
    println!("[*] Packages: core");

    let _ = Command::new("dnf")
        .args([
            "install",
            "-y",
            "--skip-unavailable",
            "--setopt=install_weak_deps=True",
            "git",
            "rsync",
            "curl",
            "wget",
            "tmux",
            "neovim",
            "btrfs-progs",
            "tree",
            "htop",
            "openssh-server",
            "mosh",
            "avahi",
            "nmap",
            "firewalld",
            "firewall-config",
            "fail2ban",
        ])
        .status();

    Ok(())
}
