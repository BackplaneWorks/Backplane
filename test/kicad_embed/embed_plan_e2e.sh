#!/bin/bash
# Suite: the routing floorplanner's hub side (native/host/src/plan/): an agent's plan tools through the hub against Backplane KiCad's plan
# API (bp.11+, `routing-plan`), with a stand-in agent (stream mode, one `mcpcall:` per message) on a copy of the fixture board.
#   P1  plan_propose puts the agent's plan into KiCad; plan_get reads it back with what awaits approval
#   P2  plan_route is refused by the hub while nothing is approved (the audit gate), and plan_approve only asks the person
#   P3  the person approves one corridor (plan.approve, as the Plan panel does); the agent's next plan_get says so in `changes`
#   P4  plan_route of that corridor is let through to KiCad
# Copies only (seed_project.py), the board open in the embedded KiCad (the plan API is the GUI's). Fork mode.
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
# the stand-in's turns short (one chunk), so each call's turn has ended before the next message
export BP_STREAM_N=1 BP_STREAM_MS=10
start_hub
PROJ="$OUT/proj"; PCB="$PROJ/ecc83-pp.kicad_pcb"
# the plan API is the open KiCad's (bp.12's headless server answers AS_UNHANDLED): the board is opened in the embedded editor first
cat >"$OUT/script.json" <<'EOF'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},
 {"kicad":{"wait_painted":60000,"kind":"PCB","tag":"pcb_init"}},{"wait":1500},{"mark":"edit_ready"},{"wait":150000}]
EOF
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_HOLD=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 240 >"$OUT/client.log" 2>&1 &
CP=$!
track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"edit_ready","event":"mark"' 480 || { echo "FAIL the board never opened in KiCad"; tail -n 20 "$OUT/client.log"; exit 1; }
N=0
# one agent tool call: a message to the seeded thread t1 that the stand-in turns into the MCP call; its answer from the stand-in's log
ask() {
  N=$((N + 1)); local before; before=$(grep -c "^MCPCALL" "$OUT/standin.log" 2>/dev/null || echo 0)
  local text; text=$(python3 -c 'import json,sys; print(json.dumps("mcpcall:" + json.dumps({"name": sys.argv[1], "arguments": json.loads(sys.argv[2])})))' "$1" "$2")
  hub_rpc "[{\"method\":\"turn.start\",\"params\":{\"thread\":\"t1\",\"text\":$text,\"msg\":\"mplan$N\"}}]" >>"$OUT/rpc.log" 2>&1 || true
  for _ in $(seq 240); do [ "$(grep -c "^MCPCALL" "$OUT/standin.log" 2>/dev/null || echo 0)" -gt "$before" ] && break; sleep 0.25; done
  sleep 2 # the turn ends
  grep "^MCPCALL $1 " "$OUT/standin.log" | tail -1 || true
}
PLAN='{"layers":[{"layer":"B.Cu","role":"signal","status":"proposed"}],"groups":[{"id":"g-k","name":"Cathodes","color":"#e8a33d","nets":["Net-(U1A-K)","Net-(U1B-K)"],"status":"proposed"}],"corridors":[{"id":"c-k","group":"g-k","layer":"B.Cu","order":1,"path":[[120,100],[150,100]],"width":2.0,"from":["U1"],"to":["P2"],"status":"proposed"}],"planes":[{"id":"p-gnd","net":"GND","layer":"F.Cu","outline":[[[110,85],[165,85],[165,125],[110,125]]],"priority":1,"status":"proposed"}]}'
R1=$(ask plan_propose "{\"plan\":$PLAN,\"mode\":\"merge\"}"); echo "$R1" | cut -c1-300
R2=$(ask plan_get '{}'); echo "$R2" | cut -c1-300
check "plan_propose put the agent's plan into KiCad, and plan_get reads it back awaiting approval" "$(echo "$R2" | grep -q 'awaiting_approval' && echo "$R2" | grep -q 'c-k' && echo 1 || echo 0)" "$(echo "$R1$R2" | cut -c1-400)"
R3=$(ask plan_route '{}'); echo "$R3" | cut -c1-300
check "plan_route is refused by the hub while nothing is approved" "$(echo "$R3" | grep -q 'nothing in the plan is approved' && echo 1 || echo 0)" "$(echo "$R3" | cut -c1-300)"
R4=$(ask plan_approve '{"ids":["c-k"],"note":"the cathode corridor"}'); echo "$R4" | cut -c1-200
check "plan_approve only asks the person" "$(echo "$R4" | grep -q 'asked' && echo 1 || echo 0)"
hub_rpc "[{\"method\":\"plan.approve\",\"params\":{\"path\":\"$PCB\",\"ids\":[\"c-k\"],\"approve\":true}}]" >"$OUT/approve.out" 2>&1 || true
check "The person's approval goes to KiCad" "$(grep -q '"ok":true' "$OUT/approve.out" && echo 1 || echo 0)" "$(head -c 300 "$OUT/approve.out")"
R5=$(ask plan_get '{}'); echo "$R5" | cut -c1-300
check "The agent's next plan_get says the person approved it" "$(echo "$R5" | python3 -c 'import sys; t=sys.stdin.read(); print(int("c-k" in t and "approved" in t and "changes" in t))')" "$(echo "$R5" | cut -c1-300)"
R6=$(ask plan_route '{"ids":["c-k"],"max_nets":2}'); echo "$R6" | cut -c1-300
check "plan_route of the approved corridor goes through to KiCad" "$(echo "$R6" | grep -q 'not approved\|nothing in the plan' && echo 0 || (echo "$R6" | grep -q '"reply"' && echo 1 || echo 0))" "$(echo "$R6" | cut -c1-300)"
finish_suite "embed_plan_e2e ($EMBED_MODE)"
