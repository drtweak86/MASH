use anyhow::{anyhow, Result};
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::Path;

const EARLY_SSH_SCRIPT: &str = r#"#!/usr/bin/env bash
set -euo pipefail
echo "[mash-early-ssh] 🚚 Bringing SSH online (LAN-safe)…"
systemctl enable --now sshd || true
systemctl enable --now firewalld || true
if command -v firewall-cmd >/dev/null 2>&1; then
  firewall-cmd --permanent --add-service=ssh || true
  firewall-cmd --permanent --add-service=mosh || true
  firewall-cmd --permanent --add-service=mdns || true
  firewall-cmd --reload || true
fi
if systemctl list-unit-files | grep -q '^avahi-daemon\.service'; then
  systemctl enable --now avahi-daemon || true
fi
echo "[mash-early-ssh] ✅ SSH should now be reachable. Try: ssh <user>@mash.local 🚚💨"
"#;

const EARLY_SSH_SERVICE: &str = r#"[Unit]
Description=MASH early SSH bring-up (one-shot)
After=network-online.target
Wants=network-online.target
ConditionPathExists=/usr/local/lib/mash/early-ssh.sh
ConditionPathExists=!/var/lib/mash-early-ssh.done

[Service]
Type=oneshot
ExecStart=/bin/bash -lc 'mkdir -p /data/mash-logs; /usr/local/lib/mash/early-ssh.sh >> /data/mash-logs/early-ssh.log 2>&1'
ExecStartPost=/bin/bash -lc 'mkdir -p /var/lib && touch /var/lib/mash-early-ssh.done'
RemainAfterExit=no

[Install]
WantedBy=multi-user.target
"#;

pub fn run(args: &[String]) -> Result<()> {
    let root = args
        .first()
        .map(String::as_str)
        .ok_or_else(|| anyhow!("Usage: stage_02_early_ssh <target-root>"))?;
    let root_path = Path::new(root);
    if !root_path.join("etc").is_dir() {
        return Err(anyhow!("target root must contain /etc"));
    }

    let unit_dir = root_path.join("etc/systemd/system");
    let wants_dir = unit_dir.join("multi-user.target.wants");
    let lib_dir = root_path.join("usr/local/lib/mash");

    fs::create_dir_all(&unit_dir)?;
    fs::create_dir_all(&wants_dir)?;
    fs::create_dir_all(&lib_dir)?;

    let script_path = lib_dir.join("early-ssh.sh");
    fs::write(&script_path, EARLY_SSH_SCRIPT)?;
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))?;

    let service_path = unit_dir.join("mash-early-ssh.service");
    fs::write(&service_path, EARLY_SSH_SERVICE)?;

    let link = wants_dir.join("mash-early-ssh.service");
    if link.exists() {
        fs::remove_file(&link)?;
    }
    symlink("../mash-early-ssh.service", &link)?;

    println!(
        "✅ Installed mash-early-ssh.service (offline) — logs -> /data/mash-logs/early-ssh.log"
    );
    Ok(())
}
