#!/bin/bash
# A Board<->Schematic switch at an unchanged pane rect is a show of the other frame and nothing else (Linux, fork, the client mapping
# KiCad's frames): no placement that reads as a change of geometry and no synthetic ConfigureNotify (rail-round4: three same-rect
# placements and three ConfigureNotify per switch). And KiCad still knows where its frame is on screen: a context menu opens at the
# pointer after the switches, and again after Backplane's window has moved and the editors were switched once more.
#   G1  across two switches (Board -> Schematic -> Board) at an unchanged rect, after a first round that shows the schematic once (its first
#       show is told its place, rightly): no `kicad_place` with "changed": true, no `kicad_place_notify`
#   G2  a right-click in the board after the switches opens KiCad's context menu at the pointer
#   G3  Backplane's window moved, two more switches: the context menu still opens at the pointer (the frame was told its new place)
# BACKPLANE_KICAD_DEBUG=1 (the lines are debug-only). Copies only (seed_project.py), fork mode, X11.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_switch_geometry_e2e: fork only"; exit 0; }
discover_bins; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
export BACKPLANE_KICAD_DEBUG=1
start_xvfb_and_wm; start_hub
python3 - "$OUT/script.json" <<'PY'
import json, sys
sw = lambda tag: [{"click": "kicad-tab:SCH"}, {"kicad": {"wait_painted": 30000, "kind": "SCH", "tag": f"{tag}_sch"}}, {"wait": 2500},
                  {"click": "kicad-tab:PCB"}, {"kicad": {"wait_painted": 30000, "kind": "PCB", "tag": f"{tag}_pcb"}}, {"wait": 2500}]
s = [{"select": 0}, {"wait": 2200}, {"click": "rail:Board"}, {"wait": 2500}, {"click": "ui:Edit"}, {"kicad": {"wait_painted": 60000, "kind": "PCB", "tag": "edit"}},
     {"wait": 5000}] + sw("w") + [{"mark": "switches"}] + sw("a") + [{"kicad": {"report": True, "tag": "switched"}}, {"mark": "switched"}, {"wait": 8000},
     {"mark": "move"}, {"wait": 5000}, {"mark": "moved"}] + sw("b") + [{"kicad": {"report": True, "tag": "switched2"}}, {"mark": "switched2"}, {"wait": 8000}, {"mark": "done"}, {"wait": 2000}]
json.dump(s, open(sys.argv[1], "w"))
PY
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 200 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
mark() { for _ in $(seq 1500); do grep -q "\"name\":\"$1\",\"event\":\"mark\"" "$OUT/client.log" 2>/dev/null && return 0; kill -0 "$CP" 2>/dev/null || return 1; sleep 0.1; done; return 1; }

# G1: the lines between the two marks (the client log's own order)
mark switched || true
read -r CHANGED NOTIFY SHOWN < <(python3 - "$OUT/client.log" <<'PY'
import json, sys
on = False; changed = notify = shown = 0
for l in open(sys.argv[1], errors="ignore"):
    if '"name":"switches","event":"mark"' in l: on = True; continue
    if '"name":"switched","event":"mark"' in l: break
    if not on or "GPUI_LOG " not in l: continue
    try: o = json.loads(l.split("GPUI_LOG ", 1)[1])
    except Exception: continue
    if "kicad_place" in o:
        shown += 1
        if o.get("changed") is True: changed += 1
    if "kicad_place_notify" in o: notify += 1
print(changed, notify, shown)
PY
)
echo "INFO G1 placements $SHOWN, changed:true $CHANGED, notify $NOTIFY"
check "G1 two switches at an unchanged rect: no placement changes geometry, no synthetic ConfigureNotify" "$([ "${SHOWN:-0}" -ge 1 ] && [ "${CHANGED:-1}" = 0 ] && [ "${NOTIFY:-1}" = 0 ] && echo 1 || echo 0)" "placements ${SHOWN:-?} (expected at least one), changed:true ${CHANGED:-?}, notify ${NOTIFY:-?}"

# G2/G3: a right-click in the board opens KiCad's context menu (an override-redirect window of KiCad's) near the pointer
menu_at() { # menu_at TAG: right-click the middle of the shown frame; prints "dx dy" of the menu from the pointer, or "none"
  local r; r=$(last_kicad_report "$OUT/client.log" "$1")
  python3 - "$r" "$KEMBED_LIB" "$XINPUT_PY" <<'PY'
import json, subprocess, sys, time
r, lib_dir, xin = json.loads(sys.argv[1] or "{}"), sys.argv[2], sys.argv[3]
sys.path.insert(0, lib_dir); import x11_embed_driver as d
f = next((f for f in r.get("frames", []) if f.get("xid") == r.get("shown")), {})
pid = r.get("pid") or 0
x, y = f.get("root_x", 0) + f.get("w", 0) // 2, f.get("root_y", 0) + f.get("h", 0) // 2
if not pid or not f.get("w"): print("none"); sys.exit()
lib = d.load_lib()
before = {w["xid"] for w in d.list_all_windows(lib) if w["shown"]}
subprocess.run(["python3", xin, "move", str(x), str(y)]); time.sleep(0.3)
subprocess.run(["python3", xin, "click", str(x), str(y), "3"])
menu = None
for _ in range(40):
    time.sleep(0.1)
    for w in d.list_all_windows(lib):
        if w["shown"] and w["override_redirect"] and w["pid"] == pid and w["w"] >= 40 and w["h"] >= 40 and w["xid"] not in before: menu = w; break
    if menu: break
subprocess.run(["python3", xin, "key", "Escape"]); time.sleep(0.3)
print("none" if not menu else f'{menu["root_x"] - x} {menu["root_y"] - y} {menu["h"]}')
PY
}
# at the pointer: its left edge at the pointer's x, and the pointer's y within its height (GTK opens it downwards, or moves a tall one up
# to fit the screen); a stale place is off by the distance the frame moved (G3 moves it by 160, 90), or by the pane's own offset
near() { python3 -c 'import sys; a=sys.argv[1].split(); sys.exit(0 if len(a) == 3 and abs(int(a[0])) <= 48 and -int(a[2]) - 48 <= int(a[1]) <= 48 else 1)' "$1"; }
M1=$(menu_at switched 2>/dev/null || echo none); echo "INFO G2 menu offset from the pointer: $M1"
check "G2 after the switches a context menu opens at the pointer" "$(near "$M1" && echo 1 || echo 0)" "offset x y, height: $M1 (x within 48 px, the pointer within its height)"
mark move || true
wm_moveresize 160 90 >"$OUT/wm_move.json" 2>&1 || true
mark switched2 || true
M2=$(menu_at switched2 2>/dev/null || echo none); echo "INFO G3 menu offset from the pointer after the window moved: $M2"
check "G3 Backplane's window moved and two more switches: the context menu opens at the pointer" "$(near "$M2" && echo 1 || echo 0)" "offset x y, height: $M2 (x within 48 px, the pointer within its height)"
grep -o '"kicad_place_notify":[^}]*' "$OUT/client.log" | head -20 > "$OUT/notify.txt" || true
wait "$CP" 2>/dev/null || true
finish_suite "embed_switch_geometry_e2e ($EMBED_MODE)"
