#!/bin/bash
# The edit-keys table (lib/edit_keys_table.json, one definition with lane M's macOS and Carbon's Windows tables) for Linux embedded:
# real X key events (XTest) with the keyboard in the embedded PCB canvas, then the schematic canvas: Ctrl+A, Ctrl+F, Ctrl+C/Ctrl+V,
# Ctrl+X/Ctrl+V, Delete, Ctrl+Z, Ctrl+Y, Ctrl+Shift+Z, BackSpace, Ctrl+S, per-actor undo, and undo with the checks' markers selected (the person's Ctrl+Z after an agent's commit
# through the hub's kicad_ipc takes back the person's edit, never the agent's). Judged from KiCad's own edit events, its selection, its
# windows and the files on disk (lib/edit_keys_drive.py), never pixels. Every row gates. Copies only (seed_project.py), fork mode.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_edit_keys_e2e: fork only"; exit 0; }
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PROJ="$OUT/proj"; PCB="$PROJ/ecc83-pp.kicad_pcb"; SCH="$PROJ/ecc83-pp.kicad_sch"; mkdir -p "$OUT/shots"
KD="$HERE/lib/edit_keys_drive.py"
start_xvfb_and_wm
export BACKPLANE_KICAD_DEBUG=1 BP_STREAM_N=1 BP_STREAM_MS=10 BP_STANDIN_MCPCALL_MAX=200000
start_hub
cat >"$OUT/script.json" <<'JSON'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},
 {"kicad":{"wait_painted":60000,"kind":"PCB","tag":"ready"}},{"wait":1500},{"kicad":{"report":true,"tag":"placed"}},{"mark":"edit_ready"},
 {"wait":6900000}]
JSON
HOME="$OUT/fh" BACKPLANE_KICAD_DEBUG=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 \
  "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 7000 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"edit_ready","event":"mark"' 240 || { echo "FAIL the board never opened in KiCad"; exit 1; }
R=$(last_kicad_report "$OUT/client.log" placed)
read -r KPID SOCK < <(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); print(r.get("pid") or 0, r.get("sock") or "")' "$R")
sleep 3

# the client's controls now (the latest snapshot): this one's middle
ctl_at() { python3 - "$OUT/client.log" "$1" <<'PY'
import json, sys
last = None
for l in open(sys.argv[1], errors="ignore"):
    if "GPUI_METRIC " in l and '"controls":[' in l:
        try: last = json.loads(l.split("GPUI_METRIC ", 1)[1])["controls"]
        except Exception: pass
c = next((c for c in (last or []) if c.get("id") == sys.argv[2] and c.get("rect")), None)
print(f'{int(c["rect"]["x"] + c["rect"]["w"] / 2)} {int(c["rect"]["y"] + c["rect"]["h"] / 2)}' if c else "")
PY
}
# an agent's call to a hub tool, as the stand-in agent makes it (one turn per call)
ask() { local N; N="$1-$(date +%s%N)"; local before; before=$(grep -c "^MCPCALL" "$OUT/standin.log" 2>/dev/null || echo 0)
  local text; text=$(python3 -c 'import json,sys; print(json.dumps("mcpcall:" + json.dumps({"name": sys.argv[1], "arguments": json.loads(sys.argv[2])})))' "$1" "$2")
  hub_rpc "[{\"method\":\"turn.start\",\"params\":{\"thread\":\"t1\",\"text\":$text,\"msg\":\"mcl$N\"}}]" >>"$OUT/rpc.log" 2>&1 || true
  for _ in $(seq 240); do [ "$(grep -c "^MCPCALL" "$OUT/standin.log" 2>/dev/null || echo 0)" -gt "$before" ] && break; sleep 0.25; done; sleep 2
  grep "^MCPCALL $1 " "$OUT/standin.log" | tail -1 || true; }

ROWS="$OUT/rows.txt"; : >"$ROWS"
run_kd() { python3 "$KD" "$@" 2>&1 | tee -a "$ROWS" || true; }   # a driver's failure is its rows' (missing rows read "not run"), never the suite's end

# the PCB editor
export HPORT HUB_RPC_MJS BUN="$(command -v bun)"
if [ -n "${KEYS_PROBE:-}" ]; then  # a diagnosis only: redo, at once and after KiCad's checks, key and action; board then schematic
  for c in ${KEYS_PROBE_CMD:-redo-probe}; do run_kd "$c" pcb "$OUT" "$SOCK" "$KPID" "$PCB"; shot "probe-$c-pcb"; done
  [ -n "${KEYS_PROBE_PCB_ONLY:-}" ] && { kill "$CP" 2>/dev/null || true; wait "$CP" 2>/dev/null || true; exit 0; }
  AT=$(ctl_at kicad-tab:SCH); [ -n "$AT" ] && { python3 "$XINPUT_PY" click $AT; sleep 8; for c in ${KEYS_PROBE_CMD:-redo-probe}; do run_kd "$c" sch "$OUT" "$SOCK" "$KPID" "$SCH"; done; }
  kill "$CP" 2>/dev/null || true; wait "$CP" 2>/dev/null || true; exit 0
