#!/bin/bash
# Suite: the management panels beside the design (native/host/src/kicad/{mgmt,bompanel}.rs, native/gpui/src/screens/{mgmt,mgmt_bom}.rs).
#   M1  on the Board viewer the strip shows BOM; a press opens the BOM panel with the schematic's lines (R1, C1, U1...), the hub's state says so
#   M2  a press on a line selects its parts: the viewer's probe lights them (bom_select, probe_sync)
#   M3  docking at the bottom is the hub's per-project state, kept in <home>/mgmt/panels.json
#   M4  (stock|fork) in the embedded editor, opening the panel shrinks KiCad's container once (one geometry change), and KiCad stays painted
# Copies only (seed_project.py), on a private Xvfb + i3. EMBED_MODE=spike runs M1-M3 only.
set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"

[ "${1:-}" != "" ] && CLIENT_BIN=$1
[ "${2:-}" != "" ] && HUB_BIN=$2
[ "${3:-}" != "" ] && OUT=$(mkdir -p "$3" && cd "$3" && pwd)

discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
[ "$EMBED_MODE" != "spike" ] && discover_kicad
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
start_xvfb_and_wm
start_hub
PROJ="$OUT/proj"
PRO="$PROJ/ecc83-pp.kicad_pro"
mkdir -p "$OUT/shots"

# The last GPUI_LOG / GPUI_METRIC object with this event (after line $2 of the client log), as JSON ("{}" for none)
last_ev() {
  python3 - "$OUT/client.log" "$1" "${2:-0}" <<'PY'
import json, sys
log, ev, after = sys.argv[1], sys.argv[2], int(sys.argv[3])
found = {}
for i, line in enumerate(open(log, encoding="utf-8", errors="replace")):
    if i < after: continue
    for tag in ("GPUI_LOG ", "GPUI_METRIC "):
        if tag in line:
            try: o = json.loads(line.split(tag, 1)[1])
            except Exception: continue
            if o.get("event") == ev: found = o
print(json.dumps(found))
PY
}
lines() { wc -l <"$OUT/client.log"; }
# the hub's info under a key prefix, as the JSON value it holds ("{}" for none); the raw answers are kept in rpc.log
mgmt_info() { hub_rpc "[{\"method\":\"mgmt.watch\",\"params\":{\"project\":\"$PRO\"}}]" >"$OUT/rpc.out" 2>&1 || true; cat "$OUT/rpc.out" >>"$OUT/rpc.log"
  python3 - "$OUT/rpc.out" "$1" <<'PY'
import json, sys
found = {}
for line in open(sys.argv[1], encoding="utf-8", errors="replace"):
    try: o = json.loads(line)
    except Exception: continue
    r = o.get("result") or {}
    found = r.get("state", {}) if sys.argv[2] == "state" else (r.get("data") or {}).get(sys.argv[2], {})
print(json.dumps(found))
PY
}
has_ctl() { python3 -c 'import json,sys; c=json.loads(sys.argv[1]).get("controls",[]); print(int(any((x.get("id") if isinstance(x,dict) else x)==sys.argv[2] for x in c)))' "$(last_ev controls)" "$1" 2>/dev/null || echo 0; }

cat >"$OUT/script.json" <<'EOF'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":2500},{"mark":"board"},{"wait":800},
 {"click":"mgmt:bom"},{"wait":2000},{"mark":"bom_open"},{"wait":1500},
 {"click":"mgmt:bom:row:R1"},{"wait":2500},{"mark":"row_pressed"},{"wait":1200},
 {"click":"mgmt:bottom"},{"wait":2000},{"mark":"docked"},{"wait":1500}]
EOF
if [ "$EMBED_MODE" != "spike" ]; then
  python3 - "$OUT/script.json" <<'PY'
import json, sys
p = sys.argv[1]; s = json.load(open(p))
s += [{"click": "mgmt:right"}, {"wait": 1500}, {"click": "mgmt:bom"}, {"wait": 1500}, {"click": "ui:Edit"},
      {"kicad": {"wait_painted": 45000, "kind": "PCB", "tag": "pcb_init"}}, {"wait": 1500}, {"mark": "edit_ready"}, {"wait": 2500},
      {"click": "mgmt:bom"}, {"wait": 2500}, {"kicad": {"report": True, "tag": "with_panel"}}, {"mark": "edit_panel"}, {"wait": 2500}]
json.dump(s, open(p, "w"))
PY
fi
HOME="$OUT/fh" \
BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" \
BACKPLANE_GPUI_RECTS=1 \
BACKPLANE_GPUI_HOLD=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 240 >"$OUT/client.log" 2>&1 &
CP=$!
track_pid "$CP"

