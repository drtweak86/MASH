#!/usr/bin/env bash
set -euo pipefail
# helpers/02_early_ssh.sh
# Installs mash-early-ssh.service + early-ssh.sh into an OFFLINE target root.
# Usage (host side): 02_early_ssh.sh /mnt/target-root

ROOT="${1:-}"
if [[ -z "${ROOT}" || ! -d "${ROOT}/etc" ]]; then
  echo "Usage: $0 /path/to/mounted/target-root"
  exit 1
fi

UNIT_DIR="${ROOT}/etc/systemd/system"
WANTS_DIR="${ROOT}/etc/systemd/system/multi-user.target.wants"
LIB_DIR="${ROOT}/usr/local/lib/mash"

mkdir -p "${UNIT_DIR}" "${WANTS_DIR}" "${LIB_DIR}"

cat > "${LIB_DIR}/early-ssh.sh" <<'EOF'
#!/usr/bin/env bash
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
EOF
chmod 0755 "${LIB_DIR}/early-ssh.sh"

cat > "${UNIT_DIR}/mash-early-ssh.service" <<'EOF'
[Unit]
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
EOF

ln -sf "../mash-early-ssh.service" "${WANTS_DIR}/mash-early-ssh.service"
echo "✅ Installed mash-early-ssh.service (offline) — logs -> /data/mash-logs/early-ssh.log"
