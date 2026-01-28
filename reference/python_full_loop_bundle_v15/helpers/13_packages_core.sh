#!/usr/bin/env bash
set -euo pipefail
echo "[*] Packages: core"
dnf install -y --skip-unavailable --setopt=install_weak_deps=True \
  git rsync curl wget tmux neovim btrfs-progs tree htop \
  openssh-server mosh avahi nmap firewalld firewall-config fail2ban || true
