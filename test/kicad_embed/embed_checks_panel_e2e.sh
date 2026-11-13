#!/bin/bash
# Suite 8: the Checks panel cross-probes into KiCad (native/gpui/src/screens/mgmt_checks.rs, native/host/src/kicad/checkrun.rs).
#   P1  the Checks tab under the embedded PCB editor opens the panel; its rows are the hub's findings (DRC and ERC)
#   P2  a real click on a DRC row selects its items in the PCB editor (GetSelection over KiCad's IPC); click -> selected in ms
#   P3  a click on an ERC row switches the editor to the schematic and selects the label there
#   P4  Down moves to the next row and cross-probes it as it goes
#   P5  "Fix with agent" sends the finding's message to the thread (the stand-in agent gets it)
# Copies only (seed_project.py), on a private Xvfb + i3. EMBED_MODE=fork (bp.9+: SelectItems).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "${1:-}" != "" ] && CLIENT_BIN=$1
[ "${2:-}" != "" ] && HUB_BIN=$2
[ "${3:-}" != "" ] && OUT=$(mkdir -p "$3" && cd "$3" && pwd)
if [ "$EMBED_MODE" != "fork" ]; then echo "SKIP embed_checks_panel_e2e: EMBED_MODE=fork only (SelectItems)"; exit 0; fi
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PROJ="$OUT/proj"; PRO="$PROJ/ecc83-pp.kicad_pro"
python3 - "$PROJ/ecc83-pp.kicad_sch" <<'PY'
import sys
p = sys.argv[1]; s = open(p).read()
lab = '\t(label "DANGLE"\n\t\t(at 30.48 30.48 0)\n\t\t(effects\n\t\t\t(font\n\t\t\t\t(size 1.27 1.27)\n\t\t\t)\n\t\t\t(justify left bottom)\n\t\t)\n\t\t(uuid "5b0c1e6e-0000-4000-8000-00000000d00d")\n\t)\n'
open(p, "w").write(s.replace("\t(sheet_instances", lab + "\t(sheet_instances", 1))
PY
( cd "$PROJ" && git add -A >/dev/null && git commit -qm "a dangling label" >/dev/null ) || true
start_xvfb_and_wm
start_hub

cat >"$OUT/script.json" <<'EOF2'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":1500},{"click":"ui:Edit"},{"kicad":{"wait_painted":45000,"kind":"PCB","tag":"pcb_init"}},
 {"wait":1500},{"click":"mgmt:checks"},{"wait":6000},{"mark":"panel"},{"wait":120000}]
EOF2
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_HOLD=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 220 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"panel","event":"mark"' 240 || { echo "FAIL the editor and panel never came up"; tail -n 30 "$OUT/client.log"; exit 1; }
R_INIT=$(last_kicad_report "$OUT/client.log" "pcb_init")
read -r KP API_SOCK GPUI_XID < <(python3 - "$R_INIT" <<'PY'
import json, sys
r = json.loads(sys.argv[1])
print(int(r.get("pid") or 0), r.get("sock") or "-", "0x%x" % int(r.get("toplevel") or 0))
PY
)
[ "$KP" -gt 0 ] && track_pid "$KP"
wait_for_ipc "$API_SOCK" 80

# the window's place on the screen, and a drawn rectangle's centre in screen pixels ("" when not drawn)
read -r GX GY < <(python3 "$DRIVER_PY" list-windows | python3 -c 'import json,sys; g=int(sys.argv[1],0); w=[x for x in json.load(sys.stdin) if x["xid_int"]==g]; print(w[0]["root_x"], w[0]["root_y"]) if w else print(0, 0)' "$GPUI_XID")
rect_xy() { python3 - "$OUT/client.log" "$1" "$GX" "$GY" <<'PY'
import json, sys
log, want, gx, gy = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4]); f = None
# the newest "controls" metric that names it (the controls on screen, with their rectangles: window pixels)
for line in open(log, errors="replace"):
    if "GPUI_METRIC " not in line or want not in line: continue
    try: v = json.loads(line.split("GPUI_METRIC ", 1)[1])
    except Exception: continue
    if v.get("event") != "controls": continue
    for c in v.get("controls", []):
        if isinstance(c, dict) and c.get("id") == want and c.get("rect"): f = c["rect"]
if f: print(int(gx + f["x"] + min(f["w"] / 2, 60)), int(gy + f["y"] + f["h"] / 2))
PY
}
panel() { hub_rpc "[{\"method\":\"mgmt.watch\",\"params\":{\"project\":\"$PRO\"}}]" | grep -v '^INFO ' | head -1 | python3 -c 'import json,sys; print(json.dumps(((json.load(sys.stdin).get("result") or {}).get("data") or {}).get("checks") or {}))'; }
for _ in $(seq 60); do P=$(panel); python3 -c 'import json,sys; d=json.loads(sys.argv[1]); sys.exit(0 if d.get("drc",{}).get("state")=="fresh" and d.get("erc",{}).get("state")=="fresh" else 1)' "$P" && break; sleep 1; done
check "The Checks panel's data is the hub's, both checks fresh (chip: $(python3 -c 'import json,sys; print(json.loads(sys.argv[1]).get("chip",""))' "$P"))" "$(python3 -c 'import json,sys; d=json.loads(sys.argv[1]); print(int(d.get("drc",{}).get("state")=="fresh" and d.get("total",0)>0 and d.get("badge",{}).get("n",0)>0))' "$P")"
sleep 1.5; shot "p1_panel"
read -r DRC_ID DRC_REF ERC_ID < <(python3 - "$P" <<'PY'
import json, re, sys
d = json.loads(sys.argv[1]); rows = [r for g in d["groups"] for r in g["rows"][:3]]  # a shut group shows 3 (SHUT_ROWS)
drc = next(r for r in rows if r["source"] == "drc" and re.search(r"\b[A-Z]{1,4}[0-9]+\b", r["names"]))
erc = next(r for r in rows if r["source"] == "erc" and r["rule"] == "label_dangling")
print(drc["id"], re.search(r"\b([A-Z]{1,4}[0-9]+)\b", drc["names"]).group(1), erc["id"])
PY
)
echo "rows: drc $DRC_ID ($DRC_REF), erc $ERC_ID"
check "The DRC and ERC rows are drawn" "$( [ -n "$(rect_xy "mgmt:checks:row:$DRC_ID")" ] && [ -n "$(rect_xy "mgmt:checks:row:$ERC_ID")" ] && echo 1 || echo 0)"

