#!/usr/bin/env bash
set -euo pipefail
# set -u safety (must exist before any use)
ARCH="${ARCH:-}"
VERSION="${VERSION:-}"




# ==========================================================
# 🍠 MASH Installer - One-Command Install Script (Pi-first)
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
CYAN='\033[0;36m'
NC='\033[0m'

REPO_DEFAULT="drtweak86/MASH"
REPO="${REPO:-$REPO_DEFAULT}"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"
TEMP_DIR="$(mktemp -d /tmp/mash-install-XXXXXX)"

# Global arch (used by install_binaries)

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
  local m
  m="$(uname -m)"
  case "$m" in
    aarch64|arm64) echo "aarch64-unknown-linux-gnu" ;;
    x86_64|amd64)  echo "x86_64-unknown-linux-gnu" ;;
    *)
      log_error "Unsupported architecture: $m (supported: aarch64, x86_64)"
      exit 1
      ;;
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
    # New naming convention (v1.2+)
    "mash-${version}.tar.gz"
    "mash-${version}.tgz"
    # Legacy naming convention
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
  local arch="$2"  # currently unused (archive is not arch-suffixed)
  local api_url="https://api.github.com/repos/$REPO/releases/tags/$version"

  log_info "Querying release metadata for $version..." >&2
  local json
  json="$(gh_api "$api_url")"

  # Extract download URLs (one per line)
  local urls
  urls="$(echo "$json" | grep -oE '"browser_download_url"\s*:\s*"[^"]+"' | sed -E 's/.*"([^"]+)".*/\1/')"

  # Prefer the new naming, then fall back to legacy
  local u
  u="$(echo "$urls" | grep -E "mash-${version}\.tar\.gz$" | head -n1 || true)"
  if [[ -z "${u:-}" ]]; then
    u="$(echo "$urls" | grep -E "mash-installer-${version}\.tar\.gz$" | head -n1 || true)"
  fi
  if [[ -z "${u:-}" ]]; then
    u="$(echo "$urls" | grep -E "mash-.*${version}.*\.tar\.gz$" | grep -v '\.sha256$' | head -n1 || true)"
  fi
  if [[ -z "${u:-}" ]]; then
    u="$(echo "$urls" | grep -E "mash.*\.tar\.gz$" | grep -v '\.sha256$' | head -n1 || true)"
  fi

  if [[ -z "${u:-}" ]]; then
    log_error "No suitable mash .tar.gz asset found for ${version}" >&2
    log_info "Available assets:" >&2
    echo "$urls" >&2
    exit 1
  fi

  echo "$u"
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
    log_info "🔧 Installing..."

    # Release layout is typically:
    #   mash-${ARCH}/mash-${ARCH} (new)
    #   mash-installer-${ARCH}/mash-installer-${ARCH} (legacy)
    local cli=""

    # Try new naming first (search in TEMP_DIR where archive was extracted)
    cli="$(find "$TEMP_DIR" -maxdepth 6 -type f -name "mash-${ARCH}" -print -quit 2>/dev/null || true)"

    # Fall back to any file named mash (not mash-installer)
    if [[ -z "${cli:-}" ]]; then
        cli="$(find "$TEMP_DIR" -maxdepth 6 -type f -name "mash" -print -quit 2>/dev/null || true)"
    fi

    # Fall back to legacy naming
    if [[ -z "${cli:-}" ]]; then
        cli="$(find "$TEMP_DIR" -maxdepth 6 -type f -name "mash-installer-${ARCH}" -print -quit 2>/dev/null || true)"
    fi

    # Fallback: anything named mash-installer* (covers naming drift)
    if [[ -z "${cli:-}" ]]; then
        cli="$(find "$TEMP_DIR" -maxdepth 6 -type f -name "mash-installer*" -print -quit 2>/dev/null || true)"
    fi

    if [[ -z "${cli:-}" ]]; then
        log_error "Could not find 'mash' binary inside the release archive."
        log_info "Archive contents:"
        find "$TEMP_DIR" -maxdepth 3 -type f -print
        exit 1
    fi

    chmod +x "$cli"
    install -m 0755 "$cli" "$INSTALL_DIR/mash"
    log_success "✅ Installed: $INSTALL_DIR/mash"
}

create_desktop_entry() {
    local desktop_file="/usr/share/applications/mash.desktop"

    if [ -f "${INSTALL_DIR}/mash-qt" ]; then
        log_info "Creating desktop entry..."

        cat > "$desktop_file" <<'EOF'
[Desktop Entry]
Version=1.0
Type=Application
Name=MASH
Comment=🍠 Install Fedora KDE on Raspberry Pi 4
Exec=pkexec /usr/local/bin/mash-qt
Icon=drive-harddisk
Terminal=false
Categories=System;Settings;
Keywords=installer;fedora;raspberry;pi;mash;
EOF

        chmod 644 "$desktop_file"
        log_success "Desktop entry created"
    fi
}

show_usage() {
    cat <<EOF

${GREEN}╔════════════════════════════════════════════════════════════╗
║                                                            ║
║             🍠 MASH Ready to Roll! 🍠                     ║
║                                                            ║
╚════════════════════════════════════════════════════════════╝${NC}

${CYAN}🎉 Quick Start (Recommended):${NC}
  ${GREEN}sudo mash${NC}

  This launches the friendly TUI wizard that guides you through
  the entire installation process step by step!

${BLUE}📋 CLI Usage (for scripting):${NC}
  # Check system requirements
  ${YELLOW}mash preflight${NC}

  # Install via CLI (advanced users)
  ${YELLOW}sudo mash flash \\
    --image /path/to/fedora-kde.raw \\
    --disk /dev/sdX \\
    --uefi-dir /path/to/uefi \\
    --auto-unmount \\
    --yes-i-know${NC}

  # Dry run (test without changes)
  ${YELLOW}sudo mash --dry-run flash \\
    --image /path/to/fedora-kde.raw \\
    --disk /dev/sdX \\
    --uefi-dir /path/to/uefi${NC}

  # Get help
  ${YELLOW}mash --help${NC}

${RED}⚠️  WARNING:${NC}
  This installer will COMPLETELY ERASE the target disk!
  Always double-check your disk selection!

${BLUE}📚 Documentation:${NC}
  https://github.com/$REPO

${GREEN}🎉 Happy Installing! Enjoy your MASH! 🍠${NC}

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
║              🍠 MASH Installer Setup 🍠                   ║
║         Fedora KDE for Raspberry Pi 4 (UEFI)              ║
║                                                            ║
╚════════════════════════════════════════════════════════════╝
EOF
  echo -e "${NC}"

local ARCH version archive
    ARCH="$(detect_arch)"
  log_info "🔍 Detected architecture: ${ARCH}"

  version="$(get_latest_version)"
  log_info "📦 Latest version: ${version}"

  log_info "⬇️  Downloading MASH ${version} for ${ARCH}..."
  archive="$(download_release_asset "$version" "$ARCH")"

  log_info "📂 Extracting..."
  extract_archive "$archive"

  log_info "🔧 Installing..."
  install_binaries
  create_desktop_entry

  log_success "🎉 Installation complete!"
  show_usage
}

main "$@"
