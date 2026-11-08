#!/bin/bash
# The size matrix on this machine (docs/e2e/SIZES.md): the GPUI client at every window size and text scale in test/e2e/layout/sizes.json, each screen audited and shot.
#   scripts/e2e-sizes.sh [--quick] [--sizes 800x600,...] [--text 80,100,125] [--only scenario,...] [--client EXE --hub EXE --assets DIR] [OUT_DIR]
#   --quick: 800x600 and 1280x720 at 100%, the screens that broke before (CI's per-push set). Needs python3 + Pillow, and a display stack: sway+grim (headless Wayland+Xwayland) or Xvfb+i3+lavapipe.
# Linux here; the Windows and macOS lanes run test/e2e/layout/matrix.py directly (--platform windows|mac) inside their desktop sessions.
set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd); cd "$ROOT"
ARGS=(); OUT=; CLIENT=; HUB=; ASSETS=
while [ $# -gt 0 ]; do case "$1" in
  --quick) ARGS+=(--sizes 800x600,1280x720 --text 100);;
  --sizes|--text|--only|--platform|--shard|--backend) ARGS+=("$1" "$2"); shift;;
  --client) CLIENT=$2; shift;; --hub) HUB=$2; shift;; --assets) ASSETS=$2; shift;;
  *) OUT=$1;; esac; shift; done
CLIENT=${CLIENT:-$ROOT/native/gpui/target/fast/backplane-gpui}; HUB=${HUB:-$ROOT/native/host/target/fast/backplane-hub}; ASSETS=${ASSETS:-$ROOT/assets}
[ -x "$CLIENT" ] || CLIENT=$ROOT/native/gpui/target/debug/backplane-gpui
[ -x "$HUB" ] || HUB=$ROOT/native/host/target/debug/backplane-hub
[ -x "$CLIENT" ] && [ -x "$HUB" ] || { echo "no client/hub binaries (build first, or pass --client/--hub)"; exit 2; }
OUT=${OUT:-$HOME/.cache/e2e-sizes-$(git rev-parse --short HEAD)}
export NO_SCOPE=1 BACKPLANE_NO_UPDATE=1
python3 test/e2e/layout/check.py || exit 2
exec python3 -W ignore test/e2e/layout/matrix.py --client "$CLIENT" --hub "$HUB" --assets "$ASSETS" --out "$OUT" "${ARGS[@]}"