# click a row and time until KiCad's selection holds the part (one IPC client polling every 10 ms)
click_and_wait() { python3 - "$KEMBED_LIB" "$API_SOCK" "$2" "$3" "$XINPUT_PY" $1 <<'PY'
import importlib.util, json, re, subprocess, sys, time
lib, sock, kind, want, xin, x, y = sys.argv[1:8]
spec = importlib.util.spec_from_file_location("g", lib + "/gui_ipc_probe.py"); g = importlib.util.module_from_spec(spec); spec.loader.exec_module(g)
cli = g.KiCadIpc(sock)
def sel():
    _, val = cli.call("kiapi.common.commands.GetSelection", g.pb_field_msg(1, g.pb_field_msg(1, cli.first_open_doc_bytes(kind))), retries=1)
    refs, n = set(), 0
    def walk(buf, depth):
        if depth > 12: return
        try: fields = g.pb_decode_fields(buf)
        except Exception: return
        for vals in fields.values():
            for v in vals:
                if not isinstance(v, (bytes, bytearray)): continue
                try:
                    t = v.decode("utf-8")
                    if re.fullmatch(r"[A-Z]{1,6}[0-9]{1,6}[A-Z]?", t): refs.add(t)
                except UnicodeDecodeError: pass
                walk(v, depth + 1)
    for item in g.pb_decode_fields(val).get(1, []): n += 1; walk(item, 0)
    return {"count": n, "refs": sorted(refs)}
subprocess.run([sys.executable, xin, "click", x, y], check=True); t1 = time.monotonic(); s = {}
while time.monotonic() - t1 < 6:
    try: s = sel()
    except Exception as e: s = {"error": str(e)}
    if (want and want in s.get("refs", [])) or (not want and s.get("count", 0) > 0):
        print(json.dumps({"ok": True, "ms": int((time.monotonic() - t1) * 1000), "sel": s})); sys.exit(0)
    time.sleep(0.005)
print(json.dumps({"ok": False, "sel": s}))
PY
}
python3 "$IPC_PY" run-action "$API_SOCK" "common.Interactive.selectionClear" >/dev/null 2>&1 || true
XY=$(rect_xy "mgmt:checks:row:$DRC_ID")
R2=$(click_and_wait "$XY" pcb "$DRC_REF")
check "Clicking the DRC row selected $DRC_REF in the PCB editor ($R2)" "$(python3 -c 'import json,sys; print(int(json.loads(sys.argv[1])["ok"]))' "$R2")"
sleep 1; shot "p2_drc_row"

# P3: the ERC row: the schematic comes up with the label selected
XY=$(rect_xy "mgmt:checks:row:$ERC_ID")
R3=$(click_and_wait "$XY" sch "")
sleep 1.5
ACTIVE=$(hub_rpc "[]" | grep '^INFO ' | grep '"key":"editor.kicad.' | tail -1 | cut -c6- | python3 -c 'import json,sys; v=json.load(sys.stdin).get("value") or {}; print(v.get("active",""))' 2>/dev/null || echo "")
check "Clicking the ERC row showed the schematic ($ACTIVE) with the label selected ($R3)" "$(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); print(int(r["ok"] and sys.argv[2]=="SCH"))' "$R3" "$ACTIVE")"
shot "p3_erc_row"

# P4: Down from the DRC row goes to the next row and probes it
XY=$(rect_xy "mgmt:checks:row:$DRC_ID"); python3 "$XINPUT_PY" click $XY; sleep 1.2
L0=$(wc -l <"$OUT/client.log")
python3 "$XINPUT_PY" key Down; sleep 1.2
NEXT=$(tail -n +"$L0" "$OUT/client.log" | grep 'GPUI_LOG' | grep '"checks_select"' | tail -1 | sed 's/.*GPUI_LOG //')
check "Down moved to the next row and cross-probed it ($NEXT)" "$(python3 -c 'import json,sys; v=json.loads(sys.argv[1] or "{}"); print(int(bool(v.get("id")) and v.get("id")!=sys.argv[2]))' "$NEXT" "$DRC_ID")"

# P5: Fix with agent: the stand-in gets the finding's message
XY=$(rect_xy "mgmt:checks:row:$ERC_ID")
python3 "$XINPUT_PY" move $XY; sleep 0.6
FXY=$(rect_xy "mgmt:checks:fix:$ERC_ID")
[ -n "$FXY" ] && python3 "$XINPUT_PY" click $FXY
GOT=0; for _ in $(seq 40); do grep -q "$ERC_ID" "$OUT/standin.log" 2>/dev/null && { GOT=1; break; }; grep -rqs "$ERC_ID" "$OUT/home/events.jsonl" && { GOT=1; break; }; sleep 0.5; done
check "Fix with agent sent the finding ($ERC_ID) to the thread's agent" "$GOT"
shot "p5_fix"
finish_suite "embed_checks_panel_e2e ($EMBED_MODE)"
