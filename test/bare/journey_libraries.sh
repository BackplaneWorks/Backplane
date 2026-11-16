# Sourced by journey_project.sh after journey_more.sh: the libraries row on the clean box (Backplane 0.16.0's bar: "a clean install works
# end to end"). The row is test/kicad_embed/libraries_row.py with the shared design test/kicad_embed/lib/libgate_design.py (one definition for
# Linux, macOS and Windows: climb-j, Carbon); here it runs inside the box as the person, so every path it hands the hub's tools and its own
# kicad-cli is the one the hub sees. The stand-in agent (coedit_standin.py) makes its tool calls: `mcpcall:` turns.
#   - a new project (make_project: an empty schematic, an empty board with a 30 x 20 mm outline; the runtime's own template's header)
#     added to the hub with a thread, and a third session with its board in the pane (L7's editor checks need the board open in KiCad);
#   - phase a: L1-L7 and L9, each line of the row a check here (PASS, FAIL, or SKIP where it says so);
#   - phase b: L8, a person's own fp-lib-table (a custom entry, no Backplane entry) met by the hub's library-table sync (libtables.rs).
#     The hub syncs when it selects a runtime (at start, after an install, when Settings' KiCad path changes): here the path is set to the
#     managed runtime's own kicad-cli, the same runtime, which runs the sync without restarting the person's hub.
# rowchecks FILE PHASE RC: each line of the row is a check of this gate (INFO lines stay in its log)
rowchecks() { local l
  while IFS= read -r l; do
    case "$l" in
      PASS\ *) ok "libraries ${l#PASS }";;
      FAIL\ *) bad "libraries ${l#FAIL }"; cut_ 1 libraries "a clean install's libraries row: ${l#FAIL }";;
      SKIP\ *) printf 'SKIP\tlibraries %s\n' "${l#SKIP }" >>"$OUT/checks.tsv"; echo "SKIP libraries ${l#SKIP }";;
    esac
  done <"$1"
  grep -q "^libraries_row $2:" "$1" || bad "libraries row ($2): it ended without its summary (exit $3; $(tail -c 300 "$1"))"; }
phase "libraries row"
LIBR=$HERE/../kicad_embed
if [ ! -f "$LIBR/libraries_row.py" ] || [ ! -f "$LIBR/lib/libgate_design.py" ]; then
  printf 'SKIP\tlibraries row: this tree has no test/kicad_embed/libraries_row.py\n' >>"$OUT/checks.tsv"; echo "SKIP libraries row (not in this tree)"
