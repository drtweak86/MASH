#!/usr/bin/env bash
set -euo pipefail

# MASH Installer - One-Command Install Script
# Usage:
#   curl -sL https://raw.githubusercontent.com/drtweak86/MASH/main/install.sh | sudo bash
# Optional:
#   export GITHUB_TOKEN=ghp_...   # increases API rate limits / private repos
#   export MASH_RELEASE=v1.0.0    # pin specific tag (otherwise latest)

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
REPO="drtweak86/MASH"
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
        log_info "Please run: curl -sL <url> | sudo bash"
        exit 1
    fi
}

detect_architecture() {
    local arch
    arch=$(uname -m)
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

# GitHub API helper (supports "/repos/.." shorthand OR full URL)
gh_api() {
    local url="$1"
    if [[ "$url" != https://api.github.com/* ]]; then
        url="https://api.github.com${url}"
    fi

    if [[ -n "${GITHUB_TOKEN:-}" ]]; then
        curl -sSL \
          -H "Authorization: token ${GITHUB_TOKEN}" \
          -H "Accept: application/vnd.github.v3+json" \
          "$url"
    else
        curl -sSL \
          -H "Accept: application/vnd.github.v3+json" \
          "$url"
    fi
}

# Get latest release tag (or use MASH_RELEASE)
get_latest_version() {
    if [[ -n "${MASH_RELEASE:-}" ]]; then
        echo "${MASH_RELEASE}"
        return
    fi

    local json version
    json="$(gh_api "/repos/$REPO/releases/latest")"
    version="$(echo "$json" | grep -E '"tag_name"\s*:' | head -n1 | sed -E 's/.*"([^"]+)".*/\1/' || true)"

    if [[ -z "$version" ]]; then
        log_error "Failed to fetch latest version from GitHub API"
        exit 1
    fi

    echo "$version"
}

# Return a list of tokens used to match asset names for a given arch
arch_tokens() {
    local arch="$1"
    case "$arch" in
        aarch64-unknown-linux-gnu) echo "aarch64 arm64 linux-aarch64 linux-arm64 armv8" ;;
        x86_64-unknown-linux-gnu)  echo "x86_64 amd64 linux-x86_64 linux-amd64" ;;
        *) echo "" ;;
    esac
}

# Find best matching asset URL for tag + arch.
# Supports your current naming like: mash-installer-v1.0.0.tar.gz
get_asset_url() {
    local version="$1"
    local arch="$2"
    local api_url="https://api.github.com/repos/$REPO/releases/tags/$version"

    log_info "Querying release metadata for $version..." >&2
    local json
    json="$(gh_api "$api_url")"

# Build "name|url" pairs
assets=()
last_name=""
while IFS= read -r line; do
    case "$line" in
        *'"name"'*)
            last_name="${line#*\"name\":}"
            last_name="${last_name#*\"}"
            last_name="${last_name%%\"*}"
            ;;
        *'"browser_download_url"'*)
            url="${line#*\"browser_download_url\":}"
            url="${url#*\"}"
            url="${url%%\"*}"
            if [[ -n "$last_name" && -n "$url" ]]; then
                assets+=("$last_name|$url")
                last_name=""
            fi
            ;;
    esac
done <<<"$json"

    if [[ "${#assets[@]}" -eq 0 ]]; then
        log_error "No assets found for release $version"
        exit 1
    fi

    # 1) Exact match: mash-installer-<version>.tar.gz (your current style)
    local name url
    for pair in "${assets[@]}"; do
        name="${pair%%|*}"
        url="${pair##*|}"
        if [[ "$name" == "mash-installer-${version}.tar.gz" || "$name" == "mash-installer-${version}.tgz" ]]; then
            echo "$url"
            return 0
        fi
    done

    # 2) Otherwise try arch tokens
    local tokens prefer_ext_regex
    tokens="$(arch_tokens "$arch")"
    prefer_ext_regex='\.(tar\.gz|tgz|zip|tar)$'

    local token
    for token in $tokens; do
        for pair in "${assets[@]}"; do
            name="${pair%%|*}"
            url="${pair##*|}"
            if echo "$name" | grep -Eiq "$prefer_ext_regex" && echo "$name" | grep -iq "$token"; then
                echo "$url"
                return 0
            fi
        done
    done

    # 3) Fallback: any mash-installer tar/zip
    for pair in "${assets[@]}"; do
        name="${pair%%|*}"
        url="${pair##*|}"
        if echo "$name" | grep -Eiq '^mash-installer.*\.(tar\.gz|tgz|zip|tar)$'; then
            echo "$url"
            return 0
        fi
    done

    log_error "No suitable release asset could be matched. Available assets:"
    for pair in "${assets[@]}"; do
        name="${pair%%|*}"
        url="${pair##*|}"
        echo "  - $name -> $url"
    done
    exit 1
}

download_and_install() {
    local version="$1"
    local arch="$2"

    log_info "Resolving release asset for $version and arch $arch..."
    local download_url
    download_url="$(get_asset_url "$version" "$arch")"
    log_info "Selected asset: $download_url"

    mkdir -p "$TEMP_DIR"
    cd "$TEMP_DIR"

    local filename
    filename="$(basename "$download_url")"

    log_info "Downloading $filename..."
    if ! curl -fL --retry 3 --retry-delay 1 -o "$filename" "$download_url"; then
        log_error "Failed to download asset: $download_url"
        exit 1
    fi

    log_info "Extracting archive..."
    case "$filename" in
        *.tar.gz|*.tgz) tar -xzf "$filename" ;;
        *.zip)
            if command -v unzip >/dev/null 2>&1; then
                unzip -q "$filename"
            else
                log_error "zip archive detected but 'unzip' not installed"
                exit 1
            fi
            ;;
        *.tar) tar -xf "$filename" ;;
        *)
            log_error "Unknown archive format: $filename"
            exit 1
            ;;
    esac

    # Install CLI binary
    local cli_binary
    if [[ -d "mash-installer-${arch}" ]]; then
        cli_binary="$(find "mash-installer-${arch}" -type f -executable -print -quit 2>/dev/null || true)"
    fi
    if [[ -z "$cli_binary" ]]; then
        cli_binary="$(find . -type f -name "mash-installer*" -executable -print -quit 2>/dev/null || true)"
    fi
    if [[ -n "$cli_binary" ]]; then
        log_info "Installing CLI from $cli_binary..."
        cp "$cli_binary" "$INSTALL_DIR/mash-installer"
        chmod +x "$INSTALL_DIR/mash-installer"
        log_success "CLI installed to $INSTALL_DIR/mash-installer"
    else
        log_error "CLI binary not found in extracted files"
        log_info "Extracted files (top-level):"
        find . -maxdepth 3 -type f -print
        exit 1
    fi

    # Optional Qt GUI: looks for an executable named "MASH-qt"
    local qt_binary
    qt_binary="$(find . -type f -name 'MASH-qt' -perm /111 -print -quit 2>/dev/null || true)"
    if [[ -n "$qt_binary" ]]; then
        log_info "Installing Qt GUI from $qt_binary..."
        cp "$qt_binary" "$INSTALL_DIR/MASH-qt"
        chmod +x "$INSTALL_DIR/MASH-qt"
        log_success "Qt GUI installed to $INSTALL_DIR/MASH-qt"
    else
        log_warning "Qt GUI not found in archive (optional component)"
    fi

    cd /
    rm -rf "$TEMP_DIR"
}

create_desktop_entry() { local desktop_file="/usr/share/applications/MASH.desktop"

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
