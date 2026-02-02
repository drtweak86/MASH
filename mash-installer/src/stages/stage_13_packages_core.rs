use anyhow::Result;

use crate::stages::package_management;

pub fn run(_args: &[String]) -> Result<()> {
    println!("[*] Packages: core");
    package_management::install_packages(&[
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
    ])?;
    Ok(())
}
