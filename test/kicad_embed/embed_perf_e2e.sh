#!/bin/bash
# Timings (stock|fork; KEMBED_PROJ_SRC for a real project's copy): Edit's cold start, then each tab switch (the click to the editor
# shown and drawn: Board, Schematic, 3D, back), and a save (Ctrl+S pressed in KiCad to the file written, KiCad clean and answering).
# Prints PERF lines; checks the budgets below (KEMBED_PERF_STRICT=0 only prints).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "spike" ] && { echo "embed_perf_e2e: stock|fork only"; exit 0; }
discover_bins; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
start_xvfb_and_wm; start_hub
python3 - "$OUT/script.json" <<'PY'
import json, sys
s = [{"select": 0}, {"wait": 2200}, {"click": "rail:Board"}, {"wait": 2500}, {"click": "ui:Edit"}, {"kicad": {"wait_painted": 60000, "kind": "PCB", "tag": "cold"}}, {"wait": 4000}]
for i, k in enumerate(["SCH", "PCB_3D", "PCB", "SCH", "PCB", "PCB_3D", "SCH"]):
    s += [{"click": f"kicad-tab:{k}"}, {"kicad": {"wait_painted": 30000, "kind": k, "tag": f"sw{i}_{k}"}}, {"wait": 1500}]
s += [{"click": "kicad-tab:PCB"}, {"kicad": {"wait_painted": 30000, "kind": "PCB", "tag": "save_ready"}}, {"mark": "save"}, {"wait": 25000}, {"click": "ui:View"}, {"wait": 1000}]
json.dump(s, open(sys.argv[1], "w"))
PY
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 200 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
for _ in $(seq 1500); do grep -q '"name":"save","event":"mark"' "$OUT/client.log" 2>/dev/null && break; kill -0 "$CP" 2>/dev/null || break; sleep 0.1; done
R=$(last_kicad_report "$OUT/client.log" save_ready)
read -r SOCK X Y W H < <(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); f=next((f for f in r.get("frames",[]) if f.get("xid")==r.get("shown")),{}); print(r.get("sock") or "-", f.get("root_x",0), f.get("root_y",0), f.get("w",0), f.get("h",0))' "$R")
PCB=$(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); print(next((f.get("doc","") for f in r.get("frames",[]) if f.get("kind")=="PCB"), ""))' "$R")
[ -n "$PCB" ] || PCB=$(find "$OUT/proj" -name "*.kicad_pcb" -not -path "*/.git/*" | head -1 || true)
# an edit (an agent's track over IPC) so there is something to save, then the person's Ctrl+S with the keyboard in KiCad
python3 "$IPC_PY" agent-add-track "$SOCK" 100 100 110 100 0.25 >/dev/null 2>&1 || true; sleep 1
python3 "$XINPUT_PY" click $((X + W * 3 / 4)) $((Y + H - 40)); sleep 0.5
M0=$(stat -c %.6Y "$PCB")
T0=$(date +%s.%N)
python3 "$XINPUT_PY" chord ctrl s
python3 - "$PCB" "$M0" "$T0" "$SOCK" "$IPC_PY" <<'PY'
import os, subprocess, sys, time, json
pcb, m0, t0, sock, ipc = sys.argv[1], float(sys.argv[2]), float(sys.argv[3]), sys.argv[4], sys.argv[5]
written = clean = None
while time.time() - t0 < 30:
    now = time.time() - t0
    if written is None and os.path.exists(pcb) and os.stat(pcb).st_mtime > m0: written = now
    if written is not None:
        out = subprocess.run(["python3", ipc, "modified", sock, "pcb"], capture_output=True, text=True).stdout
        try:
            if json.loads(out).get("modified") is False: clean = time.time() - t0; break
        except Exception: pass
    time.sleep(0.02)
print(f"PERF save_written_ms={int((written or -1)*1000)} save_clean_ms={int((clean or -1)*1000)}")
# how long KiCad answers a Ping, every 200 ms for 6 s after the save: a stall shows as a slow answer
pings = []
t1 = time.time()
while time.time() - t1 < 6:
    a = time.time(); subprocess.run(["python3", ipc, "ping", sock], capture_output=True); pings.append(int((time.time() - a) * 1000)); time.sleep(0.2)
print("PERF save_ping_ms " + " ".join(map(str, pings)))
PY
wait "$CP" 2>/dev/null || true
python3 - "$OUT/client.log" <<'PY'
import json, sys
for l in open(sys.argv[1], errors="ignore"):
    if '"event":"kicad"' in l and '"wait_ms"' in l:
        r = json.loads(l[l.index("{"):]); print(f"PERF {r.get('tag')} {r.get('wait_ms')}ms state={r.get('state')}")
PY
finish_suite "embed_perf_e2e ($EMBED_MODE)"
