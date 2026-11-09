#!/bin/sh
# A hub for the /give phone journeys: temp home, stand-in agents only, --no-tailscale, seeded by test/give/seed.py (a thread that did work, two to give to, a bot).
#   give-rig.sh start NAME HOST PORT | stop NAME | link NAME HOST PORT     (state in $BP_TMP/give-NAME; HUB: the hub binary)
# The Mac's simulator reaches a hub on this machine's tailnet address; the Pixel reaches one on 127.0.0.1 through `adb reverse`.
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd); . "$ROOT/scripts/lib-bptmp.sh"
BUN=$(command -v bun || echo "$HOME/.bun/bin/bun"); HUB=${HUB:-$ROOT/native/host/target/fast/backplane-hub}
N=${2:-a}; R=$BP_TMP/give-$N
case "${1:-}" in
start)
  HOST=$3; PORT=$4
  rm -rf "$R"; mkdir -p "$R/home" "$R/proj" "$R/bin"
  printf '#!/bin/sh\nexec %s %s "$@"\n' "$BUN" "$ROOT/test/tools/claude_standin.ts" > "$R/bin/claude"
  for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > "$R/bin/$n"; done; chmod +x "$R"/bin/*
  python3 -I "$ROOT/test/give/seed.py" "$R"
  systemd-run --user --unit "bp-give-$N" -q -p WorkingDirectory="$R/proj" -E BACKPLANE_AGENTS="$R/bin" -E BACKPLANE_NO_UPDATE=1 -E BP_STANDIN_MODE=stream -E BP_STANDIN_SUMMARY=1 -E BP_STREAM_N=200 -E TZ=Etc/UTC -E TMPDIR="$BP_TMP" \
    "$HUB" --home "$R/home" --port "$PORT" --host "$HOST" --no-tailscale --foreground
  sleep 5; curl -s -m3 "http://$HOST:$PORT/hello" | head -c 120; echo;;
stop) systemctl --user stop "bp-give-$N" 2>/dev/null || true; systemctl --user reset-failed "bp-give-$N" 2>/dev/null || true; rm -rf "$R";;
link) echo "http://$3:$4/#token=$(cat "$R/home/token")";;
*) echo "usage: $0 start NAME HOST PORT | stop NAME | link NAME HOST PORT"; exit 2;;
esac
