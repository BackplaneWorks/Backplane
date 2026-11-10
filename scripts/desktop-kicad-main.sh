#!/bin/sh
# Design files on main (docs/git.md), end to end on the desktop: a copy of KiCad's pic_programmer demo with a firmware/ tree beside it, in
# a git repository with a bare origin; the Rust hub on a temp home with stand-in agents, the GPUI client in a private Xvfb with its own i3.
#   thread A: a turn that moves a footprint ("edit:") and writes README.md ("write:") in its folder; Open in KiCad on the board it shows
#   thread B: takes the board's lock and holds its turn; thread C: a files_write of that board while B holds it (refused, naming B)
#   thread D: takes the lock and holds; the hub is killed (-9) mid-turn and started again: the lock is cleared once D is idle
# Judged by test/e2e/kicad_main.py from the hub's log, the repository and the origin; shots in OUT/shots.
# usage: desktop-kicad-main.sh CLIENT_BIN HUB_BIN ASSETS_DIR [OUT]   env: GPUI_PORT (3851), GPUI_DISP (:71), XVFB, KICAD_DEMO (the demo folder), KICAD_CLI
set -eu
ME=$(cd "$(dirname "$0")/.." && pwd); CLIENT=$1; HUBBIN=$2; ASSETS=$3; R=${4:-$(mktemp -d /tmp/kicad-main.XXXX)}
PORT=${GPUI_PORT:-3851}; DISP=${GPUI_DISP:-:71}; XV=${XVFB:-$HOME/.local/xvfb-root/usr/bin/Xvfb}
DEMO=${KICAD_DEMO:-$(ls -d "$HOME"/.backplane-bend/kicad/*/share/kicad/demos/pic_programmer 2>/dev/null | tail -1)}; CLI=${KICAD_CLI:-$HOME/.local/bin/kicad-cli}
[ -x "$CLIENT" ] && [ -x "$HUBBIN" ] && [ -x "$XV" ] && [ -d "$DEMO" ] || { echo "need CLIENT_BIN, HUB_BIN, Xvfb and the KiCad demo"; exit 2; }
[ ! -e "/tmp/.X11-unix/X${DISP#:}" ] || { echo "display $DISP is in use (set GPUI_DISP)"; exit 2; }
mkdir -p "$R/home" "$R/proj" "$R/bin" "$R/fakehome/.cache" "$R/shots" "$R/run"; chmod 700 "$R/run"
# the project: the board in the root, firmware beside it, one commit, pushed to a bare origin it tracks
cp -r "$DEMO"/. "$R/proj/"; mkdir -p "$R/proj/firmware/src"; printf 'int main(void) { return 0; }\n' > "$R/proj/firmware/src/main.c"; printf 'project(fw C)\n' > "$R/proj/firmware/CMakeLists.txt"; printf '# pic programmer\n' > "$R/proj/README.md"
G() { git -C "$R/proj" -c user.name=rig -c user.email=rig@example.invalid "$@"; }
G init -q -b main; G config user.name rig; G config user.email rig@example.invalid; G add -A; G commit -qm base
git init -q --bare -b main "$R/origin.git"; G remote add origin "$R/origin.git"; G push -q -u origin main
BUN=$(command -v bun || echo "$HOME/.bun/bin/bun"); [ -x "$BUN" ] || { echo "need bun for the stand-in agent"; exit 2; }
printf '#!/bin/sh\nexec %s %s "$@"\n' "$BUN" "$ME/test/tools/claude_standin.ts" > "$R/bin/claude"; chmod +x "$R/bin/claude"
for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > "$R/bin/$n"; chmod +x "$R/bin/$n"; done
BOARD="$R/proj/pic_programmer.kicad_pcb"
LOCK=$(python3 "$ME/test/e2e/kicad_main.py" lock "$BOARD")
NOW=$(date +%s)
printf '{"$":"ProjectCreated","id":"p1","title":"pic","root":"%s","at":%s}\n{"$":"SettingSet","key":"kicad.path","value":"%s","at":%s}\n' "$R/proj" "$NOW" "$CLI" "$NOW" > "$R/home/events.jsonl"
HOLD_B='hold:25000 mcpcall:{"name":"lock_acquire","arguments":{"name":"'$LOCK'","note":"B edits the board"}}'
WRITE_C='mcpcall:{"name":"files_write","arguments":{"path":"pic_programmer.kicad_pcb","text":"(clobbered by C)"}}'
HOLD_D='hold:120000 mcpcall:{"name":"lock_acquire","arguments":{"name":"'$LOCK'","note":"D, about to crash"}}'
python3 - "$R/script.json" "$HOLD_B" "$WRITE_C" "$HOLD_D" <<'EOF'
import json, sys
out, hb, wc, hd = sys.argv[1:]
def thread(text, wait): return [{"action": ["new-thread", "p1"]}, {"wait": 6000}, {"select": 0}, {"wait": 1500}, {"type": text}, {"send": True}, {"wait": wait}]
s = [{"wait": 5000}] + thread("edit: write: a", 9000) + [{"view": "board"}, {"wait": 5000}, {"mark": "a-board"}, {"click": "ui:Open in KiCad"}, {"wait": 5000}, {"mark": "a-opened"}]
s += thread(hb, 4000) + thread(wc, 6000) + [{"mark": "c-refused"}, {"wait": 20000}] + thread(hd, 6000) + [{"mark": "d-holding"}, {"wait": 240000}]
json.dump(s, open(out, "w"))
EOF
PIDS=; cleanup() { for p in $PIDS; do kill "$p" 2>/dev/null || true; done; }; trap cleanup EXIT INT TERM
unset I3SOCK DBUS_SESSION_BUS_ADDRESS
export XDG_RUNTIME_DIR="$R/run" LC_ALL=C BACKPLANE_AGENTS="$R/bin" BACKPLANE_NO_UPDATE=1 BP_STANDIN_MODE=stream BP_STREAM_N=2 BP_STREAM_MS=50 HOME="$R/fakehome" XDG_CACHE_HOME="$R/fakehome/.cache" XDG_CONFIG_HOME="$R/fakehome/.config" XDG_DATA_HOME="$R/fakehome/.local/share"
cd "$R/proj"
hub() { "$HUBBIN" --home "$R/home" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground >> "$R/hub.log" 2>&1 & H=$!; PIDS="$H $PIDS"; }
hub; sleep 4
"$XV" "$DISP" -screen 0 1440x900x24 -nolisten tcp -xkbdir /usr/share/X11/xkb > "$R/xvfb.log" 2>&1 & V=$!; PIDS="$V $PIDS"; sleep 2
printf 'font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\nhide_edge_borders both\nipc-socket %s/i3.sock\n' "$R" > "$R/i3.conf"
DISPLAY="$DISP" i3 -c "$R/i3.conf" > "$R/i3.log" 2>&1 & I=$!; PIDS="$I $PIDS"; sleep 1
export DISPLAY="$DISP" VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json LP_NUM_THREADS=2 BACKPLANE_GPUI_ASSETS="$ASSETS"
"$CLIENT" --url "http://127.0.0.1:$PORT" --token-file "$R/home/token" --script "$R/script.json" --exit-after 420 > "$R/client.log" 2>&1 & C=$!; PIDS="$C $PIDS"
shot() { import -window root "$R/shots/$1.png" 2>/dev/null || true; }
# shots at the script's marks, by time (the client logs each mark)
seen=; t=0
while [ $t -lt 200 ]; do
  for m in a-board a-opened c-refused d-holding; do case " $seen " in *" $m "*) continue;; esac; grep -q "\"$m\"" "$R/client.log" 2>/dev/null && { sleep 1; shot "$m"; seen="$seen $m"; }; done
  case " $seen " in *" d-holding "*) break;; esac; sleep 1; t=$((t+1))
done
# D holds the board's lock in a running turn: the hub dies with it, then comes back
t=0; until python3 "$ME/test/e2e/kicad_main.py" held "$R/home/events.jsonl" "$LOCK" "D, about to crash" || [ $t -ge 60 ]; do sleep 1; t=$((t+1)); done
kill -9 "$H"; pkill -9 -f "$R/bin/claude" 2>/dev/null || true; echo "CRASH $(date +%s)" >> "$R/hub.log"; sleep 2
hub; sleep 90; shot after-crash
python3 "$ME/test/e2e/kicad_main.py" judge "$R" "$LOCK"
