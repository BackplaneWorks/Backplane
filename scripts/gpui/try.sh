#!/bin/sh
# Try the GPUI app on your own screen, over a throwaway, trimmed copy of the store.
#   scripts/gpui/try.sh start     test hub + GPUI window (the hub loads the log first)
#   scripts/gpui/try.sh stop      stop both and delete the copy
#   scripts/gpui/try.sh status
# Never the live hub or store: its own home and port, loopback only, BACKPLANE_COPY=1
# (no network commands, no deploys, no pushes) and stand-in agents, 3.7 GiB, no swap.
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
H=$HOME/.backplane-gpui-try
SRC=$HOME/.backplane-gpui-copy/events.jsonl
PORT=3860
URL=http://127.0.0.1:$PORT
HUB=bp-gpui-try-hub
APP=bp-gpui-try-app
CAP="-p MemoryMax=3700M -p MemorySwapMax=0"
DISPLAY_TO=${GPUI_TRY_DISPLAY:-${DISPLAY:-:0}}
ICD=${GPUI_TRY_ICD:-/usr/share/vulkan/icd.d/nvidia_icd.json}

up() { curl -s -m 2 -o /dev/null "$URL/hello"; }
running() { systemctl --user is-active --quiet "$1"; }

start_hub() {
  running $HUB && return 0
  [ -f "$SRC" ] || { echo "no store copy at $SRC" >&2; exit 1; }
  mkdir -p -m 700 "$H/home" "$H/fakehome" "$H/bin" "$H/locks"
  if [ ! -f "$H/home/events.jsonl" ]; then
    echo "making a smaller copy with the ${GPUI_TRY_THREADS:-200} most recent threads..."
    python3 "$ROOT/scripts/gpui/try-trim.py" "$SRC" "$H/home/events.jsonl" "${GPUI_TRY_THREADS:-200}"
  fi
  cp "$ROOT/build/gpui/backplane-serve-current" "$H/backplane-serve"
  printf '#!/bin/sh\nexec %s %s "$@"\n' "$(command -v bun)" "$ROOT/test/tools/claude_standin.ts" > "$H/bin/claude"
  for n in codex grok muse opencode pi omp antigravity; do printf '#!/bin/sh\nexit 1\n' > "$H/bin/$n"; done
  chmod +x "$H"/bin/*
  systemd-run --user --unit=$HUB --collect -q $CAP --working-directory="$H/fakehome" \
    --setenv=HOME="$H/fakehome" --setenv=BACKPLANE_COPY=1 --setenv=BACKPLANE_NO_UPDATE=1 --setenv=BACKPLANE_ADOPT=0 \
    --setenv=BACKPLANE_PEERS= --setenv=BACKPLANE_ORDER_SERVER=http://127.0.0.1:9 --setenv=BACKPLANE_AGENTS="$H/bin" \
    --setenv=BACKPLANE_LOCK_DIR="$H/locks" --setenv=BP_STANDIN_MODE=stream --setenv=BP_STREAM_N=24 --setenv=BP_STREAM_MS=60 \
    "$H/backplane-serve" --home "$H/home" --port $PORT --host 127.0.0.1 --no-tailscale --foreground --web "$ROOT/build/gpui/web"
}

wait_hub() {
  n=0
  until up; do
    running $HUB || { echo "the test hub stopped; see: journalctl --user -u $HUB -n 30" >&2; exit 1; }
    n=$((n + 1)); [ $((n % 15)) = 0 ] && echo "hub is loading the log... ${n}s"
    [ $n -lt 900 ] || { echo "hub not up after 15 minutes" >&2; exit 1; }
    sleep 1
  done
  echo "hub ready after about ${n}s"
}

case "${1:-}" in
start)
  start_hub; wait_hub
  [ "${GPUI_TRY_NOAPP:-}" = 1 ] && exit 0
  running $APP && { echo "GPUI is already open"; exit 0; }
  cp "$ROOT/build/gpui/linux/backplane-gpui" "$H/backplane-gpui"
  # assets/ (cat rigs, icons.json) beside the binary; the app logs asset_missing and shows it in the window when it is not there
  rm -rf "$H/assets" "$H/cat-rigs"; cp -r "$ROOT/build/gpui/linux/assets" "$H/assets"
  systemd-run --user --unit=$APP --collect -q $CAP --working-directory="$H" \
    --setenv=DISPLAY="$DISPLAY_TO" --setenv=XAUTHORITY="${XAUTHORITY:-/run/user/$(id -u)/gdm/Xauthority}" --setenv=VK_ICD_FILENAMES="$ICD" \
    "$H/backplane-gpui" --url "$URL" --token-file "$H/home/token"
  echo "GPUI window opened. Stop and clean up with: scripts/gpui/try.sh stop"
  ;;
stop)
  systemctl --user stop $APP 2>/dev/null || true
  systemctl --user stop $HUB 2>/dev/null || true
  rm -rf "$H"
  echo "stopped, and $H removed"
  ;;
status)
  for u in $HUB $APP; do printf '%s: ' $u; systemctl --user is-active $u || true; done
  up && echo "hub answers at $URL" || echo "hub not answering"
  du -sh "$H" 2>/dev/null || true
  ;;
*) sed -n 2,7p "$0"; exit 2 ;;
esac
