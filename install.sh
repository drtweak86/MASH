#!/usr/bin/env bash
set -euo pipefail

# Defaults for set -u safety (populated in main)
ARCH=""
VERSION=""


# ==========================================================
# 🦀 MASH Installer - One-Command Install Script (Pi-first)
# Repo: drtweak86/MASH
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/drtweak86/MASH/main/install.sh | sudo bash
#
# Optional env:
#   MASH_RELEASE=v1.0.8          # pin a specific tag (default: latest)
#   REPO=drtweak86/MASH          # override repo
#   INSTALL_DIR=/usr/local/bin   # override install location
# ==========================================================

# Colors (safe if stdout is piped)
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

REPO_DEFAULT="drtweak86/MASH"
REPO="${REPO:-$REPO_DEFAULT}"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"
TEMP_DIR="$(mktemp -d /tmp/mash-install-XXXXXX)"

# Global arch (used by install_binaries)
ARCH=""

log_info()    { echo -e "${BLUE}[INFO]${NC} $*" >&2; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $*" >&2; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $*" >&2; }
log_error()   { echo -e "${RED}[ERROR]${NC} $*" >&2; }

cleanup() { rm -rf "$TEMP_DIR"; }
trap cleanup EXIT

need() { command -v "$1" >/dev/null 2>&1 || { log_error "missing dependency: $1"; exit 1; }; }

check_root() {
  if [[ "$(id -u)" -ne 0 ]]; then
    log_error "This script must be run as root."
    log_info "Try: curl -fsSL https://raw.githubusercontent.com/drtweak86/MASH/main/install.sh | sudo bash"
    exit 1
  fi
}

detect_arch() {
  local arch
  arch="$(uname -m)"
  case "$ARCH" in
    aarch64|arm64) echo "aarch64-unknown-linux-gnu" ;;
    x86_64|amd64)  echo "x86_64-unknown-linux-gnu" ;;
    *) log_error "Unsupported architecture: $ARCH (supported: aarch64, x86_64)"; exit 1 ;;
  esac
}

# Lightweight GitHub API helper
gh_api() {
  local url="$1"
  # allow callers to pass "/repos/..../..." or full https://api.github.com/...
  if [[ "$url" == /* ]]; then
    url="https://api.github.com${url}"
  fi
  curl -sSL -H "Accept: application/vnd.github.v3+json" "$url"
}

get_latest_version() {
  if [[ -n "${MASH_RELEASE:-}" ]]; then
    echo "${MASH_RELEASE}"
    return 0
  fi
  local json version
  json="$(gh_api "/repos/${REPO}/releases/latest")"
  version="$(printf '%s' "$json" | grep -E '"tag_name"\s*:' | head -n1 | sed -E 's/.*"tag_name"\s*:\s*"([^"]+)".*/\1/')"
  if [[ -z "${version:-}" ]]; then
    log_error "Failed to fetch latest release tag from GitHub API."
    exit 1
  fi
  echo "$version"
}

# Try hard-coded URL patterns first (fast, no JSON parsing).
try_known_urls() {
  local version="$1"
  local arch="$2"
  local arch_short
  case "$ARCH" in
    aarch64-unknown-linux-gnu) arch_short="aarch64" ;;
    x86_64-unknown-linux-gnu)  arch_short="x86_64" ;;
    *) arch_short="$ARCH" ;;
  esac

  local base="https://github.com/${REPO}/releases/download/${version}"
  local candidates=(
    # Your current battle-tested asset naming:
    "mash-installer-${version}.tar.gz"
    "mash-installer-${version}.tgz"
    # Older/alternate patterns (kept for resilience)
    "mash-installer-${version}-linux-${arch_short}.tar.gz"
    "mash-installer-${version}-linux-${arch_short}.tgz"
  )

  local c
  for c in "${candidates[@]}"; do
    if curl -fsSL -o "${TEMP_DIR}/${c}" "${base}/${c}" >/dev/null 2>&1; then
      echo "${TEMP_DIR}/${c}"
      return 0
    fi
  done
  return 1
}

