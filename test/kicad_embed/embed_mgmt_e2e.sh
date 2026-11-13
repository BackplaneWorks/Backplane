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
# MPNs the stock stand-in knows: R1 in stock, C1 out of it, C2 low (test/kicad_embed/lib/pcbparts_stub.py); no lookup leaves the machine
python3 - "$OUT/proj/ecc83-pp.kicad_sch" <<'PY'
import sys
p = sys.argv[1]; s = open(p, encoding="utf-8").read()
for ref, mpn in (("R1", "E2E-FILE"), ("C1", "E2E-LIVE"), ("C2", "E2E-LOW")):
    k = s.index(f'(property "Reference" "{ref}"')
    s = s[:k] + f'(property "MPN" "{mpn}" (at 0 0 0) (effects (font (size 1.27 1.27)) (hide yes))) ' + s[k:]
open(p, "w", encoding="utf-8").write(s)
PY
git -C "$OUT/proj" commit -qam "MPNs for the stock stand-in" 2>/dev/null || true
# three earlier days of the three parts in the history store, so the graphs have a line to draw (the store is the hub's own, under its home)
python3 - "$OUT/home/parts/history" <<'PY'
import json, os, sys, time
d = sys.argv[1]; os.makedirs(d, exist_ok=True)
now = int(time.time())
for key, fab, rows in (("mpn:E2E-FILE", "C1001", [(7000, 0.012), (6200, 0.011), (5600, 0.010)]), ("mpn:E2E-LIVE", "C1002", [(40, 1.3), (12, 1.25), (0, 1.2)]), ("mpn:E2E-LOW", "C1003", [(9, 0.6), (5, 0.55), (3, 0.5)])):
    h = 0xcbf29ce484222325
    for b in key.encode(): h = ((h ^ b) * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
    with open(os.path.join(d, f"{h:016x}.jsonl"), "w") as f:
        for i, (stock, price) in enumerate(rows):
            f.write(json.dumps({"at": now - (3 - i) * 86400, "key": key, "stock": stock, "class": "basic", "price": price, "breaks": [[1, price]], "lifecycle": "", "fab": fab, "model": key[4:]}) + "\n")
PY
export BACKPLANE_PARTS_LIFE_GAP_S=3 BACKPLANE_PARTS_GAP_MS=500
STUB_PORT=$(free_port)
own python3 "$KEMBED_LIB/pcbparts_stub.py" "$STUB_PORT" "$OUT/pcbparts.log"
export BACKPLANE_PCBPARTS_URL="http://127.0.0.1:$STUB_PORT/mcp"
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
 {"click":"mgmt:bottom"},{"wait":2000},{"mark":"docked"},{"wait":1500},
 {"mgmt":"bom:row:C1"},{"wait":300},{"mgmt":"bom:alts"},{"wait":3000},{"mark":"alts"},{"wait":800},
 {"click":"mgmt:right"},{"wait":800},{"click":"mgmt:life"},{"wait":6000},{"mgmt":"life:part:mpn:E2E-FILE"},{"wait":2500},{"mark":"life"},{"wait":1500}]
EOF
if [ "$EMBED_MODE" != "spike" ]; then
  python3 - "$OUT/script.json" <<'PY'
