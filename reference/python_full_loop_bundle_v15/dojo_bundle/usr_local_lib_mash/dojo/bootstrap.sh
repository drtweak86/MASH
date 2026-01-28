#!/usr/bin/env bash
set -euo pipefail

ASSETS="/usr/local/lib/mash/dojo/assets"
STARSHIP_TOML="$ASSETS/starship.toml"

run_bootstrap() {
  echo "== MASH bootstrap 🔥 =="
  echo "(placeholder) This should call /data/mash-staging/install_dojo.sh or mash_forge actions."
  if [[ -x /data/mash-staging/install_dojo.sh ]]; then
    sudo /data/mash-staging/install_dojo.sh /data/mash-staging
  else
    echo "⚠️ /data/mash-staging/install_dojo.sh not found."
  fi
}

preview_starship() {
  echo "== Starship preview ⭐ =="
  if ! command -v starship >/dev/null 2>&1; then
    echo "Installing starship..."
    sudo dnf -y install starship || true
  fi
  if [[ -f "$STARSHIP_TOML" ]]; then
    export STARSHIP_CONFIG="$STARSHIP_TOML"
  fi
  if command -v starship >/dev/null 2>&1; then
    echo
    echo "Prompt preview:"
    echo "----------------------------------------"
    PS1="$(starship prompt)"
    echo "$PS1"
    echo "----------------------------------------"
    echo "(Set STARSHIP_CONFIG=$STARSHIP_CONFIG)"
  else
    echo "❌ starship not available."
  fi
  read -rp "Press Enter to return… " _
}

case "${1:-}" in
  --run) shift; run_bootstrap "$@";;
  --preview-starship) shift; preview_starship "$@";;
  ""|--help|-h) : ;;
  *) echo "Unknown arg: $1" >&2; exit 2;;
esac
