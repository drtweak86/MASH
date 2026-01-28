#!/usr/bin/env bash
set -euo pipefail
echo "[*] Locale: en_GB + GB keymap"
dnf install -y langpacks-en_GB || true
localectl set-locale LANG=en_GB.UTF-8 || true
localectl set-x11-keymap gb || true