import json, sys
p = sys.argv[1]; s = json.load(open(p))
s += [{"click": "mgmt:bom"}, {"wait": 1000}, {"mgmt": "bom:row:R3"}, {"wait": 300}, {"mgmt": "bom:set:mpn=E2E-FILE"}, {"wait": 9000}, {"mark": "file_edit"}, {"wait": 500},
      {"click": "mgmt:bom"}, {"wait": 1500}, {"click": "ui:Edit"},
      {"kicad": {"wait_painted": 45000, "kind": "PCB", "tag": "pcb_init"}}, {"wait": 1500}, {"mark": "edit_ready"}, {"wait": 2500},
      {"click": "mgmt:bom"}, {"wait": 2500}, {"kicad": {"report": True, "tag": "with_panel"}}, {"mark": "edit_panel"}, {"wait": 2500},
      {"click": "kicad-tab:SCH"}, {"kicad": {"wait_painted": 45000, "kind": "SCH", "tag": "sch_init"}}, {"wait": 3000}, {"mark": "sch_ready"}, {"wait": 6000},
      {"mgmt": "bom:row:R4"}, {"wait": 300}, {"mgmt": "bom:set:mpn=E2E-UNDO"}, {"wait": 3000}, {"mark": "live_edit"}, {"wait": 12000},
      {"mgmt": "bom:undo"}, {"wait": 3000}, {"mark": "panel_undo"}, {"wait": 5000}, {"mgmt": "bom:set:mpn=E2E-UNDO"}, {"wait": 3000}, {"mark": "final"}, {"wait": 8000}]
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

# M7: each line's part in the fab's stock (the stand-in), cached and asked once each; the badge counts what cannot be ordered
for _ in $(seq 80); do B7=$(mgmt_info bom); python3 -c 'import json,sys; v=json.loads(sys.argv[1]); sys.exit(0 if v.get("checking",1)==0 and v.get("lookups",0)>=3 else 1)' "$B7" && break; sleep 0.5; done
check "Stock per line from the fab's stock: R1 in, C1 out, C2 low, with class and price" "$(python3 -c '
import json,sys; v=json.loads(sys.argv[1]); st={r:l.get("stock",{}) for l in v.get("lines",[]) for r in l["refs"]}
print(int(st["R1"].get("status")=="in" and st["R1"].get("class")=="basic" and st["R1"].get("price")==0.01 and st["C1"].get("status")=="out" and st["C2"].get("status")=="low" and v["badge"]["tone"]=="bad"))' "$B7" 2>/dev/null || echo 0)" "$(echo "$B7" | head -c 600)"
check "Each part was asked once (cached), never per keystroke ($(wc -l <"$OUT/pcbparts.log") calls)" "$(python3 -c '
import json,sys,collections; c=collections.Counter(json.dumps(json.loads(l),sort_keys=True) for l in open(sys.argv[1])); print(int(len(c)>0 and max(c.values())==1))' "$OUT/pcbparts.log" 2>/dev/null || echo 0)"
check "Each answer is a sample in the parts history store" "$([ "$(ls "$OUT/home/parts/history" 2>/dev/null | wc -l)" -ge 3 ] && echo 1 || echo 0)"
shot "m7_stock"

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

# M8: alternatives on demand for the out-of-stock line
wait_for_log "$OUT/client.log" '"name":"alts","event":"mark"' 60 || true
A8=$(last_ev bom_alts)
check "Alternatives for C1 came from the fab's stock on demand ($A8)" "$(python3 -c 'import json,sys; a=json.loads(sys.argv[1]); print(int(a.get("n")==2 and a.get("id")=="C1"))' "$A8")"
shot "m8_alts"

