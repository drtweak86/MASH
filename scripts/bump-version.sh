#!/usr/bin/env bash

# Bump version script for MASH Installer
# This will automatically trigger GitHub Actions to build and release

set -e

CARGO_TOML="mash-installer/Cargo.toml"
CMAKE_FILE="qt-gui/CMakeLists.txt"

if [ ! -f "$CARGO_TOML" ]; then
    echo "Error: $CARGO_TOML not found"
    exit 1
fi

# Get current version
CURRENT_VERSION=$(grep '^version = ' "$CARGO_TOML" | sed 's/version = "\(.*\)"/\1/')
echo "Current version: $CURRENT_VERSION"

# Parse version
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"

# Determine bump type
if [ "$1" = "major" ]; then
    MAJOR=$((MAJOR + 1))
    MINOR=0
    PATCH=0
elif [ "$1" = "minor" ]; then
    MINOR=$((MINOR + 1))
    PATCH=0
elif [ "$1" = "patch" ] || [ -z "$1" ]; then
    PATCH=$((PATCH + 1))
else
    echo "Usage: $0 [major|minor|patch]"
    echo "  major: X.0.0"
    echo "  minor: $MAJOR.X.0"
    echo "  patch: $MAJOR.$MINOR.X (default)"
    exit 1
fi

NEW_VERSION="$MAJOR.$MINOR.$PATCH"
echo "New version: $NEW_VERSION"

# Confirm
read -p "Proceed with version bump? [y/N] " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Aborted"
    exit 1
fi

# Update Cargo.toml
echo "Updating $CARGO_TOML..."
sed -i "s/^version = \".*\"/version = \"$NEW_VERSION\"/" "$CARGO_TOML"

# Update CMakeLists.txt
if [ -f "$CMAKE_FILE" ]; then
    echo "Updating $CMAKE_FILE..."
    sed -i "s/project(mash-installer-qt VERSION .*/project(mash-installer-qt VERSION $NEW_VERSION LANGUAGES CXX)/" "$CMAKE_FILE"
fi

# Update README.md version in title
README_FILE="README.md"
if [ -f "$README_FILE" ]; then
    echo "Updating $README_FILE..."
    sed -i "s/^# MASH 🐀🍕.*/# MASH 🐀🍕 v$NEW_VERSION/" "$README_FILE"
fi

# Git operations
echo "Creating git commit..."
git add "$CARGO_TOML" "$CMAKE_FILE" "$README_FILE"
git commit -m "Bump version to $NEW_VERSION"

echo "Creating git tag..."
git tag -a "v$NEW_VERSION" -m "Release v$NEW_VERSION"

echo ""
echo "✅ Version bumped to $NEW_VERSION"
echo ""
echo "Next steps:"
echo "  1. Review changes: git show"
echo "  2. Push to trigger CI/CD: git push origin main --tags"
echo ""
echo "This will automatically:"
echo "  - Build binaries for ARM64 and x86_64"
echo "  - Create a GitHub release"
echo "  - Upload artifacts"
echo ""