# Auto-discover the correct asset URL from release JSON (fallback).
get_asset_url_from_api() {
  local version="$1"
  local arch="$2"
  local api_url="https://api.github.com/repos/${REPO}/releases/tags/${version}"
  log_info "Querying release metadata for ${version}..."
  local json
  json="$(gh_api "$api_url")"

  local assets=()
  local last_name=""
  while IFS= read -r line; do
    if echo "$line" | grep -q '"name"'; then
      last_name="$(echo "$line" | sed -E 's/.*"name"\s*:\s*"([^"]+)".*/\1/')"
    elif echo "$line" | grep -q '"browser_download_url"'; then
      local u
      u="$(echo "$line" | sed -E 's/.*"browser_download_url"\s*:\s*"([^"]+)".*/\1/')"
      if [[ -n "$last_name" && -n "$u" ]]; then
        assets+=("${last_name}|${u}")
        last_name=""
      fi
    fi
  done < <(printf '%s\n' "$json" | grep -E '"name"\s*:|"browser_download_url"\s*:')

  if [[ "${#assets[@]}" -eq 0 ]]; then
    log_error "No assets found for release ${version}."
    exit 1
  fi

  # Prefer exact match to your main naming style.
  local name url
  for pair in "${assets[@]}"; do
    name="${pair%%|*}"
    url="${pair##*|}"
    if [[ "$name" == "mash-installer-${version}.tar.gz" || "$name" == "mash-installer-${version}.tgz" ]]; then
      echo "$url"
      return 0
    fi
  done

  # Otherwise: pick the first tar.gz/tgz that contains "mash-installer"
  for pair in "${assets[@]}"; do
    name="${pair%%|*}"
    url="${pair##*|}"
    if echo "$name" | grep -Eiq '^mash-installer.*\.(tar\.gz|tgz)$'; then
      echo "$url"
      return 0
    fi
  done

  log_error "Could not pick a suitable asset from release ${version}. Assets available:"
  for pair in "${assets[@]}"; do
    name="${pair%%|*}"
    url="${pair##*|}"
    echo "  - $name -> $url" >&2
  done
  exit 1
}

download_release_asset() {
  local version="$1"
  local arch="$2"

  # 1) Try known URL patterns.
  local local_path
  if local_path="$(try_known_urls "$version" "$ARCH")"; then
    echo "$local_path"
    return 0
  fi

  # 2) Fallback to API discovery.
  local url
  url="$(get_asset_url_from_api "$version" "$ARCH")"
  log_info "Selected asset: $url"
  local filename
  filename="$(basename "$url")"
  local_path="${TEMP_DIR}/${filename}"
  if ! curl -fsSL -o "$local_path" "$url"; then
    log_error "Failed to download asset: $url"
    exit 1
  fi
  echo "$local_path"
}

extract_archive() {
  local archive="$1"
  case "$archive" in
    *.tar.gz|*.tgz) tar -xzf "$archive" -C "$TEMP_DIR" ;;
    *.tar)         tar -xf "$archive" -C "$TEMP_DIR" ;;
    *.zip)
      need unzip
      unzip -q "$archive" -d "$TEMP_DIR"
      ;;
    *)
      log_error "Unknown archive type: $archive"
      exit 1
      ;;
  esac
}

install_binaries() {
    log_info "Installing..."

    # We extract into $TEMP_DIR and run from there
    # Release layout is typically:
    #   mash-installer-${ARCH}/mash-installer-${ARCH}
    local cli=""
    cli="$(find . -maxdepth 6 -type f -name "mash-installer-${ARCH}" -perm -111 -print -quit 2>/dev/null || true)"

    # Fallback: anything executable named mash-installer* (covers naming drift)
    if [[ -z "${cli:-}" ]]; then
        cli="$(find . -maxdepth 6 -type f -name "mash-installer*" -perm -111 -print -quit 2>/dev/null || true)"
    fi

    if [[ -z "${cli:-}" ]]; then
        log_error "Could not find 'mash-installer' binary inside the release archive."
        log_info "Archive contents (top-level):"
        find . -maxdepth 3 -type f -print
        exit 1
    fi

    install -m 0755 "$cli" "$INSTALL_DIR/mash-installer"
    log_success "Installed: $INSTALL_DIR/mash-installer"

    # Qt is removed in v1.1.x+; no GUI install attempt.
}

create_desktop_entry() {
    local desktop_file="/usr/share/applications/mash-installer.desktop"

    if [ -f "${INSTALL_DIR}/mash-installer-qt" ]; then
        log_info "Creating desktop entry..."

        cat > "$desktop_file" <<'EOF'
[Desktop Entry]
Version=1.0
Type=Application
Name=MASH Installer
Comment=Install Fedora KDE on Raspberry Pi 4
Exec=pkexec /usr/local/bin/mash-installer-qt
Icon=drive-harddisk
Terminal=false
Categories=System;Settings;
Keywords=installer;fedora;raspberry;pi;
EOF

        chmod 644 "$desktop_file"
        log_success "Desktop entry created"
    fi
}

show_usage() {
    cat <<EOF

${GREEN}╔════════════════════════════════════════════════════════════╗
║                                                            ║
║              🚀 MASH Installer Ready! 🚀                  ║
║                                                            ║
╚════════════════════════════════════════════════════════════╝${NC}

${BLUE}CLI Usage:${NC}
  # Check system requirements
  mash-installer preflight

  # Install (with confirmation)
  sudo mash-installer flash \
    --image /path/to/fedora-kde.raw \
    --disk /dev/sdX \
    --uefi-dir /path/to/uefi \
    --auto-unmount \
    --yes-i-know

  # Dry run (test without changes)
  sudo mash-installer flash \
    --image /path/to/fedora-kde.raw \
    --disk /dev/sdX \
    --uefi-dir /path/to/uefi \
    --dry-run
  mash-installer --help

${YELLOW}⚠️  WARNING:${NC}
  This installer will COMPLETELY ERASE the target disk!
  Always double-check your disk selection!

${BLUE}Documentation:${NC}
  https://github.com/$REPO

${GREEN}Happy Installing! 🎉${NC}

EOF
}

main() {
  need curl
  need tar
  need uname
  need install

  check_root

  echo -e "${GREEN}"
  cat <<'EOF'
╔════════════════════════════════════════════════════════════╗
║                                                            ║
║              🦀 MASH Installer Setup 🦀                   ║
║         Fedora KDE for Raspberry Pi 4 (UEFI)              ║
║                                                            ║
╚════════════════════════════════════════════════════════════╝
EOF
  echo -e "${NC}"

local ARCH version archive
    ARCH="$(detect_arch)"
  log_info "Detected architecture: ${ARCH}"

  version="$(get_latest_version)"
  log_info "Latest version: ${version}"

  log_info "Downloading MASH Installer ${version} for ${ARCH}..."
  archive="$(download_release_asset "$version" "$ARCH")"

  log_info "Extracting..."
  extract_archive "$ARCHive"

  log_info "Installing..."
  install_binaries
  create_desktop_entry

  log_success "Installation complete!"
  show_usage
}

main "$@"
