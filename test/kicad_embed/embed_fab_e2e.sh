#!/bin/bash
# Suite: the Fab management panel (native/host/src/fab/{panel,drc,setup}.rs, native/gpui/src/screens/mgmt_fab.rs).
#   F1  on the Board viewer the strip shows Fab; a press opens it with the fab's limits, each with its source and date (the hub's info)
#   F2  the panel's Write puts the fab's limits into the project's .kicad_dru (a copy; through a KiCad that has it loaded), and the panel
#       then says it is in sync
#   F3  (stock|fork) a board setup suggestion's Apply raises that minimum in the project (through a KiCad that has it loaded, saved
#       when nothing else was unsaved; else the project file), and the suggestion goes
# Copies only (seed_project.py), on a private Xvfb + i3; the hub never fetches the fab's pages (a test hub). EMBED_MODE=spike runs F1-F2.
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
DRU="$PROJ/ecc83-pp.kicad_dru"
mkdir -p "$OUT/shots"

fab_info() { hub_rpc "[{\"method\":\"mgmt.watch\",\"params\":{\"project\":\"$PRO\"}}]" >"$OUT/rpc.out" 2>&1 || true; cat "$OUT/rpc.out" >>"$OUT/rpc.log"
  python3 - "$OUT/rpc.out" <<'PY'
import json, sys
found = {}
for line in open(sys.argv[1], encoding="utf-8", errors="replace"):
    try: o = json.loads(line)
    except Exception: continue
    found = ((o.get("result") or {}).get("data") or {}).get("fab", found)
print(json.dumps(found))
PY
}
last_ev() {
  python3 - "$OUT/client.log" "$1" <<'PY'
import json, sys
found = {}
for line in open(sys.argv[1], encoding="utf-8", errors="replace"):
    for tag in ("GPUI_LOG ", "GPUI_METRIC "):
        if tag in line:
            try: o = json.loads(line.split(tag, 1)[1])
            except Exception: continue
            if o.get("event") == sys.argv[2]: found = o
print(json.dumps(found))
PY
}
# whether any controls list the client reported so far had this control (the panel's buttons come and go as the hub's data changes)
has_ctl() {
  python3 - "$OUT/client.log" "$1" <<'PY'
import json, sys
seen = 0
for line in open(sys.argv[1], encoding="utf-8", errors="replace"):
    for tag in ("GPUI_LOG ", "GPUI_METRIC "):
        if tag in line:
            try: o = json.loads(line.split(tag, 1)[1])
            except Exception: continue
            if o.get("event") == "controls" and any((x.get("id") if isinstance(x, dict) else x) == sys.argv[2] for x in o.get("controls", [])): seen = 1
print(seen)
PY
}

# the suggestion F3 applies: the project's clearance minimum, set under the fab's
python3 - "$PRO" <<'PY'
import json, sys
p = sys.argv[1]; j = json.load(open(p)); j.setdefault("board", {}).setdefault("design_settings", {}).setdefault("rules", {})["min_clearance"] = 0.0
json.dump(j, open(p, "w"), indent=2)
PY
cat >"$OUT/script.json" <<'EOF'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":2500},{"mark":"board"},{"wait":800},
 {"click":"mgmt:fab"},{"wait":2500},{"mark":"fab_open"},{"wait":1000},
 {"click":"mgmt:fab:drc"},{"wait":3000},{"mark":"drc_written"},{"wait":1000}]
EOF
if [ "$EMBED_MODE" != "spike" ]; then
  python3 - "$OUT/script.json" <<'PY'
import json, sys
p = sys.argv[1]; s = json.load(open(p))
s += [{"click": "mgmt:fab:apply:min_clearance"}, {"wait": 20000}, {"mark": "applied"}, {"wait": 1500}]
json.dump(s, open(p, "w"))
PY
fi
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_HOLD=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 180 >"$OUT/client.log" 2>&1 &
CP=$!
track_pid "$CP"

