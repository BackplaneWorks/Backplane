#!/bin/sh
# A project row's fold and file links opening inside Backplane, on the real window, judged by test/e2e/fold_links.py: the Rust hub on a temp home,
# stand-in agents only, loopback, the client in a private Xvfb with its own i3 (nothing maps on a real display), XTest mouse and keys. Screenshots: $R/shots.
# usage: desktop-fold-links.sh CLIENT_BIN HUB_BIN ASSETS_DIR [OUTDIR]   env: GPUI_PORT (3851), GPUI_DISP (:71), XVFB (default ~/.local/xvfb-root/usr/bin/Xvfb), LIGHT=1 (light theme)
set -eu
ME=$(cd "$(dirname "$0")/.." && pwd); CLIENT=$(realpath "$1"); HUBBIN=$(realpath "$2"); ASSETS=$(realpath "$3")
. "$ME/scripts/lib-bptmp.sh"; R=${4:-$(mktemp -d "$BP_TMP/fold-links.XXXX")}
PORT=${GPUI_PORT:-3851}; DISP=${GPUI_DISP:-:71}; XV=${XVFB:-$HOME/.local/xvfb-root/usr/bin/Xvfb}
[ -x "$CLIENT" ] && [ -x "$HUBBIN" ] && [ -x "$XV" ] || { echo "need CLIENT_BIN, HUB_BIN and Xvfb"; exit 2; }
[ "$DISP" != ":0" ] && [ ! -e "/tmp/.X11-unix/X${DISP#:}" ] || { echo "display $DISP is in use (set GPUI_DISP)"; exit 2; }
mkdir -p "$R/home" "$R/bp/boards/main" "$R/fw" "$R/loose" "$R/bin" "$R/fakehome/.cache" "$R/shots" "$R/run"; chmod 700 "$R/run"
F="$ME/test/files/fixtures/plot"
cp "$F/ecc83-pp.kicad_sch" "$R/bp/boards/main/main.kicad_sch"; cp "$F/ecc83-pp.kicad_pcb" "$R/bp/boards/main/main.kicad_pcb"; cp "$F/microwave.kicad_pcb" "$R/loose/loose.kicad_pcb"
for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > "$R/bin/$n"; chmod +x "$R/bin/$n"; done
printf '#!/bin/sh\nexec %s %s/test/tools/claude_standin.ts "$@"\n' "$(command -v bun)" "$ME" > "$R/bin/claude"; chmod +x "$R/bin/claude"
NOW=$(date +%s)
{ printf '{"$":"ProjectCreated","id":"p1","title":"Backplane","root":"%s","at":%s}\n' "$R/bp" "$NOW"
  printf '{"$":"ProjectCreated","id":"p2","title":"Framework Expansion Cards","root":"%s","at":%s}\n' "$R/fw" "$((NOW - 60))"
  for i in 1 2 3 4; do
    printf '{"$":"ThreadCreated","id":"t%s","project":"p1","title":"%s","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":%s}\n' "$i" "$(echo 'KiCad fork bp.8: embed m|README rendering: inline|KiCad first-party: Gemini|KiCad as a first-party p' | cut -d'|' -f$i)" "$((NOW - 100 + i))"
    # each is working in the background (a watched shell): listed under its project and counted as running, with no agent turn to end
    printf '{"$":"TurnChanged","thread":"t%s","state":"completed","at":%s}\n' "$i" "$((NOW - 50 + i))"
    printf '{"$":"SettingSet","key":"bg.t%s","value":"b%s\\twatching shell\\t\\t\\tshell","at":%s}\n' "$i" "$i" "$((NOW - 49 + i))"
  done
  printf '{"$":"ThreadCreated","id":"t5","project":"p2","title":"Expansion card pinout","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":%s}\n' "$((NOW - 90))"
  printf '{"$":"TurnChanged","thread":"t5","state":"completed","at":%s}\n' "$((NOW - 40))"
  printf '{"$":"SettingSet","key":"bg.t5","value":"b5\\twatching shell\\t\\t\\tshell","at":%s}\n' "$((NOW - 39))"
  printf '{"$":"MessagePosted","thread":"t4","msg":"m0","role":"assistant","text":"Saved main.kicad_pcb after the reroute.","at":%s}\n' "$((NOW - 45))"
  printf '{"$":"MessagePosted","thread":"t5","msg":"m1","role":"assistant","text":"The amp schematic is %s/bp/boards/main/main.kicad_sch ; a loose one: file://%s/loose/loose.kicad_pcb","at":%s}\n' "$R" "$R" "$((NOW - 30))"
} > "$R/home/events.jsonl"
echo '[{"wait":280000}]' > "$R/script.json"
PIDS=; cleanup() { for p in $PIDS; do kill "$p" 2>/dev/null || true; done; }; trap cleanup EXIT INT TERM
unset I3SOCK DBUS_SESSION_BUS_ADDRESS
export XDG_RUNTIME_DIR="$R/run" LC_ALL=C BACKPLANE_AGENTS="$R/bin" BACKPLANE_NO_UPDATE=1 BP_STANDIN_MODE=plain HOME="$R/fakehome" XDG_CACHE_HOME="$R/fakehome/.cache" XDG_CONFIG_HOME="$R/fakehome/.config" XDG_DATA_HOME="$R/fakehome/.local/share"
cd "$R/bp"
# the hub, restartable by the judge (it writes $R/restart; the hub comes back on the same home, the client reconnects)
( while :; do "$HUBBIN" --home "$R/home" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground >> "$R/hub.log" 2>&1 & h=$!; echo "$h" > "$R/hub.pid"
    while kill -0 "$h" 2>/dev/null; do [ -e "$R/restart" ] && { rm -f "$R/restart"; kill "$h"; wait "$h" 2>/dev/null || true; break; }; sleep 0.2; done
    [ -e "$R/stop" ] && exit 0; done ) & H=$!; PIDS="$H"; sleep 4
tr '\0' '\n' < "/proc/$(cat "$R/hub.pid")/environ" | grep -q "^BACKPLANE_AGENTS=$R/bin\$" || { echo "NO BACKPLANE_AGENTS on the hub"; exit 3; }
"$XV" "$DISP" -screen 0 1440x900x24 -nolisten tcp -xkbdir /usr/share/X11/xkb > "$R/xvfb.log" 2>&1 & V=$!; PIDS="$V $PIDS"; sleep 2
printf 'font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\nhide_edge_borders both\nipc-socket %s/i3.sock\n' "$R" > "$R/i3.conf"
DISPLAY="$DISP" i3 -c "$R/i3.conf" > "$R/i3.log" 2>&1 & I=$!; PIDS="$I $PIDS"; sleep 1
export DISPLAY="$DISP" VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json LP_NUM_THREADS=2 BACKPLANE_GPUI_ASSETS="$ASSETS" BACKPLANE_GPUI_RECTS=1
THEME=dark; [ "${LIGHT:-}" = 1 ] && THEME=light
"$CLIENT" --url "http://127.0.0.1:$PORT" --token-file "$R/home/token" --script "$R/script.json" --exit-after 300 --theme "$THEME" > "$R/client.log" 2>&1 & C=$!; PIDS="$C $PIDS"; sleep 8
set +e; python3 "$ME/test/e2e/fold_links.py" "$R" "$ME/test/cef/xinput.py"; rc=$?
touch "$R/stop"; kill "$(cat "$R/hub.pid")" 2>/dev/null; echo "run: $R"; exit $rc
