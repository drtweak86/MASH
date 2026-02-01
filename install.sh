#!/usr/bin/env bash
set -euo pipefail

# Minimal wrapper: delegate to mash-installer binary.
./mash-installer "$@"
