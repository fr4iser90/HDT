#!/usr/bin/env bash
# NixOS-friendly overlay launcher (uses steam-run for X11/GL libs).
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -p bgc-ui
export DISPLAY="${DISPLAY:-:0}"
if command -v steam-run >/dev/null 2>&1; then
  exec steam-run ./target/debug/bgc-ui "$@"
else
  exec ./target/debug/bgc-ui "$@"
fi
