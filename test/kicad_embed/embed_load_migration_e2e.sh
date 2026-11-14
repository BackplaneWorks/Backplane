#!/bin/bash
# What KiCad rewrites on its own must never be saved by the hub (savegate.rs): with the project's KiCad open (warmed, the person not in
# it), (2) the Board Setup press on a board KiCad migrated on load (3D .wrl -> .step, teardrops rebuilt: runtimes without
# no-load-migration) is set in KiCad and not saved, saying what KiCad changed, the board byte-identical; (3) a BOM edit, a real change,
# is saved and says when KiCad upgraded the file format in the same save. KEMBED_PROJ_SRC for another project (the fork's
# demos/complex_hierarchy: 68 .wrl references, 165 teardrop zones; a copy of Collar's boards/main). fork only.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_load_migration_e2e: fork only"; exit 0; }
discover_bins; [ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PRO=$(find "$OUT/proj" -maxdepth 3 -name '*.kicad_pro' -not -path '*/.backplane/*' -not -name '*mic_flex*' | sort | head -1); PCB=${PRO%.kicad_pro}.kicad_pcb
# a Board Setup suggestion to press: the clearance minimum under the fab's (committed, so the board is the same as git's)
python3 - "$PRO" <<'PY'
import json, sys
p = sys.argv[1]; j = json.load(open(p)); j.setdefault("board", {}).setdefault("design_settings", {}).setdefault("rules", {})["min_clearance"] = 0.0
json.dump(j, open(p, "w"), indent=2)
PY
git -C "$OUT/proj" commit -qam "clearance under the fab's" || true
counts() { python3 -c 'import sys; t=open(sys.argv[1],errors="ignore").read(); print(t.count(".wrl"), t.count(".step")+t.count(".STEP"), t.count("(teardrop"))' "$1"; }
read -r W0 S0 T0 < <(counts "$PCB"); H0=$(sha256sum "$PCB" | cut -c1-16)
echo "INFO board before: $W0 .wrl, $S0 .step, $T0 teardrop entries ($H0)"
start_xvfb_and_wm; start_hub
SCH=${PRO%.kicad_pro}.kicad_sch
schs() { for f in "$(dirname "$PRO")"/*.kicad_sch; do sha256sum "$f" | cut -c1-12; done | tr '\n' ' '; }
SH0=$(schs)
cat > "$OUT/script.json" <<'JSON'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":2500},{"mark":"shown"},{"wait":90000},{"mark":"done"}]
JSON
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 300 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
# the project's KiCad warmed by the shown thread (no Edit: the person never took it), its board loaded
for _ in $(seq 1200); do grep -q 'kicad embed: start +[0-9]*ms ready' "$OUT/hub.log" 2>/dev/null && break; sleep 0.1; done
sleep 4
echo "INFO KiCad: $(grep -o 'kicad embed: running [^ ]*' "$OUT/hub.log" | tail -1)"
# an agent's save takes the document's named lock first (one writer), as the hub tells agents to
asksave() { local out name; out=$(ask kicad_ipc "$1")
  name=$(echo "$out" | grep -o 'lock_acquire kicad-[A-Za-z0-9_-]*' | head -1 | cut -d' ' -f2 || true)
  if [ -n "$name" ]; then ask lock_acquire "{\"name\":\"$name\"}" >/dev/null; out=$(ask kicad_ipc "$1"); fi
  echo "$out"; }
ask() {
  local N; N="$1-$(date +%s%N)"; local before; before=$(grep -c "^MCPCALL" "$OUT/standin.log" 2>/dev/null || echo 0)
  local text; text=$(python3 -c 'import json,sys; print(json.dumps("mcpcall:" + json.dumps({"name": sys.argv[1], "arguments": json.loads(sys.argv[2])})))' "$1" "$2")
  hub_rpc "[{\"method\":\"turn.start\",\"params\":{\"thread\":\"t1\",\"text\":$text,\"msg\":\"mmig$N\"}}]" >>"$OUT/rpc.log" 2>&1 || true
  for _ in $(seq 240); do [ "$(grep -c "^MCPCALL" "$OUT/standin.log" 2>/dev/null || echo 0)" -gt "$before" ] && break; sleep 0.25; done
  sleep 1.5; grep "^MCPCALL $1 " "$OUT/standin.log" | tail -1 || true
}
# (2) the Board Setup press
AP=$(hub_rpc "[{\"method\":\"fab.profile\",\"params\":{\"pro\":\"$PRO\",\"action\":\"apply\",\"key\":\"min_clearance\"}}]" 2>&1 | grep -a '^{"m":"fab.profile"' | head -1)
echo "INFO Board Setup press: $(echo "$AP" | cut -c1-500)"
sleep 2
read -r W1 S1 T1 < <(counts "$PCB"); H1=$(sha256sum "$PCB" | cut -c1-16)
echo "INFO board after: $W1 .wrl, $S1 .step, $T1 teardrop entries ($H1)"
MIG=$(echo "$AP" | grep -o 'KiCad changed the board when it opened it[^"]*' | head -1 || true)
[ -n "$MIG" ] && echo "INFO KiCad migrated on load, said to the person: $MIG" || echo "INFO no load migration said on this runtime"
check "the board's 3D references are as before (no .wrl -> .step written)" "$([ "$W1" = "$W0" ] && [ "$S1" = "$S0" ] && echo 1 || echo 0)" "$W0/$S0 -> $W1/$S1"
check "the board's teardrops are as before" "$([ "$T1" = "$T0" ] && echo 1 || echo 0)" "$T0 -> $T1"
SAVED=$(echo "$AP" | python3 -c 'import json,sys
try: print(json.loads(sys.stdin.read())["result"].get("saved"))
except Exception: print("?")')
check "the Board Setup press saved only a board KiCad did not change on load" "$( { [ -n "$MIG" ] && [ "$SAVED" = False ] && [ "$H1" = "$H0" ]; } || { [ -z "$MIG" ] && [ "$SAVED" = True ]; } && echo 1 || echo 0)" "saved=$SAVED, migration said: ${MIG:+yes}"
check "the Board Setup press leaves the schematics alone" "$([ "$(schs)" = "$SH0" ] && echo 1 || echo 0)"
# (3) a BOM edit: a real change, saved (KiCad not on screen), saying when the format was upgraded with it
REF=$(cat "$(dirname "$PRO")"/*.kicad_sch | grep -o '(property "Reference" "[A-Z]\+[0-9]\+"' | head -1 | grep -o '"[A-Z]\+[0-9]\+"' | tr -d '"' || true)
BE=$(hub_rpc "[{\"method\":\"mgmt.bom.edit\",\"params\":{\"project\":\"$PRO\",\"field\":\"mpn\",\"refs\":[\"$REF\"],\"value\":\"E2E-SAVEGATE\"}}]" 2>&1 | grep -a '^{"m":"mgmt.bom.edit"' | head -1)
echo "INFO BOM edit ($REF): $(echo "$BE" | cut -c1-400)"
sleep 2
check "the BOM edit is saved" "$(grep -q 'E2E-SAVEGATE' "$(dirname "$PRO")"/*.kicad_sch && echo 1 || echo 0)" "$(echo "$BE" | cut -c1-200)"
UP=$(echo "$BE" | grep -o 'KiCad also upgraded the file format[^"]*' | head -1 || true)
echo "INFO BOM edit's format line: ${UP:-none (the files were current)}"
check "the board is untouched by the BOM edit" "$([ "$(sha256sum "$PCB" | cut -c1-16)" = "$H1" ] && echo 1 || echo 0)"
# an agent's saves (an unmodified schematic, a board whose only change is KiCad's) are refused before KiCad is asked: the hub's unit test
# an_agents_save_writes_only_real_changes (the stand-in agent's one call per turn cannot hold the document's lock across turns)
kill "$CP" 2>/dev/null || true
finish_suite "embed_load_migration_e2e ($EMBED_MODE)"
