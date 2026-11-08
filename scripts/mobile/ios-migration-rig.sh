#!/bin/sh
# A rig for the iOS 0.14 import card (Migration.swift): a hub on a fresh temp home (stand-in agents only, --no-tailscale, on this machine's tailnet address, a
# systemd user unit) that imports a large made-up 0.14 home by itself, slowly (BACKPLANE_IMPORT_AUTO=1, BACKPLANE_IMPORT_THROTTLE_MS).
#   ios-migration-rig.sh start|stop      env: HUB (the hub binary), IP (default: tailscale ip -4), PORT (3861), THROTTLE (1500), LARGE ("50 5000 300000")
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd); . "$ROOT/scripts/lib-bptmp.sh"
RIG=$BP_TMP/imig-rig; IP=${IP:-$(tailscale ip -4 | head -1)}; PORT=${PORT:-3861}; HUB=${HUB:-$ROOT/native/host/target/fast/backplane-hub}
case "${1:-}" in
start)
  systemctl --user stop bp-imig-hub 2>/dev/null || true; systemctl --user reset-failed bp-imig-hub 2>/dev/null || true
  # the 0.14 home is kept between runs (it takes a while to write); the hub's home is new each time, so the import runs again
  [ -f "$RIG/old/events.jsonl" ] || { rm -rf "$RIG/old"; mkdir -p "$RIG"; python3 "$ROOT/test/fixtures/make-v014-home.py" "$RIG/old" --large ${LARGE:-50 5000 300000} >/dev/null; }
  rm -rf "$RIG/home" "$RIG/proj" "$RIG/bin"; mkdir -p "$RIG/home" "$RIG/proj" "$RIG/bin"
  printf '#!/bin/sh\nexec %s %s "$@"\n' "$HOME/.bun/bin/bun" "$ROOT/test/tools/claude_standin.ts" > "$RIG/bin/claude"
  for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > "$RIG/bin/$n"; done; chmod +x "$RIG"/bin/*
  cp "$HUB" "$RIG/hub"
  systemd-run --user --unit bp-imig-hub -q -p WorkingDirectory="$RIG/proj" -E BACKPLANE_AGENTS="$RIG/bin" -E BACKPLANE_NO_UPDATE=1 -E TMPDIR="$BP_TMP" \
    -E BACKPLANE_IMPORT_FROM="$RIG/old" -E BACKPLANE_IMPORT_AUTO=1 -E BACKPLANE_IMPORT_THROTTLE_MS="${THROTTLE:-1500}" \
    "$RIG/hub" --home "$RIG/home" --port "$PORT" --host "$IP" --no-tailscale --foreground
  i=0; while [ ! -s "$RIG/home/token" ] && [ $i -lt 30 ]; do sleep 1; i=$((i+1)); done
  echo "rig: hub $IP:$PORT, $RIG";;
stop) systemctl --user stop bp-imig-hub 2>/dev/null || true; systemctl --user reset-failed bp-imig-hub 2>/dev/null || true; rm -rf "$RIG";;
*) echo "usage: $0 start|stop"; exit 2;;
esac
