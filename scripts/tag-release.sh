#!/bin/bash
set -e

# 1. Get the latest tag (default to v0.0.0 if none exists)
LATEST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "v1.1.12")
echo "Current Version: $LATEST_TAG"

# 2. Strip the leading 'v' if present
VERSION=${LATEST_TAG#v}

# 3. Split into Major.Minor.Patch
IFS='.' read -r MAJOR MINOR PATCH <<< "$VERSION"

# 4. Increment the Patch version
NEW_PATCH=$((PATCH + 1))
NEW_TAG="v$MAJOR.$MINOR.$NEW_PATCH"
NEW_VERSION="$MAJOR.$MINOR.$NEW_PATCH"

echo "Releasing: $NEW_TAG"

# 5. Update Cargo.toml (Optional but recommended for Rust)
# This finds version = "..." and replaces it.
sed -i "s/^version = \".*\"/version = \"$NEW_VERSION\"/" mash-installer/Cargo.toml

# 6. Update README.md version in title
sed -i "s/^# MASH 🐀🍕.*/# MASH 🐀🍕 $NEW_TAG/" README.md

# 7. Git Operations
git add mash-installer/Cargo.toml README.md
git commit -m "chore: bump version to $NEW_TAG"
git tag -a "$NEW_TAG" -m "Release $NEW_TAG"
git push origin main --tags

echo "✅ Release $NEW_TAG pushed successfully."
