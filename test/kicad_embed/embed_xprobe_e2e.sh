#!/bin/bash
# Suite 4: cross-probing (native/host/src/kicad/xprobe.rs, native/gpui/src/screens/xprobe.rs).
#   X1  a seeded agent message's "U1" is a link (kicad.names); a real click on it selects U1 in the embedded PCB editor
#       (GetSelection over the GUI's IPC socket) and the view moves onto it (pixel diff of the editor)
#   X2  the agent tool path: editor.kicad.select over the hub's RPC answers what KiCad found
#   X3  (bp.9: SelectItems / ET_SELECTION_CHANGED) a person's click in KiCad's canvas becomes the hub's selection info and
#       Backplane's viewer probe: back on View, the Board viewer lights U1; on bp.8 it is reported as skipped
#   X4  in View mode (no editor in the pane), clicking "R1" in the message lights and zooms it in Backplane's own Board viewer
# Copies only (seed_project.py), on a private Xvfb + i3. EMBED_MODE=stock|fork.
set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"

[ "${1:-}" != "" ] && CLIENT_BIN=$1
[ "${2:-}" != "" ] && HUB_BIN=$2
[ "${3:-}" != "" ] && OUT=$(mkdir -p "$3" && cd "$3" && pwd)

if [ "$EMBED_MODE" = "spike" ]; then echo "SKIP embed_xprobe_e2e: product modes only (EMBED_MODE=stock|fork)"; exit 0; fi

discover_bins
discover_kicad
init_isolated_dirs
KEMBED_SEED_MESSAGE="The tube is U1; its plate resistor is R1 and C1 couples the output." python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
start_xvfb_and_wm
start_hub
PROJ="$OUT/proj"

# bp.9 has SelectItems and ET_SELECTION_CHANGED; bp.8 cross-probes one part through ShowFrame
DESC="$(dirname "$KICAD_BIN")/../share/backplane/api.desc"
BP9=0; [ -f "$DESC" ] && grep -aq "SelectItems" "$DESC" && BP9=1
echo "runtime: $KICAD_BIN bp9=$BP9"

cat >"$OUT/script.json" <<'EOF'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":1500},{"click":"ui:Edit"},{"kicad":{"wait_painted":45000,"kind":"PCB","tag":"pcb_init"}},{"mark":"host_ready"}]
EOF
HOME="$OUT/fh" \
BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" \
BACKPLANE_GPUI_RECTS=1 \
BACKPLANE_GPUI_HOLD=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 200 >"$OUT/client.log" 2>&1 &
CP=$!
track_pid "$CP"

wait_for_log "$OUT/client.log" '"name":"host_ready","event":"mark"' 240 || { echo "FAIL the editor never came up"; tail -n 30 "$OUT/client.log"; exit 1; }
R_INIT=$(last_kicad_report "$OUT/client.log" "pcb_init")
read -r KP API_SOCK GPUI_XID CONT_XID PCB_XID < <(python3 - "$R_INIT" <<'PY'
import json, sys
r = json.loads(sys.argv[1])
print(int(r.get("pid") or 0), r.get("sock") or "-", f"0x{int(r.get('toplevel') or 0):x}", f"0x{int((r.get('container') or {}).get('xid') or 0):x}", f"0x{int(r.get('shown') or 0):x}")
PY
)
[ "$KP" -gt 0 ] && track_pid "$KP"
wait_for_ipc "$API_SOCK" 80
for _ in $(seq 50); do python3 "$IPC_PY" open-docs "$API_SOCK" pcb 2>/dev/null | grep -q '"count": 1' && break; sleep 0.25; done

# The latest painted rectangle of a message link with this href, in root coordinates ("" when not drawn)
link_xy() {
  python3 - "$OUT/client.log" "$DRIVER_PY" "$GPUI_XID" "$1" <<'PY'
import json, subprocess, sys
log, drv, gx_s, href = sys.argv[1:5]
rect = None
for line in open(log, encoding="utf-8", errors="replace"):
    if "GPUI_METRIC " not in line or '"links"' not in line:
        continue
    try: o = json.loads(line.split("GPUI_METRIC ", 1)[1])
    except Exception: continue
    if o.get("event") != "links": continue
    rect = next((l["rect"] for l in o.get("links", []) if l.get("href") == href), None)
if not rect: sys.exit(0)
gid = int(gx_s, 0); gx = gy = 0
for w in json.loads(subprocess.check_output([sys.executable, drv, "list-windows"], text=True)):
    if w["xid_int"] == gid: gx, gy = w["root_x"], w["root_y"]; break
print(int(gx + rect["x"] + min(rect["w"] / 2, 8)), int(gy + rect["y"] + rect["h"] / 2))
PY
}
wait_link() { for _ in $(seq 80); do XY=$(link_xy "$1"); [ -n "$XY" ] && return 0; sleep 0.25; done; return 1; }
# The last GPUI_LOG object with this event (after line $2 of the client log), as JSON ("{}" for none)
last_log() {
  python3 - "$OUT/client.log" "$1" "${2:-0}" <<'PY'
import json, sys
log, ev, after = sys.argv[1], sys.argv[2], int(sys.argv[3])
found = {}
for i, line in enumerate(open(log, encoding="utf-8", errors="replace")):
    if i < after or "GPUI_LOG " not in line: continue
    try: o = json.loads(line.split("GPUI_LOG ", 1)[1])
    except Exception: continue
    if o.get("event") == ev: found = o
print(json.dumps(found))
PY
}
lines() { wc -l <"$OUT/client.log"; }

