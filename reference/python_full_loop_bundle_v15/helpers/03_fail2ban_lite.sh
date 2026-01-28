#!/usr/bin/env bash
set -euo pipefail
echo "🛡️  fail2ban-lite: enabling sshd jail (LAN safe)"

sudo dnf install -y fail2ban || true

sudo install -d -m 0755 /etc/fail2ban
sudo tee /etc/fail2ban/jail.d/mash-local.conf >/dev/null <<'EOF'
[DEFAULT]
# Don't ban RFC1918 LAN ranges (keeps Batcave safe)
ignoreip = 127.0.0.1/8 ::1 10.0.0.0/8 172.16.0.0/12 192.168.0.0/16
bantime  = 1h
findtime = 10m
maxretry = 6

[sshd]
enabled = true
EOF

sudo systemctl enable --now fail2ban
sudo systemctl status fail2ban --no-pager || true
echo "✅ fail2ban running. LAN ignored. 🛡️"
