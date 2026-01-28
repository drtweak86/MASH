#!/usr/bin/env bash
set -euo pipefail

LOGDIR="/data/mash-logs"
LOGFILE="${LOGDIR}/internet-wait.log"
mkdir -p "${LOGDIR}"
exec > >(tee -a "${LOGFILE}") 2>&1

ply() {
  if command -v plymouth >/dev/null 2>&1 && plymouth --ping >/dev/null 2>&1; then
    plymouth display-message --text="$1" || true
  fi
}

echo "== Internet wait =="
date

# Wait for a default route + DNS resolution
ply "🌐 Waiting for network…"

for i in $(seq 1 60); do
  if ip route | grep -q '^default '; then
    break
  fi
  sleep 1
done

# Try DNS: resolve a well-known name (no ICMP required)
for i in $(seq 1 60); do
  if getent ahosts deb.debian.org >/dev/null 2>&1 || getent ahosts github.com >/dev/null 2>&1; then
    ply "✅ Network + DNS OK"
    echo "OK: DNS resolution working"
    exit 0
  fi
  sleep 1
done

ply "⚠️ No DNS yet — continuing anyway"
echo "WARN: DNS check failed after timeout; continuing"
exit 0