else
# the row's files as the box sees them (/opt/standins is the harness's, read-only there): the test/ layout kept (hub_rpc.mjs imports ../../wire.mjs)
LG=$OUT/standins/libgate; mkdir -p "$LG/test/kicad_embed/lib"
cp "$LIBR/libraries_row.py" "$LG/test/kicad_embed/"; cp "$LIBR/lib/libgate_design.py" "$LIBR/lib/hub_rpc.mjs" "$LG/test/kicad_embed/lib/"
# hub_rpc.mjs -> test/wire.mjs -> test/wire-codec.js and ../native/host/src/wire_dict.rs (bare-gate.sh ships all of them)
cp "$HERE/../wire.mjs" "$HERE/../wire-codec.js" "$LG/test/"; mkdir -p "$LG/native/host/src"; cp "$HERE/../../native/host/src/wire_dict.rs" "$LG/native/host/src/"
cp "$(command -v bun)" "$OUT/standins/bin/bun"
LPROJ=$PH/Documents/libgate; HLPROJ=$HH/Documents/libgate
# the new project from the runtime's own template (its board header, so this KiCad reads its own format's layer table)
python3 - "$KDIR/share/kicad/template" "$HLPROJ" "$LIBR/lib" >"$OUT/logs/libgate-project.txt" 2>&1 <<'PY'
import os, sys
tpl, out, lib = sys.argv[1:]
sys.path.insert(0, lib); import libgate_design as D
s = os.path.join(tpl, "EuroCard160mmX100mm")
print(D.make_project(out, os.path.join(tpl, "kicad.kicad_pro"), os.path.join(s, "EuroCard160mmX100mm.kicad_pcb"), os.path.join(s, "EuroCard160mmX100mm.kicad_sch")))
open(os.path.join(out, ".backplane.json"), "w").write('{"project": "%s.kicad_pro"}\n' % D.NAME)
print(D.NAME)
PY
LNAME=$(tail -1 "$OUT/logs/libgate-project.txt")
hub rpc project.new "{\"path\":\"$LPROJ\",\"orAdd\":true}" >"$OUT/logs/libgate-project-new.json"
lpid=; for _ in $(seq 40); do lpid=$(ev ProjectCreated root "$LPROJ" | tail -1); [ -n "$lpid" ] && break; sleep 0.5; done
hub rpc thread.create "{\"project\":\"$lpid\",\"title\":\"Libraries\"}" >"$OUT/logs/libgate-thread.json"
ltid=; for _ in $(seq 40); do ltid=$(ev ThreadCreated project "$lpid" | tail -1); [ -n "$ltid" ] && break; sleep 0.5; done
if [ -z "$LNAME" ] || [ ! -f "$HLPROJ/$LNAME.kicad_pcb" ] || [ -z "$lpid" ] || [ -z "$ltid" ]; then
  bad "libraries row: no libgate project or thread (project: $(tail -c 200 "$OUT/logs/libgate-project.txt"); project.new: $(head -c 200 "$OUT/logs/libgate-project-new.json"); thread: $(head -c 200 "$OUT/logs/libgate-thread.json"))"
else
  # the stand-in logs each MCP answer where the row reads it
  python3 - "$OUT/standins/bin/coedit.conf" <<'PY'
import json, sys
p = sys.argv[1]; c = json.load(open(p)); c["standin_log"] = "/home/person/libgate-standin.log"; json.dump(c, open(p, "w"))
PY
  # a third session: the libgate board in the pane, kept open while the row runs
  WP=$(cat "$HH/.client.pid" 2>/dev/null); [ -n "$WP" ] && in_box kill "$WP" 2>/dev/null; sleep 2
  cat >"$HH/libgate.json" <<J
[{"wait":3000},{"click":"project:$lpid"},{"wait":2000},{"click":"page-thread:$ltid"},{"wait":2500},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},
 {"kicad":{"wait_painted":120000,"kind":"PCB","tag":"lib"}},{"kicad":{"report":true,"tag":"lib"}},{"mark":"lib-ready"},{"wait":1800000},{"mark":"end3"}]
J
  cp "$HH/win.log" "$OUT/logs/win-2b.log" 2>/dev/null; : >"$HH/win.log"
  launch --url http://127.0.0.1:$(hub_port) --token-file $PH/.backplane/hub/token --script $PH/libgate.json
  markt lib-ready 720 && { sleep 2; shot 20-libgate-edit; } || echo "INFO the libgate board did not open in the pane (L7's editor checks will say so)"
  set -- $(painted lib); echo "libgate pane: $*" >"$OUT/logs/libgate-pane.txt"
  SV=$(basename "$KDIR" | cut -d. -f1-2)
  python3 - "$HH/libgate-row-a.json" <<PY
import json, sys
json.dump({"out": "$PH/libgate-out", "port": int("$(hub_port)"), "token_file": "$PH/.backplane/hub/token",
  "hub_rpc": ["/opt/standins/bin/bun", "/opt/standins/libgate/test/kicad_embed/lib/hub_rpc.mjs"], "standin_log": "$PH/libgate-standin.log",
  "thread": "$ltid", "runtime_root": "$KB", "managed_parent": "$(dirname "$KB")", "kicad_cli": "$KB/bin/kicad-cli", "config_root": "$PH/.config/kicad",
  "settings_version": "$SV", "project_dir": "$LPROJ", "pcb": "$LPROJ/$LNAME.kicad_pcb", "sch": "$LPROJ/$LNAME.kicad_sch", "platform": "linux"},
  open(sys.argv[1], "w"), indent=1)
