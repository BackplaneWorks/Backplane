#!/bin/sh
# Pinned threads and picked project icons on the real window, judged by test/e2e/pins_icons.py: the Rust hub on a temp home, stand-in agents only, loopback, the client in a
# private Xvfb with its own i3 (nothing maps on a real display), XTest mouse and keys. The `claude` here answers the icon picker's one-shot itself (no model is asked):
# "usb teal" for a project that names USB, "presentation" for one with a deck, "plug" the second time a project is asked (Pick again); everything else goes to the stand-in.
# usage: desktop-pins-icons.sh CLIENT_BIN HUB_BIN ASSETS_DIR [OUTDIR]   env: GPUI_PORT (3847), GPUI_DISP (:67), XVFB (the Xvfb binary, default ~/.local/xvfb-root/usr/bin/Xvfb)
set -eu
ME=$(cd "$(dirname "$0")/.." && pwd); CLIENT=$1; HUBBIN=$2; ASSETS=$3
. "$ME/scripts/lib-bptmp.sh"; R=${4:-$(mktemp -d "$BP_TMP/pins-icons.XXXX")}
PORT=${GPUI_PORT:-3847}; DISP=${GPUI_DISP:-:67}; XV=${XVFB:-$HOME/.local/xvfb-root/usr/bin/Xvfb}
[ -x "$CLIENT" ] && [ -x "$HUBBIN" ] && [ -x "$XV" ] || { echo "need CLIENT_BIN, HUB_BIN and Xvfb"; exit 2; }
[ "$DISP" != ":0" ] && [ ! -e "/tmp/.X11-unix/X${DISP#:}" ] || { echo "display $DISP is in use (set GPUI_DISP)"; exit 2; }
mkdir -p "$R/home" "$R/usb/boards/main" "$R/usb/.backplane" "$R/deck" "$R/bin" "$R/fakehome/.cache" "$R/shots" "$R/run"; chmod 700 "$R/run"
printf '# Goal\nA USB 2.0 protocol analyzer that sniffs a link between a host and a device.\n\n# Parts\n- Main board, boards/main, the analyzer, to order\n' > "$R/usb/.backplane/intent.md"
touch "$R/usb/boards/main/main.kicad_pro" "$R/deck/pitch.pptx" "$R/deck/notes.md"
cat > "$R/bin/claude" <<EOF
#!/bin/sh
case "\$*" in *"choose the icon for a project"*)
  IN=\$(cat); echo "\$IN" >> "$R/picks.log"; N=\$(grep -c '^Name: ' "$R/picks.log")
  if echo "\$IN" | grep -q 'Name: USB'; then if [ "\$(grep -c '^Name: USB' "$R/picks.log")" -gt 1 ]; then A=plug; else A='usb teal'; fi
  elif echo "\$IN" | grep -q 'pptx'; then A=presentation; else A='a skull'; fi
  printf '{"type":"result","subtype":"success","is_error":false,"result":"%s","session_id":"one"}\n' "\$A"; exit 0;;
esac
exec $(command -v bun) $ME/test/tools/claude_standin.ts "\$@"
EOF
chmod +x "$R/bin/claude"
for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > "$R/bin/$n"; chmod +x "$R/bin/$n"; done
NOW=$(date +%s)
{ printf '{"$":"ProjectCreated","id":"p1","title":"USB sniffer","root":"%s","at":%s}\n' "$R/usb" "$NOW"
  printf '{"$":"ProjectCreated","id":"p2","title":"Deck","root":"%s","at":%s}\n' "$R/deck" "$NOW"
  for i in 1 2 3 4; do printf '{"$":"ThreadCreated","id":"t%s","project":"p1","title":"thread %s","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":%s}\n' "$i" "$i" "$((NOW - 10 + i))"; done
} > "$R/home/events.jsonl"
echo '[{"wait":280000}]' > "$R/script.json"
PIDS=; cleanup() { for p in $PIDS; do kill "$p" 2>/dev/null || true; done; }; trap cleanup EXIT INT TERM
unset I3SOCK DBUS_SESSION_BUS_ADDRESS   # a private runtime dir: i3 dies on the shared one's socket, and with no window manager the client never draws
export XDG_RUNTIME_DIR="$R/run" LC_ALL=C BACKPLANE_AGENTS="$R/bin" BACKPLANE_NO_UPDATE=1 BP_STANDIN_MODE=plain HOME="$R/fakehome" XDG_CACHE_HOME="$R/fakehome/.cache" XDG_CONFIG_HOME="$R/fakehome/.config" XDG_DATA_HOME="$R/fakehome/.local/share"
cd "$R/usb"
# the hub, restartable by the judge (it writes $R/restart; the hub comes back on the same home, the client reconnects)
( while :; do "$HUBBIN" --home "$R/home" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground >> "$R/hub.log" 2>&1 & h=$!; echo "$h" > "$R/hub.pid"
    while kill -0 "$h" 2>/dev/null; do [ -e "$R/restart" ] && { rm -f "$R/restart"; kill "$h"; wait "$h" 2>/dev/null || true; break; }; sleep 0.2; done
    [ -e "$R/stop" ] && exit 0; done ) & H=$!; PIDS="$H"; sleep 4
tr '\0' '\n' < "/proc/$(cat "$R/hub.pid")/environ" | grep -q "^BACKPLANE_AGENTS=$R/bin\$" || { echo "NO BACKPLANE_AGENTS on the hub"; exit 3; }
"$XV" "$DISP" -screen 0 1440x900x24 -nolisten tcp -xkbdir /usr/share/X11/xkb > "$R/xvfb.log" 2>&1 & V=$!; PIDS="$V $PIDS"; sleep 2
printf 'font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\nhide_edge_borders both\nipc-socket %s/i3.sock\n' "$R" > "$R/i3.conf"
DISPLAY="$DISP" i3 -c "$R/i3.conf" > "$R/i3.log" 2>&1 & I=$!; PIDS="$I $PIDS"; sleep 1
export DISPLAY="$DISP" VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json LP_NUM_THREADS=2 BACKPLANE_GPUI_ASSETS="$ASSETS" BACKPLANE_GPUI_RECTS=1
"$CLIENT" --url "http://127.0.0.1:$PORT" --token-file "$R/home/token" --script "$R/script.json" --exit-after 300 > "$R/client.log" 2>&1 & C=$!; PIDS="$C $PIDS"; sleep 8
set +e; python3 "$ME/test/e2e/pins_icons.py" "$R" "$ME/test/cef/xinput.py"; rc=$?
touch "$R/stop"; kill "$(cat "$R/hub.pid")" 2>/dev/null; echo "run: $R"; exit $rc