# F1: the tab and the limits
wait_for_log "$OUT/client.log" '"name":"board","event":"mark"' 120 || { echo "FAIL the Board page never came up"; tail -n 30 "$OUT/client.log"; exit 1; }
check "The strip under the Board viewer has a Fab tab" "$(has_ctl mgmt:fab)" "$(last_ev controls | head -c 400)"
wait_for_log "$OUT/client.log" '"name":"fab_open","event":"mark"' 60 || true
sleep 0.5; shot "f1_fab_open"
FAB=$(fab_info)
check "The hub published the fab's limits with source and date" "$(python3 -c 'import json,sys; v=json.loads(sys.argv[1]); t=[l for l in v.get("limits",[]) if l.get("key")=="track_min"]; print(int(bool(t) and t[0]["free"]["source"].startswith("https://") and len(t[0]["free"]["fetched"])==10))' "$FAB" 2>/dev/null || echo 0)" "$(echo "$FAB" | head -c 300)"
# (the client offered Write, which it does only while the rules lack the fab's section: read from its controls, since on a slow venue the
# script's press can come before this check reads the hub)
check "Before Write the panel offered to write the fab's limits into the DRC rules" "$(has_ctl mgmt:fab:drc)"
check "The clearance minimum under the fab's is a suggestion" "$(has_ctl mgmt:fab:apply:min_clearance)"

# F2: Write
wait_for_log "$OUT/client.log" '"name":"drc_written","event":"mark"' 60 || true
sleep 0.5; shot "f2_drc_written"
# the section as Backplane writes the file, or (a KiCad with the project loaded, even only warmed) the fab's rules by name through it
check "Write put the fab's limits into the project's .kicad_dru" "$({ grep -q '^# BEGIN Backplane fab profile' "$DRU" 2>/dev/null && grep -q '^# END Backplane fab profile' "$DRU"; } || grep -q '^(rule "fab: ' "$DRU" 2>/dev/null && echo 1 || echo 0)" "$(head -c 300 "$DRU" 2>/dev/null)"
FAB=$(fab_info)
check "After Write the panel says the DRC rules are in sync" "$(python3 -c 'import json,sys; print(int(json.loads(sys.argv[1])["drc"]["state"]=="current"))' "$FAB" 2>/dev/null || echo 0)" "$(echo "$FAB" | head -c 200)"

# F3: Apply through the headless KiCad
if [ "$EMBED_MODE" != "spike" ]; then
  wait_for_log "$OUT/client.log" '"name":"applied","event":"mark"' 90 || true
  sleep 0.5; shot "f3_applied"
  V=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["board"]["design_settings"]["rules"].get("min_clearance"))' "$PRO" 2>/dev/null || echo "?")
  # what the hub answers an Apply now (the panel's press logs no reply): said when the value did not move
  python3 -c 'import sys; sys.exit(0 if float(sys.argv[1]) >= 0.09 - 1e-9 else 1)' "$V" 2>/dev/null || echo "INFO a second Apply answers: $(hub_rpc "[{\"method\":\"fab.profile\",\"params\":{\"pro\":\"$PROJ/ecc83-pp.kicad_pro\",\"action\":\"apply\",\"key\":\"min_clearance\"}}]" 2>&1 | grep -a "^{\"m\":\"fab.profile\"" | cut -c1-700)"
  check "Apply raised the project's clearance minimum to the fab's (now $V)" "$(python3 -c 'import sys; print(int(float(sys.argv[1]) >= 0.09 - 1e-9))' "$V" 2>/dev/null || echo 0)" "$(grep -i 'fab.profile\|apply' "$OUT/client.log" | tail -n 5)"
  FAB=$(fab_info)
  check "The suggestion is gone once applied" "$(python3 -c 'import json,sys; print(int(not any(s.get("key")=="min_clearance" for s in json.loads(sys.argv[1]).get("suggestions",[]))))' "$FAB" 2>/dev/null || echo 0)"
fi

finish_suite "embed_fab_e2e ($EMBED_MODE)"
