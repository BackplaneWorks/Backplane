#!/bin/sh
# Every project icon on the real window: a hub on a temp home with one project per icon of the hub's list (titled by its name, its icon set as the model's pick) and one named
# Backplane (the logo), the client in a private Xvfb with its own i3. Screenshots of the sidebar and of the badge editor; fails when the client logs an icon it cannot draw.
# usage: desktop-icon-gallery.sh CLIENT_BIN HUB_BIN ASSETS_DIR [OUTDIR]   env: GPUI_PORT (3849), GPUI_DISP (:68), XVFB (default ~/.local/xvfb-root/usr/bin/Xvfb)
set -eu
ME=$(cd "$(dirname "$0")/.." && pwd); CLIENT=$1; HUBBIN=$2; ASSETS=$3
. "$ME/scripts/lib-bptmp.sh"; R=${4:-$(mktemp -d "$BP_TMP/icon-gallery-desk.XXXX")}
PORT=${GPUI_PORT:-3849}; DISP=${GPUI_DISP:-:68}; XV=${XVFB:-$HOME/.local/xvfb-root/usr/bin/Xvfb}
[ -x "$CLIENT" ] && [ -x "$HUBBIN" ] && [ -x "$XV" ] || { echo "need CLIENT_BIN, HUB_BIN and Xvfb"; exit 2; }
[ "$DISP" != ":0" ] && [ ! -e "/tmp/.X11-unix/X${DISP#:}" ] || { echo "display $DISP is in use (set GPUI_DISP)"; exit 2; }
mkdir -p "$R/home" "$R/proj" "$R/bin" "$R/fakehome/.cache" "$R/shots" "$R/run"; chmod 700 "$R/run"
for n in claude codex grok muse; do printf '#!/bin/sh\nexit 1\n' > "$R/bin/$n"; chmod +x "$R/bin/$n"; done
python3 - "$ME/native/host/src/present/badge.rs" "$R" <<'PY'
import json, sys, time
src, R = open(sys.argv[1]).read(), sys.argv[2]; now = int(time.time())
body = src.split('pub const ICONS: &[&str] = &[')[1].split('];')[0]
icons = [x.strip().strip('"') for x in ' '.join(l.split('//')[0] for l in body.split('\n')).split(',') if x.strip()]
tints = ["blue", "teal", "green", "lime", "amber", "orange", "red", "pink", "violet", "slate"]; ev = []
for i, n in enumerate(icons):
    ev += [{"$": "ProjectCreated", "id": f"g{i:02d}", "title": n, "root": f"{R}/proj", "at": now - 100 + i}, {"$": "SettingSet", "key": f"project.pick.g{i:02d}", "value": f"{n} {tints[i % 10]}", "at": now}]
ev.append({"$": "ProjectCreated", "id": "gbp", "title": "Backplane", "root": f"{R}/proj", "at": now})
open(f"{R}/home/events.jsonl", "w").write(''.join(json.dumps(e) + '\n' for e in ev)); print(f"{len(icons)} icons")
PY
echo '[{"wait":6000},{"action":["look","g00"]},{"wait":40000}]' > "$R/script.json"
PIDS=; cleanup() { for p in $PIDS; do kill "$p" 2>/dev/null || true; done; }; trap cleanup EXIT INT TERM
unset I3SOCK DBUS_SESSION_BUS_ADDRESS
export XDG_RUNTIME_DIR="$R/run" LC_ALL=C BACKPLANE_AGENTS="$R/bin" BACKPLANE_NO_UPDATE=1 BACKPLANE_SAFE=1 HOME="$R/fakehome" XDG_CACHE_HOME="$R/fakehome/.cache" XDG_CONFIG_HOME="$R/fakehome/.config" XDG_DATA_HOME="$R/fakehome/.local/share"
cd "$R/proj"
"$HUBBIN" --home "$R/home" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground > "$R/hub.log" 2>&1 & H=$!; PIDS="$H"; sleep 4
"$XV" "$DISP" -screen 0 1100x1400x24 -nolisten tcp -xkbdir /usr/share/X11/xkb > "$R/xvfb.log" 2>&1 & V=$!; PIDS="$V $PIDS"; sleep 2
printf 'font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\nhide_edge_borders both\nipc-socket %s/i3.sock\n' "$R" > "$R/i3.conf"
DISPLAY="$DISP" i3 -c "$R/i3.conf" > "$R/i3.log" 2>&1 & I=$!; PIDS="$I $PIDS"; sleep 1
export DISPLAY="$DISP" VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json LP_NUM_THREADS=2 BACKPLANE_GPUI_ASSETS="$ASSETS"
"$CLIENT" --url "http://127.0.0.1:$PORT" --token-file "$R/home/token" --script "$R/script.json" --exit-after 60 > "$R/client.log" 2>&1 & C=$!; PIDS="$C $PIDS"
sleep 5; import -window root "$R/shots/sidebar.png"; sleep 6; import -window root "$R/shots/editor.png"
miss=$(grep -c '"asset_missing":"icon' "$R/client.log" || true)
echo "shots: $R/shots"; [ "$miss" = 0 ] && echo "ALL PASS: no icon missing" || { grep '"asset_missing":"icon' "$R/client.log"; exit 1; }