fi
# a freshly opened editor, no click on it at all (after its startup check): Ctrl+A, Ctrl+C; then an agent's check; then the redo
# rows: 10 cycles each (KEYS_CYCLES), keys only, with and without the keyboard's trip to Backplane's chat
run_kd fresh-keys pcb "$OUT" "$SOCK" "$KPID" "$PCB"
run_kd agent-check pcb "$OUT" "$SOCK" "$KPID" "$PCB"
run_kd keys-cycles pcb "$OUT" "$SOCK" "$KPID" "$PCB"
run_kd table pcb "$OUT" "$SOCK" "$KPID" "$PCB"; shot pcb_table
# per-actor undo: the person deletes everything, an agent commits a track through the hub (save: false), the person presses Ctrl+Z
S=$(run_kd actor-before pcb "$OUT" "$SOCK" "$KPID" | sed -n 's/^SEQ //p' | tail -1)
B=$(ask kicad_ipc "{\"path\":\"$PCB\",\"type\":\"kiapi.common.commands.BeginCommit\",\"message\":{\"header\":{\"document\":\"\$doc\"}}}")
CID=$(echo "$B" | sed 's/^MCPCALL kicad_ipc //' | python3 -c 'import json,sys
try: o=json.loads(sys.stdin.read()); m=json.loads(o["result"]["content"][0]["text"])["message"]; print(m["id"]["value"])
except Exception as e: print("")')
ask kicad_ipc "{\"path\":\"$PCB\",\"type\":\"kiapi.common.commands.CreateItems\",\"message\":{\"header\":{\"document\":\"\$doc\"},\"items\":[{\"@type\":\"type.googleapis.com/kiapi.board.types.Track\",\"start\":{\"x_nm\":120000000,\"y_nm\":100000000},\"end\":{\"x_nm\":140000000,\"y_nm\":100000000},\"width\":{\"value_nm\":300000},\"layer\":\"BL_F_Cu\"}]}}" >/dev/null
ask kicad_ipc "{\"path\":\"$PCB\",\"type\":\"kiapi.common.commands.EndCommit\",\"message\":{\"header\":{\"document\":\"\$doc\"},\"id\":{\"value\":\"$CID\"},\"action\":\"CMA_COMMIT\",\"message\":\"an agent's track\"},\"save\":false}" >/dev/null
echo "INFO the agent's commit through the hub: id ${CID:-none}"
sleep 1
run_kd actor-after pcb "$OUT" "$SOCK" "$KPID" "$PCB" "${S:-0}"; shot pcb_actor
# the undo-markers case (d2ce4ba5c6): select all with the check's markers, Delete, undo, redo, undo; a check and an agent read between
run_kd undo-markers pcb "$OUT" "$SOCK" "$KPID" "$PCB"; shot pcb_undo_markers
# typing in the Properties panel's field while an automatic check runs (R1's Value)
run_kd props-typing pcb "$OUT" "$SOCK" "$KPID" "$PCB"; shot pcb_props_typing

# the schematic editor (its tab in the pane)
AT=$(ctl_at kicad-tab:SCH)
if [ -n "$AT" ]; then
  python3 "$XINPUT_PY" click $AT
  for _ in $(seq 240); do python3 "$DRIVER_PY" list-windows --match "schematic editor" 2>/dev/null | python3 -c 'import json,sys; sys.exit(0 if any(w.get("shown") and w["w"]>300 for w in json.load(sys.stdin)) else 1)' && break; sleep 0.25; done
  sleep 6
  run_kd fresh-keys sch "$OUT" "$SOCK" "$KPID" "$SCH"
  run_kd agent-check sch "$OUT" "$SOCK" "$KPID" "$SCH"
  run_kd keys-cycles sch "$OUT" "$SOCK" "$KPID" "$SCH"
  run_kd table sch "$OUT" "$SOCK" "$KPID" "$SCH"; shot sch_table
  run_kd undo-markers sch "$OUT" "$SOCK" "$KPID" "$SCH"; shot sch_undo_markers
else
  echo "ROW sch all nothing the pane's Schematic tab (kicad-tab:SCH) was not found" | tee -a "$ROWS"
fi

# the table, and the gate: every row works
echo "=== edit keys, Linux embedded (works / nothing / wrong)"
python3 - "$ROWS" "$HERE/lib/edit_keys_table.json" <<'PY'
import json, sys
rows = {}
for l in open(sys.argv[1]):
    if l.startswith("ROW "):
        p = l.rstrip("\n").split(" ", 4); rows[(p[1], p[2])] = (p[3], p[4] if len(p) > 4 else "")
ids = [r["id"] for r in json.load(open(sys.argv[2]))["rows"]]
print(f'{"row":<16}{"PCB":<9}{"schematic":<9}')
for i in ids:
    pv = rows.get(("pcb", i), rows.get(("pcb", "all"), ("not run", "")))[0]
    sv = "n/a" if i in ("per_actor_undo", "props_typing") else rows.get(("sch", i), rows.get(("sch", "all"), ("not run", "")))[0]
    print(f"{i:<16}{pv:<9}{sv:<9}")
PY
while read -r _ ed id verdict evidence; do
  [ "$verdict" = info ] && { echo "INFO edit keys: $ed $id (informational, never gating): $evidence"; continue; }
  check "edit keys: $ed $id works" "$([ "$verdict" = works ] && echo 1 || echo 0)" "$verdict: $evidence"
  case "$verdict:$id" in nothing:undo_markers|nothing:undo|nothing:redo_after_*|nothing:fresh_*|nothing:props_typing|nothing:redo*|nothing:copy_paste|nothing:cut*|nothing:delete|nothing:backspace|nothing:save|nothing:all|wrong:*)
    echo "ALERT $ed $id $verdict (to the lead and the integrator): $evidence";; esac
done < <(grep '^ROW ' "$ROWS")
kill "$CP" 2>/dev/null || true; wait "$CP" 2>/dev/null || true
finish_suite "embed_edit_keys_e2e ($EMBED_MODE)"
