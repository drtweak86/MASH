#!/usr/bin/env bash
# MASH Helper: Install essential fonts for terminal + desktop
# Recommended to run early for best terminal experience
set -euo pipefail

echo "========================================"
echo "  MASH Font Installation"
echo "========================================"
echo ""

# Check for internet
if ! ping -c1 -W2 github.com &>/dev/null; then
    echo "ERROR: No internet connection detected."
    echo "Please connect to the internet and try again."
    exit 1
fi

echo "Installing essential fonts for terminal and desktop..."
echo ""

# === System fonts via DNF ===
echo "[1/4] Installing Terminus fonts (clean monospace)..."
sudo dnf install -y terminus-fonts terminus-fonts-console || true

echo ""
echo "[2/4] Installing Noto Emoji fonts..."
sudo dnf install -y google-noto-emoji-fonts google-noto-emoji-color-fonts || true

echo ""
echo "[3/4] Installing additional monospace fonts..."
sudo dnf install -y \
    dejavu-sans-mono-fonts \
    liberation-mono-fonts \
    fira-code-fonts \
    || true

# === Nerd Fonts (user fonts) ===
echo ""
echo "[4/4] Installing JetBrainsMono Nerd Font (for Starship prompt)..."

FONT_DIR="${HOME}/.local/share/fonts"
mkdir -p "${FONT_DIR}"

NERD_FONT_VERSION="v3.3.0"
NERD_FONT_URL="https://github.com/ryanoasis/nerd-fonts/releases/download/${NERD_FONT_VERSION}/JetBrainsMono.zip"

cd /tmp
if wget -q --show-progress "${NERD_FONT_URL}" -O JetBrainsMono.zip; then
    unzip -o JetBrainsMono.zip -d "${FONT_DIR}/" >/dev/null 2>&1
    rm -f JetBrainsMono.zip
    echo "  JetBrainsMono Nerd Font installed to ${FONT_DIR}"
else
    echo "  WARNING: Could not download Nerd Font. Skipping."
fi

# === Refresh font cache ===
echo ""
echo "Refreshing font cache..."
fc-cache -fv >/dev/null 2>&1

echo ""
echo "========================================"
echo "  Font Installation Complete!"
echo "========================================"
echo ""
echo "Installed fonts:"
echo "  - Terminus (terminal monospace)"
echo "  - Noto Emoji (emoji support)"
echo "  - DejaVu Sans Mono"
echo "  - Liberation Mono"
echo "  - Fira Code"
echo "  - JetBrainsMono Nerd Font (Starship icons)"
echo ""
echo "To use in terminal:"
echo "  1. Open Konsole Preferences"
echo "  2. Edit your Profile -> Appearance"
echo "  3. Select 'JetBrainsMono Nerd Font' or 'Terminus'"
echo ""
echo "Nerd Font is required for Starship prompt icons!"