# M1: the strip and the BOM panel
wait_for_log "$OUT/client.log" '"name":"board","event":"mark"' 120 || { echo "FAIL the Board page never came up"; tail -n 30 "$OUT/client.log"; exit 1; }
check "The strip under the Board viewer has a BOM tab" "$(has_ctl mgmt:bom)" "$(last_ev controls | head -c 400)"
wait_for_log "$OUT/client.log" '"name":"bom_open","event":"mark"' 60 || true
L1=$(lines)
sleep 0.5; shot "m1_bom_open"
ST=$(mgmt_info state)
check "The hub's state has the BOM panel open ($ST)" "$(python3 -c 'import json,sys; v=json.loads(sys.argv[1]); print(int(v.get("open")=="bom" and v.get("side")=="right"))' "$ST" 2>/dev/null || echo 0)"
BOM=$(mgmt_info bom)
check "The hub published the schematic's BOM lines" "$(python3 -c 'import json,sys; v=json.loads(sys.argv[1]); ids=[l["id"] for l in v.get("lines",[])]; print(int(v.get("source")=="file" and "R1" in ids and "C1" in ids and "U1" in ids))' "$BOM" 2>/dev/null || echo 0)" "$(echo "$BOM" | head -c 300)"
check "The panel draws a row per line (R1's row is a control)" "$(has_ctl mgmt:bom:row:R1)"

# M2: a line pressed lights its parts in the viewer
wait_for_log "$OUT/client.log" '"name":"row_pressed","event":"mark"' 60 || true
SEL=$(last_ev bom_select "$L1"); PS=$(last_ev probe_sync "$L1")
check "Pressing the R1 line selected R1 and the viewer's probe lit it ($SEL $PS)" "$(python3 -c 'import json,sys; s=json.loads(sys.argv[1]); p=json.loads(sys.argv[2]); print(int("R1" in s.get("refs",[]) and "R1" in p.get("probe","")))' "$SEL" "$PS")"
shot "m2_row_pressed"

# M3: docked at the bottom, kept by the hub
wait_for_log "$OUT/client.log" '"name":"docked","event":"mark"' 60 || true
sleep 0.5; shot "m3_bottom"
SAVED=$(cat "$OUT/home/mgmt/panels.json" 2>/dev/null || echo '{}')
check "The bottom dock is the hub's per-project state ($SAVED)" "$(python3 -c 'import json,sys; m=json.loads(sys.argv[1]); print(int(any(v.get("side")=="bottom" and v.get("open")=="bom" and v.get("project","").endswith("ecc83-pp.kicad_pro") for v in m.values())))' "$SAVED")"

# M4: beside the embedded editor
if [ "$EMBED_MODE" != "spike" ]; then
  wait_for_log "$OUT/client.log" '"name":"edit_ready","event":"mark"' 240 || { echo "FAIL the editor never came up"; tail -n 30 "$OUT/client.log"; }
  R0=$(last_kicad_report "$OUT/client.log" "pcb_init")
  read -r KP API_SOCK < <(python3 -c 'import json,sys; r=json.loads(sys.argv[1] or "{}"); print(int(r.get("pid") or 0), r.get("sock") or "-")' "$R0")
  [ "${KP:-0}" -gt 0 ] && track_pid "$KP"
  CONT=$(python3 -c 'import json,sys; print(f"0x{int(((json.loads(sys.argv[1] or "{}").get("container") or {}).get("xid") or 0)):x}")' "$R0")
  # KiCad's container sampled every 25 ms from before the press to after it: the widths it went through
  python3 - "$CONT" "$OUT/geo.txt" <<'PY' &
import subprocess, sys, time
xid, out = sys.argv[1], sys.argv[2]
t0 = time.time(); seen = []
while time.time() - t0 < 6.5:
    o = subprocess.run(["xwininfo", "-id", xid], capture_output=True, text=True).stdout
    w = next((l.split(":")[1].strip() for l in o.splitlines() if l.strip().startswith("Width:")), "")
    h = next((l.split(":")[1].strip() for l in o.splitlines() if l.strip().startswith("Height:")), "")
    if w and (not seen or seen[-1] != (w, h)): seen.append((w, h))
    time.sleep(0.025)
open(out, "w").write(" ".join(f"{w}x{h}" for w, h in seen))
PY
  SAMPLER=$!
  shot "m4a_editor"
  wait_for_log "$OUT/client.log" '"name":"edit_panel","event":"mark"' 60 || true
  sleep 0.5; shot "m4b_editor_panel"
  wait "$SAMPLER" 2>/dev/null || true
  R1=$(last_kicad_report "$OUT/client.log" "with_panel")
  GEO=$(cat "$OUT/geo.txt" 2>/dev/null || echo "")
  W0=$(python3 -c 'import json,sys; print(int(((json.loads(sys.argv[1] or "{}").get("container") or {}).get("w") or 0)))' "$R0")
  W1=$(python3 -c 'import json,sys; print(int(((json.loads(sys.argv[1] or "{}").get("container") or {}).get("w") or 0)))' "$R1")
  check "Opening the panel made KiCad's container narrower ($W0 -> $W1 px)" "$([ "$W1" -gt 0 ] && [ "$W1" -lt "$W0" ] && echo 1 || echo 0)"
  check "KiCad's container changed size once when the panel opened (sizes seen: $GEO)" "$(python3 -c 'import sys; print(int(len(sys.argv[1].split())==2))' "$GEO")"
  hub_rpc "[{\"method\":\"editor.kicad.close\",\"params\":{\"project\":\"$PRO\",\"save\":\"discard\"}}]" >/dev/null || true
fi

finish_suite "embed_mgmt_e2e ($EMBED_MODE)"
