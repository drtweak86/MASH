#!/bin/bash
# MASH Dojo Installation Script
# Run this after first boot to complete the Dojo setup
#
# Usage: sudo /data/mash-staging/install_dojo.sh

set -e

echo "🥋 Welcome to MASH Dojo Setup!"
echo "================================"
echo ""

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo "❌ Please run as root: sudo $0"
    exit 1
fi

echo "📦 Updating system packages..."
dnf update -y

echo "🔧 Installing Dojo dependencies..."
dnf install -y \
    docker \
    docker-compose \
    git \
    curl \
    wget \
    zsh \
    htop \
    neofetch

echo "🐳 Enabling Docker..."
systemctl enable --now docker

echo "🔐 Adding user to docker group..."
if [ -n "$SUDO_USER" ]; then
    usermod -aG docker "$SUDO_USER"
    echo "   Added $SUDO_USER to docker group"
fi

echo ""
echo "🎉 Dojo staging complete!"
echo ""
echo "Next steps:"
echo "  1. Log out and back in (for docker group)"
echo "  2. Clone your Dojo repository"
echo "  3. Configure docker-compose.yml"
echo "  4. Run: docker-compose up -d"
echo ""
echo "🍠 Enjoy your MASH!"
