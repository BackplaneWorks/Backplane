#!/bin/bash
# Suite: an agent routes an approved plan while the person looks at it in KiCad's plan view, and the Plan panel's rows work there
# (bp.13 batch 11, `plan-view-api`: plan view is a mode, busy to the API only while the person drags or draws a plan item).
#   L1  plan view on (RunAction togglePlanView), as KiCad says (GetPlan in_plan_view); the person approves c-k from the Plan panel
#   L2  plan_route goes ahead in plan view and routes c-k (steps until done); plan view stays on
#   L3  plan_verify: c-k's group has no DRC violation
#   L4  only while the person holds a drag on a plan item does plan_route answer "Nothing was routed" with the reason; released, it goes on
#   L5  the Plan panel's row (plan.focus) selects and zooms to c-k and makes its layer active, plan view staying on
# Mode and layer are read from KiCad, never from pixels; pixels only find the item to drag. Copies only (seed_project.py). Fork mode;
# skips on runtimes without plan-view-api.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_plan_route_live_e2e: fork only"; exit 0; }
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
RT=$(dirname "$(dirname "$KICAD_BIN")")
grep -q '"plan-view-api"' "$RT/manifest.json" 2>/dev/null || grep -qa "plan-view-api" "$RT/bin/kicad.real" 2>/dev/null \
  || { echo "SKIP embed_plan_route_live_e2e: this KiCad has no plan-view-api (bp.13 batch 11)"; exit 0; }
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
start_xvfb_and_wm
export BP_STREAM_N=1 BP_STREAM_MS=10 BP_STANDIN_MCPCALL_MAX=200000
start_hub
PROJ="$OUT/proj"; PCB="$PROJ/ecc83-pp.kicad_pcb"; PRO="$PROJ/ecc83-pp.kicad_pro"; mkdir -p "$OUT/shots"
cat >"$OUT/script.json" <<'JSON'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},
 {"kicad":{"wait_painted":5000,"kind":"PCB","tag":"ready"}},{"wait":1500},{"click":"kicad:full"},{"wait":2500},{"kicad":{"report":true,"tag":"placed"}},{"mark":"edit_ready"},{"wait":240000}]
