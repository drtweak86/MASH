#!/bin/bash
# Quick fix for binary detection

# Replace the old binary search
sed -i 's|cli_binary="$(find . -type f -name '\''MASH'\''|cli_binary="$(find "mash-installer-${arch}" -type f -executable|' install.sh

# Add fallback if directory doesn't exist
sed -i '/cli_binary="$(find "mash-installer-${arch}"/a\    if [[ -z "$cli_binary" ]]; then cli_binary="$(find . -type f -name '\''mash-installer*'\'' -executable -print -quit 2>/dev/null || true)"; fi' install.sh

echo "Fixed!"
