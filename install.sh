#!/usr/bin/env bash

# MASH Installer - One-Command Install Script
# Usage:
#   curl -sSL https://raw.githubusercontent.com/drtweak86/MASH/main/install.sh | sudo bash
#
# Optional env:
#   REPO="drtweak86/MASH"        # override repo if you fork
#   MASH_RELEASE="v1.0.0"        # pin a tag (else latest)
#   GITHUB_TOKEN="ghp_..."       # optional (rate limits/private)
#
# NOTE: We only touch this header section. Everything below stays your way. 😆🦀🥋

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
REPO_DEFAULT="drtweak86/MASH"
REPO="${REPO:-$REPO_DEFAULT}"
INSTALL_DIR="/usr/local/bin"
TEMP_DIR="/tmp/mash-install-$$"

# Functions
log_info()    { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
log_error()   { echo -e "${RED}[ERROR]${NC} $1"; }

check_root() {
    if [ "$EUID" -ne 0 ]; then
        log_error "This script must be run as root"
        log_info "Please run: curl -sSL <url> | sudo bash"
        exit 1
    fi
}

need_cmd() {
    command -v "$1" >/dev/null 2>&1 || { log_error "Missing required command: $1"; exit 1; }
}

detect_architecture() {
    local arch
    arch="$(uname -m)"
    case "$arch" in
        aarch64|arm64) echo "aarch64-unknown-linux-gnu" ;;
        x86_64|amd64)  echo "x86_64-unknown-linux-gnu" ;;
        *)
            log_error "Unsupported architecture: $arch"
            log_info "Supported: aarch64, x86_64"
            exit 1
            ;;
    esac
}

gh_api() {
    # Accept either "/repos/OWNER/REPO/..." or full URL.
    local url="$1"
    if [[ "$url" == /* ]]; then
        url="https://api.github.com${url}"
    fi

    if [[ -n "${GITHUB_TOKEN:-}" ]]; then
        curl -sSL -H "Authorization: token ${GITHUB_TOKEN}" -H "Accept: application/vnd.github+json" "$url"
    else
        curl -sSL -H "Accept: application/vnd.github+json" "$url"
    fi
}

get_latest_version() {
    if [[ -n "${MASH_RELEASE:-}" ]]; then
        echo "${MASH_RELEASE}"
        return 0
    fi

    local json version
    json="$(gh_api "/repos/${REPO}/releases/latest")"
    version="$(echo "$json" | grep -E '"tag_name"\s*:' | head -n1 | sed -E 's/.*"([^"]+)".*/\1/' || true)"

    if [[ -z "${version:-}" ]]; then
        log_error "Failed to fetch latest version"
        exit 1
    fi

    echo "$version"
}

# Finds the correct tarball asset URL from the GitHub release,
# so we don't hardcode filenames (your assets are like mash-installer-v1.0.0.tar.gz).
get_release_tarball_url() {
    local version="$1"
    local api_url="https://api.github.com/repos/${REPO}/releases/tags/${version}"
    local json
    json="$(gh_api "$api_url")"

    # Prefer mash-installer-*.tar.gz
    local url
    url="$(echo "$json" \
      | grep -E '"name"\s*:|"browser_download_url"\s*:' \
      | sed -E 's/.*"([^"]+)".*/\1/' \
      | paste - - \
      | awk -F'|' '
          BEGIN{best=""}
          {
            name=$1; u=$2;
            if (name ~ /^mash-installer-.*\.tar\.gz$/) { print u; exit }
            if (best=="" && name ~ /\.tar\.gz$/) best=u
          }
          END{ if (best!="") print best }
      ' | head -n1)"

    if [[ -z "${url:-}" ]]; then
        log_error "No .tar.gz asset found for release ${version}"
        log_info "Check the release assets on GitHub for ${REPO} / ${version}"
        exit 1
    fi

    echo "$url"
}

download_and_install() {
    local version="$1"
    local arch="$2"

    log_info "Resolving release asset for $version and arch $arch..."
    local download_url
    download_url=$(get_asset_url "$version" "$arch")
    log_info "Selected asset: $download_url"

    mkdir -p "$TEMP_DIR"
    cd "$TEMP_DIR"

    local filename
    filename=$(basename "$download_url")

    log_info "Downloading $filename..."
    if ! curl -fsSL -o "$filename" "$download_url"; then
        log_error "Failed to download asset: $download_url"
        exit 1
    fi

    log_info "Extracting archive..."
    case "$filename" in
        *.tar.gz|*.tgz)
            tar -xzf "$filename"
            ;;
        *.zip)
            if command -v unzip >/dev/null 2>&1; then
                unzip -q "$filename"
            else
                log_error "zip archive detected but 'unzip' not installed"
                exit 1
            fi
            ;;
        *.tar)
            tar -xf "$filename"
            ;;
        *)
            # try tar.gz extraction as fallback
            if tar -tzf "$filename" >/dev/null 2>&1; then
                tar -xzf "$filename"
            else
                # maybe it's a single binary; install directly
                if file "$filename" | grep -iq 'ELF'; then
                    log_info "Detected single ELF binary, installing directly..."
                    cp "$filename" "$INSTALL_DIR/MASH"
                    chmod +x "$INSTALL_DIR/MASH"
                    log_success "CLI installed to $INSTALL_DIR/MASH"
                    return 0
                fi
                log_error "Unknown archive/asset format: $filename"
                exit 1
            fi
            ;;
    esac

    # Locate CLI binary
    local cli_binary
    cli_binary=$(find . -type f -name 'MASH' -perm /111 -print -quit 2>/dev/null || true)
    if [ -z "$cli_binary" ]; then
        # try arch-specific patterns and generic names
        cli_binary=$(find . -type f \( -iname "*mash*" -o -iname "mash-*" \) -perm /111 -print -quit 2>/dev/null || true)
    fi

    if [ -n "$cli_binary" ]; then
        log_info "Installing CLI binary from $cli_binary..."
        cp "$cli_binary" "$INSTALL_DIR/MASH"
        chmod +x "$INSTALL_DIR/MASH"
        log_success "CLI installed to $INSTALL_DIR/MASH"
    else
        log_error "CLI binary not found in extracted files"
        log_info "Extracted files (top-level):"
        find . -maxdepth 3 -type f -print
        exit 1
    fi

    # Optional Qt GUI
    local qt_binary
    qt_binary=$(find . -type f -iname 'MASH-qt' -perm /111 -print -quit 2>/dev/null || true)
    if [ -n "$qt_binary" ]; then
        log_info "Installing Qt GUI from $qt_binary..."
        cp "$qt_binary" "$INSTALL_DIR/MASH-qt"
        chmod +x "$INSTALL_DIR/MASH-qt"
        log_success "Qt GUI installed to $INSTALL_DIR/MASH-qt"
    else
        log_warning "Qt GUI not found in archive (optional component)"
    fi

    # Cleanup
    cd /
    rm -rf "$TEMP_DIR"
}