# M9: lifecycle: NRND and EOL from the distributors' readings; the BOM's cost and a part's stock over time from the history store
wait_for_log "$OUT/client.log" '"name":"life","event":"mark"' 60 || true
for _ in $(seq 20); do L9=$(mgmt_info life); python3 -c 'import json,sys; v=json.loads(sys.argv[1]); sys.exit(0 if v.get("counts",{}).get("nrnd",0)>=1 else 1)' "$L9" && break; sleep 0.5; done
check "Lifecycle per part: C1 NRND, C2 EOL (the distributors' words), the tab's badge counts them" "$(python3 -c '
import json,sys; v=json.loads(sys.argv[1]); c=v.get("counts",{}); print(int(c.get("nrnd")==1 and c.get("eol")==1 and v["badge"]["n"]==2 and v["badge"]["tone"]=="bad"))' "$L9" 2>/dev/null || echo 0)" "$(echo "$L9" | head -c 500)"
check "The whole BOM over time: a point a day from the history (cost a board)" "$(python3 -c 'import json,sys; v=json.loads(sys.argv[1]); print(int(len(v["bom"]["cost"])>=4))' "$L9" 2>/dev/null || echo 0)"
P9=$(last_ev life_part)
check "A part's own history on demand ($P9)" "$(python3 -c 'import json,sys; p=json.loads(sys.argv[1]); print(int(p.get("n",0)>=4))' "$P9")"
shot "m9_life"
B9=$(mgmt_info bom)
check "Each BOM line says its library class, lifecycle and stock with when each was checked (what the design review reads)" "$(python3 -c '
import json,sys; v=json.loads(sys.argv[1]); by={r:l for l in v.get("lines",[]) for r in l["refs"]}
c1, r1 = by["C1"], by["R1"]
print(int(c1.get("lifecycle",{}).get("status")=="nrnd" and c1["lifecycle"].get("checked",0)>0 and r1.get("library",{}).get("class")=="basic" and r1["library"].get("checked",0)>0 and r1["stock"].get("checked",0)>0 and "lifecycle" in by["C2"]))' "$B9" 2>/dev/null || echo 0)" "$(echo "$B9" | head -c 300)"
check "The fab's recorded stock changes are in C2's part history from before today" "$(python3 -c '
import json,sys,glob
n=0
for f in glob.glob(sys.argv[1]+"/*.jsonl"):
    for l in open(f):
        o=json.loads(l)
        if o.get("key")=="mpn:E2E-LOW" and o.get("source")=="fab_history" and o.get("stock") in (400,90): n+=1
print(int(n==2))' "$OUT/home/parts/history")"

