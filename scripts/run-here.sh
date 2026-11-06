#!/bin/sh
# Launch the GPUI desktop client on YOUR screen (uses your display and GPU, e.g. the RTX 5080).
# usage: scripts/run-here.sh [client args]   (default hub: the shared GPUI hub over the tailnet)
ROOT=$(cd "$(dirname "$0")/.." && pwd)
exec "$ROOT/build/gpui/linux/backplane-gpui" --url "${BACKPLANE_URL:-http://desktop.tail9e0cf2.ts.net:3799}" --token-file "${BACKPLANE_TOKEN_FILE:-$HOME/.backplane-gpui/token}" "$@"
