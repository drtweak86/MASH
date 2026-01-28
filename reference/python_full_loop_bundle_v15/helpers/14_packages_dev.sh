#!/usr/bin/env bash
set -euo pipefail
echo "[*] Packages: dev/build"
dnf install -y --skip-unavailable --setopt=install_weak_deps=True \
  gcc gcc-c++ make cmake ninja-build ccache pkgconf-pkg-config \
  python3-devel python3-pip patchelf git || true
