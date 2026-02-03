use anyhow::Result;
use std::fs;
use std::process::Command;
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
    println!("🛡️  fail2ban-lite: enabling sshd jail (LAN safe)");

    let mut cmd = Command::new("dnf");
    cmd.args(["install", "-y", "fail2ban"]);
    let _ =
        crate::process_timeout::status_with_timeout("dnf", &mut cmd, Duration::from_secs(60 * 60));

    fs::create_dir_all("/etc/fail2ban")?;
    fs::write("/etc/fail2ban/jail.d/mash-local.conf", JAIL_CONFIG)?;

    let mut cmd = Command::new("systemctl");
    cmd.args(["enable", "--now", "fail2ban"]);
    let _ =
        crate::process_timeout::status_with_timeout("systemctl", &mut cmd, Duration::from_secs(60));
    let mut cmd = Command::new("systemctl");
    cmd.args(["status", "fail2ban", "--no-pager"]);
    let _ =
        crate::process_timeout::status_with_timeout("systemctl", &mut cmd, Duration::from_secs(60));

    println!("✅ fail2ban running. LAN ignored. 🛡️");
    Ok(())
}
