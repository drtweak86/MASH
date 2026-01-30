#!/bin/bash
# LEGACY: superseded by tools/mash-release (Rust)
# Backup of the release script that bumps Cargo.toml/README, commits, tags, and pushes.
# Known bugs: invalid SemVer/tag when existing version contains ".."; non-SemVer Cargo version can break version parsing.
set -e

echo "╔════════════════════════════════════════╗"
echo "║     🚀 MASH Release Script             ║"
echo "╚════════════════════════════════════════╝"
echo ""

# 1. Get the latest tag (default to v1.1.12 if none exists)
LATEST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "v1.1.12")
echo "📌 Current Version: $LATEST_TAG"

# 2. Strip the leading 'v' if present
VERSION=${LATEST_TAG#v}

# 3. Split into Major.Minor.Patch
IFS='.' read -r MAJOR MINOR PATCH <<< "$VERSION"

# 4. Increment the Patch version
NEW_PATCH=$((PATCH + 1))
NEW_TAG="v$MAJOR.$MINOR.$NEW_PATCH"
NEW_VERSION="$MAJOR.$MINOR.$NEW_PATCH"

echo "🆕 New Version: $NEW_TAG"
echo ""

# 5. Update Cargo.toml
echo "📝 Updating Cargo.toml..."
sed -i "s/^version = \".*\"/version = \"$NEW_VERSION\"/" mash-installer/Cargo.toml

# 6. Update README.md version in title
echo "📝 Updating README.md..."
sed -i "s/^# MASH 🐀🍕.*/# MASH 🐀🍕 $NEW_TAG/" README.md

# 7. Show current status
echo ""
echo "📋 Current git status:"
git status --short
echo ""

# 8. Stage all changes
echo "📦 Staging all changes..."
git add -A

# 9. Show what will be committed
echo ""
echo "📋 Changes to be committed:"
git diff --cached --stat
echo ""

# 10. Prompt for commit message
echo "════════════════════════════════════════"
echo "Enter your commit message (or press Enter for default):"
echo "Default: 'chore: bump version to $NEW_TAG'"
echo "════════════════════════════════════════"
read -r USER_MESSAGE

if [ -z "$USER_MESSAGE" ]; then
    COMMIT_MSG="chore: bump version to $NEW_TAG"
else
    COMMIT_MSG="$USER_MESSAGE"
fi

# 11. Confirm before proceeding
echo ""
echo "📝 Commit message: $COMMIT_MSG"
echo "🏷️  Tag: $NEW_TAG"
echo ""
read -p "Proceed with commit, tag, and push? [y/N] " -n 1 -r
echo ""

if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "❌ Aborted. Changes are staged but not committed."
    exit 1
fi

# 12. Commit
echo ""
echo "💾 Committing..."
git commit -m "$COMMIT_MSG"

# 13. Create tag
echo "🏷️  Creating tag $NEW_TAG..."
git tag -a "$NEW_TAG" -m "Release $NEW_TAG"

# 14. Push
echo "🚀 Pushing to remote..."
git push origin main --tags

echo ""
echo "╔════════════════════════════════════════╗"
echo "║  ✅ Release $NEW_TAG pushed!           ║"
echo "╚════════════════════════════════════════╝"
echo ""
echo "GitHub Actions will now build the release."