create_desktop_entry() {
    local desktop_file="/usr/share/applications/MASH.desktop"

    if [ -f "$INSTALL_DIR/MASH-qt" ]; then
        log_info "Creating desktop entry..."

        cat > "$desktop_file" <<'EOF'
[Desktop Entry]
Version=1.0
Type=Application
Name=MASH Installer
Comment=Install Fedora KDE on Raspberry Pi 4
Exec=pkexec /usr/local/bin/MASH-qt
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
  MASH preflight

  # Install (with confirmation)
  sudo MASH flash \\
    --image /path/to/fedora-kde.raw \\
    --disk /dev/sdX \\
    --uefi-dir /path/to/uefi \\
    --auto-unmount \\
    --yes-i-know

  # Dry run (test without changes)
  sudo MASH flash \\
    --image /path/to/fedora-kde.raw \\
    --disk /dev/sdX \\
    --uefi-dir /path/to/uefi \\
    --dry-run

${BLUE}GUI Usage:${NC}
  sudo MASH-qt

${BLUE}Help:${NC}
  MASH --help

${YELLOW}⚠️  WARNING:${NC}
  This installer will COMPLETELY ERASE the target disk!
  Always double-check your disk selection!

${BLUE}Documentation:${NC}
  https://github.com/$REPO

${GREEN}Happy Installing! 🎉${NC}

EOF
}

main() {
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

    check_root

    local arch
    arch=$(detect_architecture)
    log_info "Detected architecture: $arch"

    local version
    version=$(get_latest_version)
    log_info "Latest version: $version"

    download_and_install "$version" "$arch"
    create_desktop_entry

    log_success "Installation complete!"
    show_usage
}

main "$@"
