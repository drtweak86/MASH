#!/usr/bin/env bash

# MASH Installer - One-Command Install Script
# Usage: curl -fsSL https://raw.githubusercontent.com/drtweak86/MASH/main/install.sh | bash

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
REPO="drtweak86/MASH"  # Update this!
INSTALL_DIR="/usr/local/bin"
TEMP_DIR="/tmp/mash-install-$$"

# Functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

check_root() {
    if [ "$EUID" -ne 0 ]; then
        log_error "This script must be run as root"
        log_info "Please run: curl -L <url> | sudo bash"
        exit 1
    fi
}

detect_architecture() {
    local arch=$(uname -m)
    case $arch in
        aarch64|arm64)
            echo "aarch64-unknown-linux-gnu"
            ;;
        x86_64|amd64)
            echo "x86_64-unknown-linux-gnu"
            ;;
        *)
            log_error "Unsupported architecture: $arch"
            log_info "Supported: aarch64, x86_64"
            exit 1
            ;;
    esac
}

get_latest_version() {
    local version=$(curl -L "https://api.github.com/repos/$REPO/releases/latest" | \
                    grep '"tag_name":' | \
                    sed -E 's/.*"([^"]+)".*/\1/')
    
    if [ -z "$version" ]; then
        log_error "Failed to fetch latest version"
        exit 1
    fi
    
    echo "$version"
}

download_and_install() {
    local version=$1
    local arch=$2
    local download_url="https://github.com/$REPO/releases/download/$version/MASH-$version.tar.gz"
    
    log_info "Downloading MASH Installer $version for $arch..."
    
    mkdir -p "$TEMP_DIR"
    cd "$TEMP_DIR"
    
    if ! curl -fsSL -o MASH.tar.gz "$download_url"; then
        log_error "Failed to download installer"
        exit 1
    fi
    
    log_info "Extracting archive..."
    tar -xzf MASH.tar.gz
    
    # Install CLI binary
    local cli_binary="MASH-$arch/MASH-$arch"
    if [ -f "$cli_binary" ]; then
        log_info "Installing CLI binary..."
        cp "$cli_binary" "$INSTALL_DIR/MASH"
        chmod +x "$INSTALL_DIR/MASH"
        log_success "CLI installed to $INSTALL_DIR/MASH"
    else
        log_error "CLI binary not found in archive"
        exit 1
    fi
    
    # Install Qt GUI (optional)
    if [ -f "MASH-qt/MASH-qt" ]; then
        log_info "Installing Qt GUI..."
        cp "MASH-qt/MASH-qt" "$INSTALL_DIR/MASH-qt"
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
    
    local arch=$(detect_architecture)
    log_info "Detected architecture: $arch"
    
    local version=$(get_latest_version)
    log_info "Latest version: $version"
    
    download_and_install "$version" "$arch"
    create_desktop_entry
    
    log_success "Installation complete!"
    show_usage
}

main "$@"
