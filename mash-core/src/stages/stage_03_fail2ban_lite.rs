use anyhow::Result;
use mash_hal::ProcessOps;
use std::fs;
use std::time::Duration;

const JAIL_CONFIG: &str = r#"[DEFAULT]
# Don't ban RFC1918 LAN ranges (keeps Batcave safe)
ignoreip = 127.0.0.1/8 ::1 10.0.0.0/8 172.16.0.0/12 192.168.0.0/16
bantime  = 1h
findtime = 10m
maxretry = 6

[sshd]
enabled = true
"#;

pub fn run(_args: &[String]) -> Result<()> {
    log::info!("🛡️  fail2ban-lite: enabling sshd jail (LAN safe)");

    let hal = mash_hal::LinuxHal::new();
    let _ = hal.command_status(
        "dnf",
        &["install", "-y", "fail2ban"],
        Duration::from_secs(60 * 60),
    );

    fs::create_dir_all("/etc/fail2ban")?;
    fs::write("/etc/fail2ban/jail.d/mash-local.conf", JAIL_CONFIG)?;

    let _ = hal.command_status(
        "systemctl",
        &["enable", "--now", "fail2ban"],
        Duration::from_secs(60),
    );
    let _ = hal.command_status(
        "systemctl",
        &["status", "fail2ban", "--no-pager"],
        Duration::from_secs(60),
    );

    log::info!("✅ fail2ban running. LAN ignored. 🛡️");
    Ok(())
}