PY
  t=$(date +%s); timeout 2400 docker exec -i -u person -w $PH -e HOME=$PH "$C" python3 /opt/standins/libgate/test/kicad_embed/libraries_row.py a $PH/libgate-row-a.json \
    >"$OUT/logs/libraries-row-a.txt" 2>&1; rc=$?
  tm libraries-row "$(( $(date +%s) - t ))"
  rowchecks "$OUT/logs/libraries-row-a.txt" a $rc
  # phase b: the person's table, the record of what the hub added forgotten (met for the first time), the sync, then L8
  WP=$(cat "$HH/.client.pid" 2>/dev/null); [ -n "$WP" ] && in_box kill "$WP" 2>/dev/null; sleep 2
  KD=$HH/.config/kicad/$SV; mkdir -p "$KD"
  printf '(fp_lib_table\n  (version 7)\n  (lib (name "KiCad")(type "Table")(uri "${KICAD10_TEMPLATE_DIR}/fp-lib-table")(options "")(descr "KiCad Default Libraries"))\n  (lib (name "My_Parts")(type "KiCad")(uri "${KIPRJMOD}/My_Parts.pretty")(options "")(descr "a person'"'"'s own"))\n)\n' >"$KD/fp-lib-table"
  [ -f "$KD/sym-lib-table" ] || printf '(sym_lib_table\n  (version 7)\n  (lib (name "KiCad")(type "Table")(uri "${KICAD10_TEMPLATE_DIR}/sym-lib-table")(options "")(descr "KiCad Default Libraries"))\n)\n' >"$KD/sym-lib-table"
  cp "$KD/fp-lib-table" "$HH/fp-lib-table.before"; cp "$KD/sym-lib-table" "$HH/sym-lib-table.before"
  rm -f "$HUBH/kicad-libtables.json"
  n0=$(grep -ac "library tables" "$HUBH/hub.log" 2>/dev/null); n0=${n0:-0}
  hub rpc setting.set "{\"key\":\"kicad.path\",\"value\":\"$KB/bin/kicad-cli\"}" >"$OUT/logs/libgate-setting.json"
  for _ in $(seq 60); do [ "$(grep -ac "library tables" "$HUBH/hub.log" 2>/dev/null)" -gt "$n0" ] && break; sleep 0.5; done
  grep -a "library tables" "$HUBH/hub.log" | tail -2 >"$OUT/logs/libgate-sync.txt"
  python3 -c 'import json,sys; c=json.load(open(sys.argv[1])); c.update(table_before=sys.argv[3], sym_before=sys.argv[4]); json.dump(c, open(sys.argv[2], "w"), indent=1)' \
    "$HH/libgate-row-a.json" "$HH/libgate-row-b.json" "$PH/fp-lib-table.before" "$PH/sym-lib-table.before"
  timeout 900 docker exec -i -u person -w $PH -e HOME=$PH "$C" python3 /opt/standins/libgate/test/kicad_embed/libraries_row.py b $PH/libgate-row-b.json \
    >"$OUT/logs/libraries-row-b.txt" 2>&1; rc=$?
  rowchecks "$OUT/logs/libraries-row-b.txt" b $rc
  # the person's KiCad path is theirs again (empty: the runtime is found by itself)
  hub rpc setting.set '{"key":"kicad.path","value":""}' >/dev/null 2>&1
  for f in "$HH/libgate-standin.log" "$HH/libgate-row-a.json" "$HH/libgate-row-b.json" "$HH/fp-lib-table.before"; do [ -f "$f" ] && cp "$f" "$OUT/logs/"; done
  cp -r "$HLPROJ" "$OUT/logs/libgate-project" 2>/dev/null
  cp "$HH/win.log" "$OUT/logs/win-3.log" 2>/dev/null
fi
fi