RPT=$(python3 "$DRIVER_PY" report "$GPUI_XID" "$CONT_XID" "$PCB_XID")
read -r BX BY BW BH < <(python3 -c 'import json,sys; n=json.loads(sys.argv[1]).get("native",{}); print(n.get("root_x",800), n.get("root_y",200), n.get("w",600), n.get("h",600))' "$RPT")

# X1: the message's U1 is a link; a click on it selects U1 in KiCad and moves the view onto it
LINK_OK=0; wait_link "kicad:U1" && LINK_OK=1
check "The seeded message's U1 is drawn as a design link (kicad.names gave the board's parts)" "$LINK_OK" "$(grep -c '"links"' "$OUT/client.log") links metrics"
SEL0=$(python3 "$IPC_PY" selection "$API_SOCK" pcb 2>&1 || echo '{}')
# the board zoomed far out first, so the cross-probe's zoom onto U1 is plain to see
for _ in 1 2 3; do python3 "$IPC_PY" run-action "$API_SOCK" "common.Control.zoomOutCenter" >/dev/null 2>&1 || true; sleep 0.2; done
sleep 0.8
shot "x1a_before_click"
L0=$(lines)
if [ "$LINK_OK" = 1 ]; then
  # shellcheck disable=SC2086
  python3 "$XINPUT_PY" click $XY
fi
for _ in $(seq 40); do [ "$(last_log xprobe_select "$L0")" != "{}" ] && break; sleep 0.25; done
sleep 1.2
shot "x1b_after_click"
REPLY=$(last_log xprobe_select "$L0")
SEL1=$(python3 "$IPC_PY" selection "$API_SOCK" pcb 2>&1 || echo '{}')
SEL_OK=$(python3 -c 'import json,sys; s=json.loads(sys.argv[1]); print(int("U1" in s.get("refs",[])))' "$SEL1" 2>/dev/null || echo 0)
check "Clicking U1 in the chat selected U1 in the embedded PCB editor (GetSelection: $SEL1; before: $SEL0; hub: $REPLY)" "$SEL_OK"
DIFF=$(python3 "$DRIVER_PY" img-diff "$OUT/shots/x1a_before_click.png" "$OUT/shots/x1b_after_click.png" $((BX + 30)) $((BY + 60)) $((BW - 60)) $((BH - 120)))
ZOOM_OK=$(python3 -c 'import json,sys; print(int(json.loads(sys.argv[1])["changed_ratio"] > 0.03))' "$DIFF")
check "The editor's view moved onto U1 ($DIFF)" "$ZOOM_OK"

# X2: the same through the hub's RPC (the agent's kicad_select): what KiCad found and the viewer probe it set
R2=$(hub_rpc "[{\"method\":\"editor.kicad.select\",\"params\":{\"project\":\"$PROJ/ecc83-pp.kicad_pro\",\"references\":[\"C1\",\"Q99\"],\"zoom\":true}}]")
X2_OK=$(python3 - "$R2" <<'PY'
import json, sys
for line in sys.argv[1].splitlines():
    if not line or line.startswith("INFO "): continue
    d = json.loads(line); r = d.get("reply") or {}
    print(int(d.get("ok") is True and r.get("kicad") is True and "C1" in (r.get("found") or {}).get("references", []) and r.get("probe", "").startswith("footprint C1"))); break
else: print(0)
PY
)
check "editor.kicad.select over the hub found C1 in KiCad and set the viewers' probe" "$X2_OK" "$R2"
SEL2=$(python3 "$IPC_PY" selection "$API_SOCK" pcb 2>&1 || echo '{}')
check "KiCad's selection is now C1 ($SEL2)" "$(python3 -c 'import json,sys; print(int("C1" in json.loads(sys.argv[1]).get("refs",[])))' "$SEL2" 2>/dev/null || echo 0)"

