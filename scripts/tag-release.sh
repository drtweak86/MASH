#!/usr/bin/env bash
# Simple tag-based release for MASH
# Just create a tag, push it, and GitHub Actions does the rest!

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Check for uncommitted changes
if ! git diff-index --quiet HEAD --; then
    log_error "You have uncommitted changes. Commit or stash them first."
    git status --short
    exit 1
fi

# Get version argument or prompt
if [ -n "$1" ]; then
    VERSION="$1"
else
    # Show existing tags
    echo -e "${BLUE}Existing tags:${NC}"
    git tag -l | tail -5
    echo ""
    read -p "Enter new version (e.g., 1.0.6 or v1.0.6): " VERSION
fi

# Normalize version (add 'v' prefix if missing)
if [[ ! "$VERSION" =~ ^v ]]; then
    VERSION="v$VERSION"
fi

# Check if tag already exists
if git rev-parse "$VERSION" >/dev/null 2>&1; then
    log_error "Tag $VERSION already exists!"
    exit 1
fi

echo ""
log_info "Creating release: $VERSION"
echo ""

# Confirm
read -p "$(echo -e ${YELLOW}Create tag and trigger release? [y/N]${NC} )" -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    log_error "Aborted"
    exit 1
fi

# Create and push tag
log_info "Creating tag $VERSION..."
git tag -a "$VERSION" -m "Release $VERSION"

log_info "Pushing tag to GitHub..."
git push origin "$VERSION"

log_success "Tag $VERSION pushed!"

echo ""
echo -e "${GREEN}╔════════════════════════════════════════════════════════════╗"
echo -e "║                                                            ║"
echo -e "║              🚀 Release Triggered! 🚀                      ║"
echo -e "║                                                            ║"
echo -e "╚════════════════════════════════════════════════════════════╝${NC}"
echo ""
log_info "GitHub Actions is now:"
log_info "  1. Building Rust binary for ARM64 and x86_64"
log_info "  2. Creating release tarball"
log_info "  3. Generating changelog"
log_info "  4. Creating GitHub release"
echo ""
log_info "Watch progress at:"
echo -e "  ${BLUE}https://github.com/$(git config --get remote.origin.url | sed 's/.*github.com[\/:]\(.*\)\.git/\1/')/actions${NC}"
echo ""
log_info "Release will be available at:"
echo -e "  ${BLUE}https://github.com/$(git config --get remote.origin.url | sed 's/.*github.com[\/:]\(.*\)\.git/\1/')/releases/tag/$VERSION${NC}"
echo ""
log_info "This usually takes 3-5 minutes ⏱️"
echo ""
