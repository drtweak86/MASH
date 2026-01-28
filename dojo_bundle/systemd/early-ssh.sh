#!/usr/bin/env bash
set -euo pipefail

LOGDIR="/data/mash-logs"
mkdir -p "$LOGDIR"
LOG="$LOGDIR/early-ssh.log"

{
  echo "=== $(date -Is) :: early-ssh start ==="
  echo "hostname: $(hostname)"

  # Ensure sshd
  if systemctl list-unit-files | grep -q '^sshd\.service'; then
    systemctl enable --now sshd || true
    echo "✅ sshd enabled"
  else
    echo "⚠️ sshd.service not present"
  fi

  # Avahi (mDNS) if installed
  if systemctl list-unit-files | grep -q '^avahi-daemon\.service'; then
    systemctl enable --now avahi-daemon || true
    echo "✅ avahi-daemon enabled (mDNS)"
  else
    echo "ℹ️ avahi-daemon not installed yet"
  fi

  # Firewalld sane defaults: allow ssh + mosh on LAN (trusted/home)
  if systemctl list-unit-files | grep -q '^firewalld\.service'; then
    systemctl enable --now firewalld || true

    # Prefer 'home' zone if it exists, else 'trusted', else default
    ZONE=""
    if firewall-cmd --get-zones | tr ' ' '\n' | grep -qx home; then ZONE="home"; fi
    if [[ -z "$ZONE" ]] && firewall-cmd --get-zones | tr ' ' '\n' | grep -qx trusted; then ZONE="trusted"; fi
    if [[ -z "$ZONE" ]]; then ZONE="$(firewall-cmd --get-default-zone)"; fi

    # Ensure ssh service
    firewall-cmd --permanent --zone="$ZONE" --add-service=ssh || true
    # Mosh uses UDP 60000-61000; service name may exist on Fedora
    if firewall-cmd --get-services | tr ' ' '\n' | grep -qx mosh; then
      firewall-cmd --permanent --zone="$ZONE" --add-service=mosh || true
    else
      firewall-cmd --permanent --zone="$ZONE" --add-port=60000-61000/udp || true
    fi
    firewall-cmd --reload || true
    echo "✅ firewalld: opened SSH + mosh on zone=$ZONE"
  else
    echo "ℹ️ firewalld not present"
  fi

  echo "=== $(date -Is) :: early-ssh done ==="
} | tee -a "$LOG"
