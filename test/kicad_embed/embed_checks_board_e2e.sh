#!/bin/bash
# The Checks panel on a real board (a COPY: CHECKS_BOARD=<folder holding one .kicad_pro>, e.g. a copy of Hubble_DK): the counts, the run's time,
# screenshots, and the frame times while the list scrolls with a run coming in (the perf overlay's numbers). Measurement, not a gate: it
# fails only when the panel never shows the board's findings. EMBED_MODE=fork.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ -n "${CHECKS_BOARD:-}" ] && [ -d "$CHECKS_BOARD" ] || { echo "SKIP embed_checks_board_e2e: set CHECKS_BOARD to a copy of a project folder"; exit 0; }
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PROJ="$OUT/proj"
# the board in place of the fixture (its files only: no backups, outputs or lock files)
( cd "$PROJ" && git rm -q -r --cached . >/dev/null && find . -mindepth 1 -maxdepth 1 ! -name .git -exec rm -rf {} + )
rsync -a --exclude '*backups*' --exclude 'build' --exclude 'mfg' --exclude 'docs' --exclude '~*.lck' --exclude '.git' "$CHECKS_BOARD"/ "$PROJ"/
PRO=$(ls "$PROJ"/*.kicad_pro | head -1)
echo "{\"project\": \"$(basename "$PRO")\"}" >"$PROJ/.backplane.json"
( cd "$PROJ" && git add -A >/dev/null && git commit -qm board >/dev/null )
start_xvfb_and_wm
start_hub
python3 - "$OUT/script.json" <<'PY'
import json, sys
s = [{"select": 0}, {"wait": 2000}, {"click": "rail:Board"}, {"wait": 1500}, {"click": "ui:Edit"}, {"kicad": {"wait_painted": 90000, "kind": "PCB", "tag": "pcb_init"}},
     {"wait": 1500}, {"click": "mgmt:checks"}, {"wait": 12000}, {"mark": "panel"}, {"wait": 3000},
     # the biggest group opened (its hundreds of rows), then a screen down the list every 80 ms with a run coming in
     {"click": "mgmt:checks:more:drc:unconnected_items:false"}, {"wait": 2500}, {"mark": "opened"}]
for i in range(60):
    s.append({"click": "mgmt:checks:scroll"}); s.append({"wait": 80})
    if i == 15: s.append({"click": "mgmt:checks:run"})
s += [{"read": "perf"}, {"mark": "measured"}, {"wait": 60000}]
json.dump(s, open(sys.argv[1], "w"))
PY
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_HOLD=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 260 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"panel","event":"mark"' 240 || { echo "FAIL the editor and panel never came up"; tail -n 30 "$OUT/client.log"; exit 1; }
R_INIT=$(last_kicad_report "$OUT/client.log" "pcb_init")
read -r KP GPUI_XID < <(python3 - "$R_INIT" <<'PY'
import json, sys
r = json.loads(sys.argv[1]); print(int(r.get("pid") or 0), "0x%x" % int(r.get("toplevel") or 0))
PY
)
[ "$KP" -gt 0 ] && track_pid "$KP"
read -r GX GY < <(python3 "$DRIVER_PY" list-windows | python3 -c 'import json,sys; g=int(sys.argv[1],0); w=[x for x in json.load(sys.stdin) if x["xid_int"]==g]; print(w[0]["root_x"], w[0]["root_y"]) if w else print(0, 0)' "$GPUI_XID")
ctl_xy() { python3 - "$OUT/client.log" "$1" "$GX" "$GY" <<'PY'
import json, sys
log, want, gx, gy = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4]); f = None
for line in open(log, errors="replace"):
    if "GPUI_METRIC " not in line or want not in line: continue
    try: v = json.loads(line.split("GPUI_METRIC ", 1)[1])
    except Exception: continue
    for c in v.get("controls", []) if v.get("event") == "controls" else []:
        if isinstance(c, dict) and c.get("id", "").startswith(want) and c.get("rect"): f = c["rect"]
if f: print(int(gx + f["x"] + min(f["w"] / 2, 60)), int(gy + f["y"] + f["h"] / 2))
PY
}
S=$(hub_rpc "[{\"method\":\"kicad.checks.watch\",\"params\":{\"project\":\"$PRO\"}}]" | grep -v '^INFO ' | head -1)
python3 -c 'import json,sys; r=json.loads(sys.argv[1])["result"]
for k in ("drc","erc"): x=r[k]; print(k, x["state"], x["counts"], "took", x["took_ms"], "ms", "peak", x["peak_mb"], "MB")
print("chip:", r["chip"])' "$S" | tee "$OUT/counts.txt"
shot "b1_panel"
# the perf overlay on (the script then opens the biggest group and scrolls the list while a run comes in)
python3 "$XINPUT_PY" chord ctrl shift h
wait_for_log "$OUT/client.log" '"name":"opened","event":"mark"' 60 || true
shot "b1b_opened"
wait_for_log "$OUT/client.log" '"name":"measured","event":"mark"' 60 || true
shot "b2_scrolled"
python3 - "$OUT/client.log" <<'PY' | tee "$OUT/perf.txt"
import json, sys
t = ""
for line in open(sys.argv[1], errors="replace"):
    if "GPUI_METRIC " in line and '"read"' in line:
        try: v = json.loads(line.split("GPUI_METRIC ", 1)[1])
        except Exception: continue
        if v.get("what") == "perf": t = v.get("text", "")
print(t or "no perf read")
PY
check "The panel showed the board's findings" "$(python3 -c 'import json,sys; r=json.loads(sys.argv[1])["result"]; print(int(r["drc"]["has_result"]))' "$S")"
finish_suite "embed_checks_board_e2e ($EMBED_MODE)"
