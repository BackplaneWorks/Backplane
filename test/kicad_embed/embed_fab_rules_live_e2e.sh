#!/bin/bash
# Suite: the fab's DRC limits into a project whose board is open in KiCad go through KiCad (fab_profile action "drc" -> EditCustomRules),
#   never behind its back (fab/mod.rs write_rules, fab/drc.rs edit_message).
#   R1  fab_profile {action: drc} with the board open answers via "kicad"; the rules file then has the fab's rules
#   R2  the person's own rule is byte-identical, and the fab's track-width rule is left out (the person's track rule keeps deciding)
#   R3  fab_profile get says the rules are current; a second write changes no byte
#   R4  stock KiCad 10 (/usr/bin/kicad-cli, when present) runs DRC on the result without a rules error
# Copies only (seed_project.py), the board open in the embedded editor. Fork mode; skips when the open KiCad lacks rules-merge.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_fab_rules_live_e2e: fork only"; exit 0; }
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PROJ="$OUT/proj"; PCB="$PROJ/ecc83-pp.kicad_pcb"; DRU="$PROJ/ecc83-pp.kicad_dru"; mkdir -p "$OUT/shots"
MINE='(rule "mine: wide tracks"
	(constraint track_width (min 0.3mm)))'
printf '(version 1)\n# my own rules, by hand\n%s\n' "$MINE" >"$DRU"
printf '{"kind":"pcb","name":"ECC83","source":"ecc83-pp.kicad_pcb","qty":5,"options":{"layers":2}}\n' >"$PROJ/ecc83-pp.fab.json"
git -C "$PROJ" add -A >/dev/null 2>&1 || true; git -C "$PROJ" commit -qm "own rules" >/dev/null 2>&1 || true
# paint bounds (Nov 13, bp.13 batch 3, 16 cores busy): KiCad painted 0.07-0.08 s after Edit and the board was open, full size, 9.7-13.3 s
# after the client started; the main-thread stall fixed in 82c67d16a made them 11 s and 88-133 s. So 5 s to paint and 45 s to be ready.
start_xvfb_and_wm
export BP_STREAM_N=1 BP_STREAM_MS=10 BP_STANDIN_MCPCALL_MAX=200000
start_hub
cat >"$OUT/script.json" <<'JSON'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},
 {"kicad":{"wait_painted":5000,"kind":"PCB","tag":"ready"}},{"wait":1500},
 {"click":"kicad:full"},{"wait":1500},{"mark":"edit_ready"},{"wait":150000}]
JSON
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_HOLD=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 420 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"edit_ready","event":"mark"' 180 || { echo "FAIL the board never opened in KiCad"; tail -n 20 "$OUT/client.log"; exit 1; }
ask() {
  local N; N="$1-$(date +%s%N)"; local before; before=$(grep -c "^MCPCALL" "$OUT/standin.log" 2>/dev/null || echo 0)
  local text; text=$(python3 -c 'import json,sys; print(json.dumps("mcpcall:" + json.dumps({"name": sys.argv[1], "arguments": json.loads(sys.argv[2])})))' "$1" "$2")
  hub_rpc "[{\"method\":\"turn.start\",\"params\":{\"thread\":\"t1\",\"text\":$text,\"msg\":\"mmark$N\"}}]" >>"$OUT/rpc.log" 2>&1 || true
  for _ in $(seq 480); do [ "$(grep -c "^MCPCALL" "$OUT/standin.log" 2>/dev/null || echo 0)" -gt "$before" ] && break; sleep 0.25; done
  sleep 2
  grep "^MCPCALL $1 " "$OUT/standin.log" | tail -1 || true
}
CAPS=$(ask kicad_ipc "{\"path\":\"$PCB\",\"type\":\"kiapi.backplane.GetCapabilities\",\"message\":{}}")
echo "$CAPS" | grep -q 'rules-merge' || { echo "SKIP embed_fab_rules_live_e2e: the open KiCad has no rules-merge"; finish_suite "embed_fab_rules_live_e2e ($EMBED_MODE)"; exit 0; }
field() { sed 's/^MCPCALL fab_profile //' | python3 -c 'import json,sys
try: o = json.loads(sys.stdin.read()); r = json.loads(o["result"]["content"][0]["text"])
except Exception as e: print("?"); sys.exit()
v = r
for k in sys.argv[1].split("."): v = v.get(k) if isinstance(v, dict) else None
print(json.dumps(v))' "$1"; }
W1=$(ask fab_profile '{"action":"drc"}'); echo "$W1" | cut -c1-300
VIA=$(echo "$W1" | field drc.via); LEFT=$(echo "$W1" | field drc.left_out)
echo "INFO via $VIA; left out: $LEFT"
check "the write went through KiCad (EditCustomRules), not behind its back" "$([ "$VIA" = '"kicad"' ] && echo 1 || echo 0)" "$(echo "$W1" | cut -c1-300)"
check "the rules file has the fab's rules" "$(grep -q '(rule "fab: via ring"' "$DRU" && echo 1 || echo 0)" "$(head -c 600 "$DRU")"
check "the person's own rule is byte-identical" "$(python3 -c 'import sys; print(int(sys.argv[2] in open(sys.argv[1]).read()))' "$DRU" "$MINE")"
check "the fab's track-width rule is left out: the person's own track rule keeps deciding" "$({ ! grep -q '"fab: track width"' "$DRU" && echo "$LEFT" | grep -q 'fab: track width'; } && echo 1 || echo 0)" "$LEFT"
cp "$DRU" "$OUT/dru.after1"
G=$(ask fab_profile '{}'); CUR=$(echo "$G" | field drc.current)
check "fab_profile says the rules are current" "$([ "$CUR" = true ] && echo 1 || echo 0)" "current=$CUR"
ask fab_profile '{"action":"drc"}' >/dev/null
check "a second write changes no byte" "$(cmp -s "$DRU" "$OUT/dru.after1" && echo 1 || echo 0)" "$(diff "$OUT/dru.after1" "$DRU" | head -5)"
if [ -x /usr/bin/kicad-cli ] && /usr/bin/kicad-cli version 2>/dev/null | grep -q '^10\.0'; then
  mkdir -p "$OUT/stock"; cp "$PROJ"/ecc83-pp.kicad_{pro,pcb,dru} "$OUT/stock/"
  ( cd "$OUT/stock" && timeout 120 /usr/bin/kicad-cli pcb drc --format json -o drc.json ecc83-pp.kicad_pcb >stock.log 2>&1 ) && SR=0 || SR=$?
  check "stock KiCad 10 runs DRC on the result without a rules error" "$({ [ -s "$OUT/stock/drc.json" ] && ! grep -qi 'rule.*error\|error.*rule\|parse' "$OUT/stock/stock.log"; } && echo 1 || echo 0)" "exit $SR: $(head -c 300 "$OUT/stock/stock.log")"
else echo "INFO no stock KiCad 10 kicad-cli here: R4 not run"; fi
finish_suite "embed_fab_rules_live_e2e ($EMBED_MODE)"
