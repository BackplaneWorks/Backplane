#!/bin/sh
# A rig for the iOS terminal journey: a hub on a temp home (stand-in agents only, --no-tailscale) and a relay in front of it that a file can cut,
# both as systemd user units (a Backplane restart does not kill them). The Mac's simulator reaches them on this machine's tailnet address.
#   ios-term-rig.sh start|stop|cut|heal      env: HUB (the hub binary), IP (default: tailscale ip -4), PORT (hub, 3801; the relay is PORT+1)
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd); . "$ROOT/scripts/lib-bptmp.sh"
RIG=$BP_TMP/iterm-rig; IP=${IP:-$(tailscale ip -4 | head -1)}; PORT=${PORT:-3801}; HUB=${HUB:-$ROOT/native/host/target/fast/backplane-hub}
case "${1:-}" in
start)
  rm -rf "$RIG"; mkdir -p "$RIG/home" "$RIG/proj" "$RIG/bin"; git -C "$RIG/proj" init -q; touch "$RIG/proj/alpha.txt" "$RIG/proj/beta.txt" "$RIG/proj/gamma.txt"
  printf '#!/bin/sh\nexec %s %s "$@"\n' "$HOME/.bun/bin/bun" "$ROOT/test/tools/claude_standin.ts" > "$RIG/bin/claude"
  for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > "$RIG/bin/$n"; done; chmod +x "$RIG"/bin/*
  systemd-run --user --unit bp-iterm-hub -q -p WorkingDirectory="$RIG/proj" -E BACKPLANE_AGENTS="$RIG/bin" -E BACKPLANE_NO_UPDATE=1 -E FAKE_LOG="$RIG/standin.log" -E TZ=Etc/UTC -E TMPDIR="$BP_TMP" \
    "$HUB" --home "$RIG/home" --port "$PORT" --host "$IP" --no-tailscale --foreground
  systemd-run --user --unit bp-iterm-relay -q /usr/bin/python3 "$ROOT/scripts/mobile/tcp-relay.py" "$IP" $((PORT+1)) "$IP" "$PORT" "$RIG/cut"
  echo "rig: hub $IP:$PORT, relay $IP:$((PORT+1)) (control $((PORT+2))), $RIG";;
cut) : > "$RIG/cut";;
heal) rm -f "$RIG/cut";;
stop) systemctl --user stop bp-iterm-hub bp-iterm-relay 2>/dev/null || true; systemctl --user reset-failed bp-iterm-hub bp-iterm-relay 2>/dev/null || true; rm -rf "$RIG";;
*) echo "usage: $0 start|stop|cut|heal"; exit 2;;
esac