# X3: a person's selection in KiCad reaches Backplane (bp.9)
if [ "$BP9" = 1 ]; then
  hub_rpc "[{\"method\":\"editor.kicad.select\",\"params\":{\"project\":\"$PROJ/ecc83-pp.kicad_pro\",\"references\":[\"U1\"],\"zoom\":true}}]" >/dev/null
  sleep 1.0
  # a person first clicks into KiCad (it takes the keyboard); Escape then clears KiCad's selection. Pressed while Backplane has the
  # keyboard, Escape is Backplane's (it leaves the editor), and the clicks after it never reach KiCad
  python3 "$XINPUT_PY" click $((BX + BW / 2)) $((BY + BH / 2)); sleep 0.6
  python3 "$XINPUT_PY" key Escape; sleep 0.3
  # the view is zoomed on U1: a click at its centre is the person selecting it
  python3 "$XINPUT_PY" click $((BX + BW / 2)) $((BY + BH / 2)); sleep 0.4
  python3 "$XINPUT_PY" click $((BX + BW / 2)) $((BY + BH / 2)); sleep 1.2
  INFO=$(hub_rpc "[]" | grep '^INFO ' | grep 'kicad.sel.' | tail -1 | cut -c6- || true)
  X3_OK=$(python3 -c 'import json,sys; v=(json.loads(sys.argv[1] or "{}").get("value") or {}); s=v.get("selection",{}); print(int(v.get("frame")=="PCB" and ("U1" in s.get("references",[]) or any(p.startswith("U1.") for p in s.get("pins",[]))) and "ref=U1" in v.get("probe","")))' "$INFO" 2>/dev/null || echo 0)
  check "The person's click on U1 in KiCad became the hub's selection (kicad.sel) and the viewers' probe" "$X3_OK" "$INFO"
else
  echo "SKIP KiCad -> Backplane selection: $KICAD_BIN has no ET_SELECTION_CHANGED (bp.9)"
fi

# A second window on the same hub, on Board with no editor in its pane (KiCad stays warm in the hub)
kill -TERM "$CP" 2>/dev/null || true; wait "$CP" 2>/dev/null || true
echo '[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":2500},{"mark":"view_ready"}]' >"$OUT/script2.json"
L3=$(lines)
HOME="$OUT/fh" \
BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" \
BACKPLANE_GPUI_RECTS=1 \
BACKPLANE_GPUI_HOLD=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script2.json" --exit-after 120 >>"$OUT/client.log" 2>&1 &
CP=$!
track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"view_ready","event":"mark"' 160 || true
GPUI_XID=$(python3 "$DRIVER_PY" list-windows --pid "$CP" | python3 -c 'import json,sys; w=sorted([x for x in json.load(sys.stdin) if x.get("shown")], key=lambda x: -x.get("w",0)*x.get("h",0)); print(w[0]["xid"] if w else "")')
echo "second window: $GPUI_XID"
if [ "$BP9" = 1 ]; then
  for _ in $(seq 40); do last_log probe_lit "$L3" | grep -q '"n"' && break; sleep 0.25; done
  P3=$(last_log probe_sync "$L3"); LIT3=$(last_log probe_lit "$L3")
  check "The Board viewer lights the part the person selected in KiCad ($P3 $LIT3)" "$(python3 -c 'import json,sys; p=json.loads(sys.argv[1]); l=json.loads(sys.argv[2]); print(int("ref=U1" in p.get("probe","") and l.get("n",0)>0))' "$P3" "$LIT3")"
  shot "x3_viewer_lit"
fi

# X4: View mode: a click on R1 in the chat lights and zooms it in Backplane's own Board viewer
sleep 1.5
L4=$(lines)
XY=""; wait_link "kicad:R1" || true
if [ -n "$XY" ]; then
  # shellcheck disable=SC2086
  python3 "$XINPUT_PY" click $XY
fi
for _ in $(seq 60); do [ "$(last_log xprobe_zoom "$L4")" != "{}" ] && break; sleep 0.25; done
sleep 1.0
Z4=$(last_log xprobe_zoom "$L4"); P4=$(last_log probe_sync "$L4"); LIT4=$(last_log probe_lit "$L4")
check "In View mode, clicking R1 in the chat lit it in Backplane's Board viewer and zoomed onto it ($Z4 $P4 $LIT4)" \
  "$(python3 -c 'import json,sys; z,p,l=(json.loads(a) for a in sys.argv[1:4]); print(int("ref=R1" in z.get("probe","") and "ref=R1" in p.get("probe","") and l.get("n",0)>0))' "$Z4" "$P4" "$LIT4")"
shot "x4_viewer_zoom"

hub_rpc "[{\"method\":\"editor.kicad.close\",\"params\":{\"project\":\"$PROJ/ecc83-pp.kicad_pro\",\"save\":\"discard\"}}]" >/dev/null || true
finish_suite "embed_xprobe_e2e ($EMBED_MODE)"