# M4: beside the embedded editor
if [ "$EMBED_MODE" != "spike" ]; then
  # M6a: with no KiCad open, the panel's edit is written to the schematic file (headless KiCad, holding the schematic's lock)
  wait_for_log "$OUT/client.log" '"name":"file_edit","event":"mark"' 90 || true
  check "With KiCad closed, an MPN typed in the panel is saved in the schematic file" "$(grep -q '(property "MPN" "E2E-FILE"' "$PROJ/ecc83-pp.kicad_sch" && echo 1 || echo 0)" "$(last_ev bom_edited)"
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
  # M5: with the schematic open in KiCad, the BOM is read from it (live), not the file
  wait_for_log "$OUT/client.log" '"name":"sch_ready","event":"mark"' 90 || true
  for _ in $(seq 20); do BOM5=$(mgmt_info bom); python3 -c 'import json,sys; sys.exit(0 if json.loads(sys.argv[1]).get("source")=="kicad" else 1)' "$BOM5" && break; sleep 0.5; done
  check "With the schematic open in KiCad the BOM comes live from it" "$(python3 -c 'import json,sys; v=json.loads(sys.argv[1]); ids=[l["id"] for l in v.get("lines",[])]; print(int(v.get("source")=="kicad" and "R1" in ids and "U1" in ids))' "$BOM5" 2>/dev/null || echo 0)" "$(echo "$BOM5" | head -c 400)"
  shot "m5_live"

  # M6: an MPN typed in the panel. With KiCad up it is a commit in KiCad (in its undo, unsaved); before, it went to the file (headless, under the lock)
  wait_for_log "$OUT/client.log" '"name":"live_edit","event":"mark"' 60 || true
  shot "m6_live_edit"
  mpn_of() { python3 -c 'import json,sys; v=json.loads(sys.argv[1]); print(next((l.get("mpn","") for l in v.get("lines",[]) if sys.argv[2] in l.get("refs",[])), "?"))' "$(mgmt_info bom)" "$1"; }
  E1=$(last_ev bom_edit 0)
  check "The panel sent the R4 MPN edit ($E1)" "$(python3 -c 'import json,sys; e=json.loads(sys.argv[1]); print(int(e.get("field")=="mpn" and e.get("value")=="E2E-UNDO"))' "$E1")"
  M_LIVE=""; for _ in $(seq 20); do M_LIVE=$(mpn_of R4); [ "$M_LIVE" = "E2E-UNDO" ] && break; sleep 0.5; done
  check "KiCad holds the new MPN and the BOM shows it before any save (R4: $M_LIVE)" "$([ "$M_LIVE" = "E2E-UNDO" ] && echo 1 || echo 0)"
  check "The schematic file does not have it yet (KiCad saves it)" "$(grep -q 'E2E-UNDO' "$PROJ/ecc83-pp.kicad_sch" && echo 0 || echo 1)"
  ED=$(last_ev bom_edited); MODE=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1]).get("undo") or "")' "$ED")
  check "The edit went into the open KiCad and the hub says who takes it back ($ED)" "$(python3 -c 'import json,sys; e=json.loads(sys.argv[1]); print(int(e.get("ok") is True and e.get("via")=="kicad" and e.get("undo") in ("kicad","panel") and e.get("was")==""))' "$ED")"
  # the person's Ctrl+Z in KiCad: a click on the sheet's empty lower part gives KiCad the keyboard, Escape clears what it picked
  read -r CX CY CW CH < <(xwininfo -id "$CONT" | awk -F: '/Absolute upper-left X/{x=$2}/Absolute upper-left Y/{y=$2}/Width/{w=$2}/Height/{h=$2}END{print x+0, y+0, w+0, h+0}')
  python3 "$XINPUT_PY" click $((CX + CW / 2)) $((CY + CH * 9 / 10)); sleep 0.5; python3 "$XINPUT_PY" key Escape; sleep 0.3
  python3 "$XINPUT_PY" chord ctrl z >"$OUT/undo.json" 2>&1 || true
  M_UNDO=""; for _ in $(seq 20); do M_UNDO=$(mpn_of R4); [ "$M_UNDO" != "E2E-UNDO" ] && break; sleep 0.5; done
  UNDO_OK=$([ "$M_UNDO" != "E2E-UNDO" ] && [ "$M_UNDO" != "?" ] && echo 1 || echo 0)
  if [ "$MODE" = kicad ]; then
    # a runtime that puts the hub's (the person's) edits in their undo (bp.13, person: client)
    check "KiCad's Ctrl+Z takes the panel's edit back (R4's MPN now '$M_UNDO')" "$UNDO_OK"
    python3 "$XINPUT_PY" chord ctrl y >/dev/null 2>&1 || true
  else
    echo "KNOWN KiCad's Ctrl+Z is not the person's for the hub's edits before bp.13 (R4: $M_UNDO); the panel offers its own Undo"
    # the panel's own Undo: the old value written back (the script pressed it after this)
    wait_for_log "$OUT/client.log" '"name":"panel_undo","event":"mark"' 60 || true
    M_PU=""; for _ in $(seq 20); do M_PU=$(mpn_of R4); [ "$M_PU" != "E2E-UNDO" ] && break; sleep 0.5; done
    check "The panel's Undo put R4's MPN back (now '$M_PU')" "$([ "$M_PU" = "" ] && echo 1 || echo 0)"
  fi
  wait_for_log "$OUT/client.log" '"name":"final","event":"mark"' 60 || true
  sleep 1.5
  python3 "$IPC_PY" save-doc "$API_SOCK" sch >/dev/null 2>&1 || true
  for _ in $(seq 20); do grep -q 'E2E-UNDO' "$PROJ/ecc83-pp.kicad_sch" && break; sleep 0.5; done
  check "Saved in KiCad, the MPN is in the schematic file as a standard field" "$(grep -q '(property "MPN" "E2E-UNDO"' "$PROJ/ecc83-pp.kicad_sch" && echo 1 || echo 0)"
  shot "m6_saved"
  hub_rpc "[{\"method\":\"editor.kicad.close\",\"params\":{\"project\":\"$PRO\",\"save\":\"discard\"}}]" >/dev/null || true
fi

finish_suite "embed_mgmt_e2e ($EMBED_MODE)"