JSON
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_HOLD=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 300 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"edit_ready","event":"mark"' 180 || { echo "FAIL the board never opened in KiCad"; tail -n 20 "$OUT/client.log"; exit 1; }
ask() {
  local N; N="$1-$(date +%s%N)"; local before; before=$(grep -c "^MCPCALL" "$OUT/standin.log" 2>/dev/null || echo 0)
  local text; text=$(python3 -c 'import json,sys; print(json.dumps("mcpcall:" + json.dumps({"name": sys.argv[1], "arguments": json.loads(sys.argv[2])})))' "$1" "$2")
  hub_rpc "[{\"method\":\"turn.start\",\"params\":{\"thread\":\"t1\",\"text\":$text,\"msg\":\"mlive$N\"}}]" >>"$OUT/rpc.log" 2>&1 || true
  for _ in $(seq 240); do [ "$(grep -c "^MCPCALL" "$OUT/standin.log" 2>/dev/null || echo 0)" -gt "$before" ] && break; sleep 0.25; done
  sleep 2
  grep "^MCPCALL $1 " "$OUT/standin.log" | tail -1 || true
}
answer() { sed 's/^MCPCALL [a-z_]* //' | python3 -c 'import json,sys
try: o = json.loads(sys.stdin.read()); print(o["result"]["content"][0]["text"])
except Exception: print("{}")'; }
X() { python3 "$XINPUT_PY" "$@"; sleep "${KEMBED_STEP_S:-0.5}"; }
BOARD_SPEC="{\"type\":\"DOCTYPE_PCB\",\"board_filename\":\"ecc83-pp.kicad_pcb\",\"project\":{\"name\":\"ecc83-pp\",\"path\":\"$PROJ\"}}"
in_plan_view() { ask kicad_ipc "{\"path\":\"$PCB\",\"type\":\"kiapi.backplane.plan.GetPlan\",\"message\":{\"board\":$BOARD_SPEC,\"derived\":false}}" | answer | python3 -c '
import json, sys
m = json.loads(sys.stdin.read() or "{}").get("message", {})
print("on" if m.get("in_plan_view") else "off")'; }   # proto3 leaves a false bool out: absent is off on this build
active_layer() { ask kicad_ipc "{\"path\":\"$PCB\",\"type\":\"kiapi.board.commands.GetActiveLayer\",\"message\":{\"board\":$BOARD_SPEC}}" | grep -o 'BL_[A-Za-z0-9_]*' | head -1; }
run_action() { ask kicad_ipc "{\"path\":\"$PCB\",\"type\":\"kiapi.common.commands.RunAction\",\"message\":{\"action\":\"$1\"}}"; }

# c-k (orange) routes U1's cathodes; c-d (green) is only there to be dragged; both on B.Cu
PLAN='{"layers":[{"layer":"B.Cu","role":"signal","status":"proposed"}],"groups":[{"id":"g-k","name":"Cathodes","color":"#e8a33d","nets":["Net-(U1A-K)","Net-(U1B-K)"],"status":"proposed"},{"id":"g-d","name":"Drag","color":"#3de86b","nets":["Net-(U1A-G)"],"status":"proposed"}],"corridors":[{"id":"c-k","group":"g-k","layer":"B.Cu","order":1,"path":[[120,100],[150,100]],"width":2.0,"from":["U1"],"to":["P2"],"status":"proposed"},{"id":"c-d","group":"g-d","layer":"B.Cu","order":2,"path":[[130,127],[160,127]],"width":4.0,"status":"proposed"}]}'
R=$(ask plan_propose "{\"plan\":$PLAN,\"mode\":\"merge\"}"); echo "$R" | cut -c1-200
# L1: plan view on, from KiCad's own answer; the person approves c-k (the Plan panel's Approve: plan.approve as the person)
M0=$(in_plan_view)
[ "$M0" = on ] || { RA=$(run_action pcbnew.Plan.togglePlanView); echo "INFO togglePlanView: $(echo "$RA" | grep -o 'RAS_[A-Z_]*\|AS_[A-Z_]*' | head -1)"; sleep 1; }
M1=$(in_plan_view)
check "plan view is on (KiCad's GetPlan in_plan_view)" "$([ "$M1" = on ] && echo 1 || echo 0)" "before $M0, now $M1"
hub_rpc "[{\"method\":\"plan.approve\",\"params\":{\"pro\":\"$PRO\",\"ids\":[\"c-k\"],\"approve\":true}}]" >"$OUT/approve.out" 2>&1 || true
check "the person approves c-k from the Plan panel in plan view" "$(grep -q '"ok":true' "$OUT/approve.out" && echo 1 || echo 0)" "$(head -c 300 "$OUT/approve.out")"
sleep 2
# L2: the agent routes it while plan view is on: no "Nothing was routed", steps until done
DONE=0; LAST=""
for step in 1 2 3 4 5 6; do
  RR=$(ask plan_route '{"ids":["c-k"],"max_nets":2}'); LAST=$(echo "$RR" | answer)
  echo "INFO plan_route step $step: $(echo "$LAST" | cut -c1-200)"
  echo "$LAST" | grep -q "Nothing was routed" && break
  echo "$LAST" | python3 -c 'import json,sys; d=json.loads(sys.stdin.read() or "{}"); m=d.get("message",d); sys.exit(0 if m.get("done") else 1)' 2>/dev/null && { DONE=1; break; }
done
check "plan_route goes ahead in plan view (KiCad is not busy to it)" "$(echo "$LAST" | grep -q "Nothing was routed\|AS_BUSY" && echo 0 || echo 1)" "$(echo "$LAST" | cut -c1-300)"
check "plan_route routes c-k to done" "$DONE" "$(echo "$LAST" | cut -c1-300)"
check "plan view stayed on while it routed" "$([ "$(in_plan_view)" = on ] && echo 1 || echo 0)"
shot l2_routed
# L3: DRC on the routed group
V=$(ask plan_verify '{}' | answer); echo "INFO plan_verify: $(echo "$V" | cut -c1-300)"
check "plan_verify: c-k's group has no DRC violation" "$(echo "$V" | python3 -c '
import json, sys
d = json.loads(sys.stdin.read() or "{}"); m = d.get("message", d)
try: rep = json.loads(m.get("report") or "{}")
except Exception: rep = {}
g = rep.get("g-k") or {}
print(int("drc" in g and int(g["drc"] or 0) == 0))')" "$(echo "$V" | cut -c1-300)"
# L4: a drag held on c-d: RoutePlan busy, said as nothing routed with the reason; released, it goes on
R=$(last_kicad_report "$OUT/client.log" placed)
read -r FX FY FW FH < <(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); f=next((f for f in r.get("frames",[]) if f.get("xid")==r.get("shown")),{}); print(f.get("root_x",0), f.get("root_y",0), f.get("w",0), f.get("h",0))' "$R")
X click "$((FX + 60))" "$((FY + FH / 2))"; X key Home; sleep 1.5; shot l4_plan
read -r DX DY < <(python3 - "$OUT/shots/l4_plan.png" "$FX" "$FY" "$FW" "$FH" <<'PY'
import sys
import numpy as np
from PIL import Image
a = np.asarray(Image.open(sys.argv[1]).convert("RGB")).astype(int); fx, fy, fw, fh = map(int, sys.argv[2:6])
a = a[fy + 160:fy + fh - 40, fx + 40:fx + fw - 280]; r, g, b = a[..., 0], a[..., 1], a[..., 2]
ys, xs = np.nonzero((g > 150) & (g - r > 60) & (g - b > 30))
print(f"{int(np.median(xs)) + fx + 40} {int(np.median(ys)) + fy + 160}" if len(xs) > 50 else "0 0")
PY
)
echo "INFO c-d on screen at ($DX,$DY)"
check "c-d is drawn in plan view (found to drag)" "$([ "${DX:-0}" -gt 0 ] && echo 1 || echo 0)" "see l4_plan.png"
X bdown "$DX" "$DY"; for i in 1 2 3 4 5 6; do python3 "$XINPUT_PY" move "$((DX + 6 * i))" "$DY"; sleep 0.05; done; sleep 0.5
RB=$(ask plan_route '{"ids":["c-k"],"max_nets":1}' | answer); shot l4_dragging
X bup; sleep 1
echo "INFO plan_route during the drag: $(echo "$RB" | cut -c1-300)"
check "during the person's drag plan_route answers that nothing was routed, with the reason" "$(echo "$RB" | grep -q "Nothing was routed" && echo "$RB" | grep -qi "busy" && echo 1 || echo 0)" "$(echo "$RB" | cut -c1-300)"
RC=$(ask plan_route '{"ids":["c-k"],"max_nets":1}' | answer)
check "released, plan_route goes on (not busy)" "$(echo "$RC" | grep -q "Nothing was routed\|AS_BUSY" && echo 0 || echo 1)" "$(echo "$RC" | cut -c1-300)"
# L5: the Plan panel's row: c-k's nets and parts selected and zoomed, B.Cu active, plan view still on
SL=$(ask kicad_ipc "{\"path\":\"$PCB\",\"type\":\"kiapi.board.commands.SetActiveLayer\",\"message\":{\"board\":$BOARD_SPEC,\"layer\":\"BL_F_Cu\"}}" >/dev/null; active_layer)
FO=$(hub_rpc "[{\"method\":\"plan.focus\",\"params\":{\"pro\":\"$PRO\",\"id\":\"c-k\"}}]" 2>&1 | grep -a '^{"m":"plan.focus"' | head -1)
sleep 1; LF=$(active_layer); MF=$(in_plan_view); shot l5_focus
echo "INFO plan.focus: $(echo "$FO" | cut -c1-300)"
SEL=$(ask kicad_ipc "{\"path\":\"$PCB\",\"type\":\"kiapi.common.commands.GetSelection\",\"message\":{\"header\":{\"document\":$BOARD_SPEC}}}")
check "the Plan panel's row selects c-k's copper in KiCad (zoomed to it)" "$(echo "$FO" | grep -q '"ok":true' && echo "$SEL" | grep -q 'kiapi.board.types' && echo 1 || echo 0)" "focus: $(echo "$FO" | cut -c1-200); selection: $(echo "$SEL" | grep -o 'kiapi.board.types.[A-Za-z]*' | sort -u | tr '\n' ' ')"
check "it makes c-k's layer (B.Cu) active" "$([ "$LF" = BL_B_Cu ] && echo 1 || echo 0)" "active $SL -> $LF"
check "plan view stays on" "$([ "$MF" = on ] && echo 1 || echo 0)" "plan view $MF"
finish_suite "embed_plan_route_live_e2e ($EMBED_MODE)"
