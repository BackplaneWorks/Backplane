#!/bin/bash
# Suite: the person's edits in KiCad's own plan view (Backplane KiCad bp.13: Alt+P, click to select, A accept, R reject, L lock, W wider)
# reach the hub as the person's: the agent's plan_get `changes` says what they did, the Plan panel shows it live, and the agent that asked
# for approval is woken with their decision. Real X input (XTest) into the embedded editor on a copy of the fixture board; fork only.
#   Q1  the agent's plan (two wide corridors on F.Cu either side of the board's centre) is in KiCad, awaiting approval
#   Q2  in KiCad's plan view the person selects c-p, accepts it (A), locks it (L), widens it (W); selects c-q and rejects it (R)
#   Q3  plan_get `changes`: c-p approved, locked, edited by the person, a constraint by the person; c-q rejected
#   Q4  the Plan panel (mgmt.plan, the hub's data) shows c-p approved and locked, c-q rejected, without a refresh
#   Q5  the agent that asked is woken with the person's decision
#   Q7  (bp.13 batch 4) with B.Cu active, a click on an F.Cu corridor (c-r) selects it and makes F.Cu active, and W widens it; W with
#       nothing selected changes nothing. Batch 3's hit test looked only at the active layer: there the first case is expected to fail,
#       so it is told (XFAIL) without failing the suite until the build has the fix (capability rules-insert-position marks batch 4;
#       PLAN_XLAYER=expect runs it for real on any build)
#   Q8  (bp.13 batch 5) Escape with nothing selected leaves plan view, and Ctrl+Shift+R starts it again; earlier builds XFAIL
#   Q6  layer approval is the person's: KiCad's right-click menu "Approve Layer" on B.Cu (PageDown, then the menu by keyboard) approves
#       it; an agent's kicad_ipc RunAction pcbnew.Plan.approveLayer and ApprovePlan are refused, its plan_approve only asks (F.Cu stays
#       unapproved); the Plan panel's Approve (plan.approve: ApprovePlan as the person) approves F.Cu with plan view on
# PLAN_VIA=key: plan view by the real hotkey (Ctrl+Shift+R, or the build's own), pressed straight after the editor opens, no canvas
# click first; otherwise (default) by RunAction pcbnew.Plan.togglePlanView.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_plan_person_e2e: fork only"; exit 0; }
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
grep -qa "planAccept\|Plan.accept" "$(dirname "$KICAD_BIN")/_pcbnew.kiface" 2>/dev/null || { echo "SKIP embed_plan_person_e2e: this KiCad has no plan view edits (bp.13+)"; exit 0; }
# bp.12.1 has the plan view's actions but not the person's approvals in it (A/R, Approve Layer and ApprovePlan leave the plan proposed;
# Nov 13: 13/20 on bp.12.1, 26/26 on bp.13): the suite is bp.13's, told by its embed-frame-drawn capability
grep -qa "embed-frame-drawn" "$(dirname "$KICAD_BIN")/kicad.real" 2>/dev/null || { echo "SKIP embed_plan_person_e2e: this KiCad's plan view has no person approvals (bp.13+)"; exit 0; }
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
# paint bounds (Nov 13, bp.13 batch 3, 16 cores busy): KiCad painted 0.07-0.08 s after Edit and the board was open, full size, 9.7-13.3 s
# after the client started; the main-thread stall fixed in 82c67d16a made them 11 s and 88-133 s. So 5 s to paint and 45 s to be ready.
start_xvfb_and_wm
export BP_STREAM_N=1 BP_STREAM_MS=10 BP_STANDIN_MCPCALL_MAX=200000
start_hub
PROJ="$OUT/proj"; PCB="$PROJ/ecc83-pp.kicad_pcb"; mkdir -p "$OUT/shots"
cat >"$OUT/script.json" <<'JSON'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},
 {"kicad":{"wait_painted":5000,"kind":"PCB","tag":"ready"}},{"wait":1500},{"click":"kicad:full"},{"wait":2500},{"kicad":{"report":true,"tag":"placed"}},{"mark":"edit_ready"},{"wait":200000}]
JSON
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_HOLD=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 480 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"edit_ready","event":"mark"' 180 || { echo "FAIL the board never opened in KiCad"; tail -n 20 "$OUT/client.log"; exit 1; }
ask() {
  local N; N="$1-$(date +%s%N)"; local before; before=$(grep -c "^MCPCALL" "$OUT/standin.log" 2>/dev/null || echo 0)
  local text; text=$(python3 -c 'import json,sys; print(json.dumps("mcpcall:" + json.dumps({"name": sys.argv[1], "arguments": json.loads(sys.argv[2])})))' "$1" "$2")
  hub_rpc "[{\"method\":\"turn.start\",\"params\":{\"thread\":\"t1\",\"text\":$text,\"msg\":\"mperson$N\"}}]" >>"$OUT/rpc.log" 2>&1 || true
  for _ in $(seq 240); do [ "$(grep -c "^MCPCALL" "$OUT/standin.log" 2>/dev/null || echo 0)" -gt "$before" ] && break; sleep 0.25; done
  sleep 2
  grep "^MCPCALL $1 " "$OUT/standin.log" | tail -1 || true
}
plan_info() { hub_rpc "[{\"method\":\"mgmt.watch\",\"params\":{\"project\":\"$PROJ/ecc83-pp.kicad_pro\"}}]" 2>/dev/null | python3 -c 'import json,sys
f={}
for l in sys.stdin:
    try: o=json.loads(l)
    except Exception: continue
    f=((o.get("result") or {}).get("data") or {}).get("plan", f)
print(json.dumps(f))'; }
# the board's outline centre (147.3, 113.3); c-p left of it, c-q right, both wide enough for a click; the plane off the active layer
PLAN='{"layers":[{"layer":"F.Cu","role":"signal","status":"proposed"},{"layer":"B.Cu","role":"plane","status":"proposed"}],"groups":[{"id":"g-k","name":"Cathodes","color":"#e8a33d","nets":["Net-(U1A-K)","Net-(U1B-K)"],"status":"proposed"},{"id":"g-r","name":"Spare","color":"#3de86b","nets":["Net-(U1A-G)"],"status":"proposed"}],"corridors":[{"id":"c-p","group":"g-k","layer":"F.Cu","order":1,"path":[[122,113.3],[140,113.3]],"width":12.0,"status":"proposed"},{"id":"c-q","group":"g-k","layer":"F.Cu","order":2,"path":[[155,113.3],[173,113.3]],"width":12.0,"status":"proposed"},{"id":"c-r","group":"g-r","layer":"F.Cu","order":3,"path":[[136,129],[159,129]],"width":3.0,"status":"proposed"}],"planes":[{"id":"p-gnd","net":"GND","layer":"B.Cu","outline":[[[122,91],[173,91],[173,108],[122,108]]],"priority":1,"status":"proposed"}]}'
R1=$(ask plan_propose "{\"plan\":$PLAN,\"mode\":\"merge\"}"); echo "$R1" | cut -c1-200
R2=$(ask plan_get '{}')
check "the agent's plan is in KiCad, awaiting approval" "$(echo "$R2" | grep -q 'c-q' && echo "$R2" | grep -q awaiting_approval && echo 1 || echo 0)" "$(echo "$R2" | cut -c1-300)"
R3=$(ask plan_approve '{"ids":["c-p","c-q"],"note":"the two cathode corridors"}')
check "plan_approve only asks the person" "$(echo "$R3" | grep -q asked && echo 1 || echo 0)"
sleep 3
# the board editor's frame (its root rectangle); the canvas is most of it. KiCad's own GL canvas is no X window of its own, so the plan's
# corridors are found on the screen by their colour (#e8a33d, the group's) after zoom to fit
R=$(last_kicad_report "$OUT/client.log" placed)
read -r FRAME FX FY FW FH < <(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); f=next((f for f in r.get("frames",[]) if f.get("xid")==r.get("shown")),{}); print(r.get("shown") or 0, f.get("root_x",0), f.get("root_y",0), f.get("w",0), f.get("h",0))' "$R")
if [ "${FRAME:-0}" = 0 ]; then
  shot q0_no_editor
  check "the board is open in KiCad's editor" 0 "$(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); print(r.get("state"), r.get("reason"))' "$R")"
  finish_suite "embed_plan_person_e2e ($EMBED_MODE)"; exit 1
fi
echo "INFO editor frame $FRAME ${FW}x${FH}+${FX}+${FY}"
check "the board editor is shown full size" "$([ "${FW:-0}" -gt 700 ] && echo 1 || echo 0)" "${FW}x${FH}"
X() { python3 "$XINPUT_PY" "$@"; sleep "${KEMBED_STEP_S:-0.7}"; }
HOTKEY="Alt+P"; grep -qa "Routing Plan View" "$(dirname "$KICAD_BIN")/_pcbnew.kiface" 2>/dev/null && HOTKEY="Ctrl+Shift+R"
hotkey() { if [ "$HOTKEY" = "Ctrl+Shift+R" ]; then X chord ctrl shift r; else X chord alt p; fi; }
if [ "${PLAN_VIA:-action}" = key ]; then
  # the real key, straight after the editor opened: no click on the canvas first (bp.13 build #2's Alt+P opened the Place menu here)
  hotkey; VIA="$HOTKEY (pressed without a canvas click)"; sleep 1.5; shot q0_after_hotkey
else
# the keyboard to KiCad: a click on the canvas (its centre is the board's, where nothing of the plan is), then Escape
X click "$((FX + FW / 2))" "$((FY + FH / 2))"; X key Escape
# plan view: KiCad's own action (RunAction pcbnew.Plan.togglePlanView, every build); the hotkey only when the action is refused
# (bp.13 build #2: Alt+P; batch 3 on: Ctrl+Shift+R; the build is told by what it does, never by its version string)
RA=$(ask kicad_ipc "{\"path\":\"$PCB\",\"type\":\"kiapi.common.commands.RunAction\",\"message\":{\"action\":\"pcbnew.Plan.togglePlanView\"}}")
echo "INFO RunAction togglePlanView: $(echo "$RA" | grep -o 'RAS_[A-Z_]*' | head -1)"
if echo "$RA" | grep -q 'RAS_OK'; then VIA=action
else VIA="$HOTKEY"; hotkey
fi
fi
echo "INFO plan view by $VIA"; sleep 1.5
X key Home; sleep 1.5                    # zoom to fit
shot q1_plan_view
# c-p and c-q on the screen: the corridor colour's pixels inside the frame, split at the widest gap in x into the two biggest runs
spots() { python3 - "$OUT/shots/$1.png" "$FX" "$FY" "$FW" "$FH" <<'PY'
import sys
import numpy as np
from PIL import Image
f, fx, fy, fw, fh = sys.argv[1], *map(int, sys.argv[2:])
a = np.asarray(Image.open(f).convert("RGB")).astype(int)[fy + 60:fy + fh - 40, fx + 40:fx + fw - 40]
r, g, b = a[..., 0], a[..., 1], a[..., 2]
m = (r > 120) & (r - b > 70) & (g > 60) & (g < r - 10) & (g > b + 20)
ys, xs = np.nonzero(m)
if len(xs) < 50: print("0 0 0 0"); sys.exit()
cols = np.bincount(xs, minlength=a.shape[1]); on = cols > 3
runs, start = [], None
for x, v in enumerate(list(on) + [False]):
    if v and start is None: start = x
    if not v and start is not None: runs.append((start, x)); start = None
runs = sorted(sorted(runs, key=lambda r: -cols[r[0]:r[1]].sum())[:2])
if len(runs) < 2: print("0 0 0 0"); sys.exit()
out = []
for x0, x1 in runs:
    sel = (xs >= x0) & (xs < x1); out += [int(np.median(xs[sel])) + fx + 40, int(np.median(ys[sel])) + fy + 60]
print(*out)
PY
}
read -r PX PY QX QY < <(spots q1_plan_view)
echo "INFO corridors on screen: c-p ($PX,$PY) c-q ($QX,$QY)"
check "both corridors are drawn in KiCad's plan view" "$([ "${PX:-0}" -gt 0 ] && [ "${QX:-0}" -gt "${PX:-0}" ] && echo 1 || echo 0)" "see q1_plan_view.png"
X click "$PX" "$PY"; X key a; X key l; X key w; sleep 1
shot q2_cp_done
X click "$QX" "$QY"; X key r; sleep 2
shot q3_cq_rejected
R4=$(ask plan_get '{}'); echo "$R4" | cut -c1-600
J=$(echo "$R4" | sed 's/^MCPCALL plan_get //')
check "plan_get: the person approved c-p in KiCad" "$(echo "$J" | python3 -c 'import sys; t=sys.stdin.read(); print(int("c-p" in t and "approved" in t))')" "$(echo "$R4" | cut -c1-300)"
check "plan_get: the person locked c-p" "$(echo "$J" | grep -q 'c-p locked\|\\"c-p\\",\\"kind\\":\\"corridor\\",\\"change\\":\\"locked' && echo 1 || (echo "$J" | python3 -c 'import sys,re; t=sys.stdin.read(); print(int(bool(re.search(r"c-p[^}]*locked", t))))'))"
check "plan_get: c-p edited by the person (wider)" "$(echo "$J" | python3 -c 'import sys,re; t=sys.stdin.read(); print(int(bool(re.search(r"c-p[^}]*edited by the person", t)) or bool(re.search(r"constraint[^}]*wide", t))))')"
check "plan_get: the person rejected c-q" "$(echo "$J" | python3 -c 'import sys,re; t=sys.stdin.read(); print(int("rejected" in t and "c-q" in t))')"
P=$(plan_info)
check "the Plan panel shows c-p approved and locked, c-q rejected (live)" "$(python3 -c 'import json,sys; d=json.loads(sys.argv[1]); it={i["id"]:i for l in d.get("layers",[]) for i in l["items"]}; p=it.get("c-p",{}); q=it.get("c-q",{}); print(int(p.get("state")=="approved" and p.get("locked") is True and q.get("state")=="rejected"))' "$P" 2>/dev/null || echo 0)" "$(echo "$P" | head -c 400)"
for _ in $(seq 60); do grep -q "The person judged" "$OUT/standin.log" 2>/dev/null && break; sleep 0.5; done
check "the agent that asked is woken with the person's decision in KiCad" "$(grep -q "The person judged.*c-p" "$OUT/standin.log" && echo 1 || echo 0)" "$(grep -o 'The person judged[^"]*' "$OUT/standin.log" | head -2)"
shot q4_panel
# Q6 layer approval, by the action and by KiCad's right-click menu
if grep -qa "pcbnew.Plan.approveLayer" "$(dirname "$KICAD_BIN")/_pcbnew.kiface" 2>/dev/null; then
  # a layer's status from plan_get: the MCP answer decoded down to the plan's layers list (corridors name layers too, so no regex)
  layer_status() { ask plan_get '{}' | sed 's/^MCPCALL plan_get //' | python3 -c '
import json, sys
def walk(o):
    if isinstance(o, str):
        try: o = json.loads(o)
        except Exception: return None
        return walk(o) if isinstance(o, (dict, list)) else None
    if isinstance(o, dict):
        ls = o.get("layers")
        if isinstance(ls, list) and all(isinstance(l, dict) and "layer" in l and "id" not in l for l in ls) and ls: return ls
        o = list(o.values())
    if isinstance(o, list):
        for v in o:
            r = walk(v)
            if r: return r
    return None
try: ls = walk(json.loads(sys.stdin.read())) or []
except Exception: ls = []
print(next((l.get("status", "?") for l in ls if l.get("layer") == sys.argv[1]), "?"))' "$1"; }
  X key Escape; sleep 0.5
  # the menu: B.Cu active (PageDown), a right-click off the board (left of it: nothing of the plan there, so the selection stays empty),
  # then the menu by keyboard: Cancel, Add Note, Draw Corridor, Draw Plane Region, Approve Layer (plan_tool.cpp's menu, nothing selected)
  X key Next; sleep 1
  S=$(( (QX - PX) / 33 )); [ "$S" -gt 0 ] || S=10
  MX=$(( PX - 22 * S )); [ "$MX" -gt $((FX + 60)) ] || MX=$((FX + 60))
  X click "$MX" "$PY" 3; sleep 1; shot q5_menu
  for _ in 1 2 3 4 5; do X key Down; done; sleep 0.5; shot q5_menu_approve
  X key Return; sleep 2; shot q5_layer_b
  LB=$(layer_status B.Cu)
  check "layer approval from KiCad's right-click menu (Approve Layer on B.Cu)" "$([ "$LB" = approved ] && echo 1 || echo 0)" "B.Cu: $LB (see q5_menu_approve.png)"
  # F.Cu (PageUp) stays the person's: an agent's kicad_ipc can neither run the plan view's approve-layer nor send ApprovePlan (the hub
  # refuses both on every build; the fork refuses agent clients too from bp.13 batch 4), and plan_approve only asks
  X key Prior; sleep 1
  agent_ipc() { ask kicad_ipc "{\"path\":\"$PCB\",\"type\":\"$1\",\"message\":$2}"; }
  RA=$(agent_ipc kiapi.common.commands.RunAction '{"action":"pcbnew.Plan.approveLayer"}')
  check "an agent's RunAction pcbnew.Plan.approveLayer is refused" "$(echo "$RA" | grep -q "the person's alone" && echo 1 || echo 0)" "$(echo "$RA" | cut -c1-200)"
  RB=$(agent_ipc kiapi.backplane.plan.ApprovePlan "{\"board\":{\"type\":\"DOCTYPE_PCB\"},\"layer\":\"F.Cu\",\"approve\":true}")
  check "an agent's ApprovePlan is refused" "$(echo "$RB" | grep -q "the person's alone" && echo 1 || echo 0)" "$(echo "$RB" | cut -c1-200)"
  # past the hub's guard, straight to KiCad as an agent's client: the fork itself refuses the approval actions (bp.13 batch 4, f74ea40f83;
  # before the busy check, so in plan view too). Batch 3 does not: there it is not sent (it would approve F.Cu), only told
  SOCK=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1] or "{}").get("sock",""))' "$R")
  B4=$(ask kicad_ipc "{\"path\":\"$PCB\",\"type\":\"kiapi.backplane.GetCapabilities\",\"message\":{}}" | grep -q 'rules-insert-position' && echo 1 || echo 0)
  if [ "$B4" = 1 ] && [ -S "$SOCK" ]; then
    for act in pcbnew.Plan.approveLayer pcbnew.Plan.rejectLayer pcbnew.Plan.accept pcbnew.Plan.reject; do
      A=$(python3 "$KEMBED_LIB/agent_runaction.py" "$SOCK" "$act" agent:t1 2>&1 | tail -1)
      check "KiCad itself refuses an agent's RunAction $act" "$(echo "$A" | grep -q "approval is the person's" && echo 1 || echo 0)" "$A"
    done
  else echo "XFAIL KiCad itself refusing an agent's approval actions: not this build (bp.13 batch 4), or no socket ($SOCK)"; fi
  ask plan_approve '{"layer":"F.Cu","note":"F.Cu"}' >/dev/null; sleep 2
  LF0=$(layer_status F.Cu)
  check "F.Cu is still not approved after the agent's attempts and its plan_approve" "$([ "$LF0" != approved ] && echo 1 || echo 0)" "F.Cu: $LF0"
  # the Plan panel's Approve (plan.approve: KiCad's ApprovePlan as the person), with plan view on
  PA=$(hub_rpc "[{\"method\":\"plan.approve\",\"params\":{\"pro\":\"$PROJ/ecc83-pp.kicad_pro\",\"layer\":\"F.Cu\",\"approve\":true}}]" 2>&1 | tail -1)
  sleep 2; LF=$(layer_status F.Cu); shot q6_layers
  check "the Plan panel's Approve (ApprovePlan as the person) approves F.Cu in plan view" "$([ "$LF" = approved ] && echo 1 || echo 0)" "F.Cu: $LF; plan.approve: $(echo "$PA" | cut -c1-200)"
else
  echo "SKIP layer approval: this KiCad has no pcbnew.Plan.approveLayer"
fi
# Q7: the plan's other layers are hit (bp.13 batch 4)
CAPS7=$(ask kicad_ipc "{\"path\":\"$PCB\",\"type\":\"kiapi.backplane.GetCapabilities\",\"message\":{}}")
XL=${PLAN_XLAYER:-$(echo "$CAPS7" | grep -q 'rules-insert-position' && echo expect || echo xfail)}
plan_field() { ask plan_get '{}' | sed 's/^MCPCALL plan_get //' | python3 -c '
import json, sys
try: o = json.loads(sys.stdin.read()); r = json.loads(o["result"]["content"][0]["text"])
except Exception: print("?"); sys.exit()
p = r.get("plan", {})
if sys.argv[1] == "rev": print(r.get("summary", {}).get("revision", "?"))
else: print(next((c.get("width") for c in p.get("corridors", []) if c.get("id") == sys.argv[1]), "?"))' "$1"; }
active_layer() { ask kicad_ipc "{\"path\":\"$PCB\",\"type\":\"kiapi.board.commands.GetActiveLayer\",\"message\":{\"board\":{\"type\":\"DOCTYPE_PCB\",\"board_filename\":\"ecc83-pp.kicad_pcb\",\"project\":{\"name\":\"ecc83-pp\",\"path\":\"$PROJ\"}}}}" | grep -o 'BL_[A-Za-z_]*' | head -1; }
xcheck() { # $1 what, $2 1|0, $3 detail: a FAIL only where the build is expected to have the fix
  if [ "$XL" = expect ] || [ "$2" = 1 ]; then check "$1" "$2" "$3"; else echo "XFAIL $1 (this build's hit test sees only the active layer; fixed in bp.13 batch 4): $3"; fi
}
# no Escape here: with nothing selected it ends KiCad's plan tool (the plan stays drawn, the select tool takes the clicks)
X key Next; sleep 1                                                # B.Cu active, nothing selected
# c-r's middle (147.5, 129) on the screen, from c-p's (131, 113.3) and c-q's (164, 113.3); the B.Cu plane stops at y 108, so nothing of
# the active layer is under the click
read -r RX RY < <(python3 -c 'import sys; px,py,qx=map(float,sys.argv[1:]); s=(qx-px)/33; print(round(px+(147.5-131)*s), round(py+(129-113.3)*s))' "$PX" "$PY" "$QX")
W0=$(plan_field c-r); REV0=$(plan_field rev); L0=$(active_layer)
echo "INFO Q7 ($XL): c-r at ($RX,$RY), width $W0, active $L0"
X click "$RX" "$RY"; sleep 0.8; X key w; sleep 1.5; shot q7_cross_layer
W1=$(plan_field c-r); L1=$(active_layer)
xcheck "with B.Cu active, a click on an F.Cu corridor and W widen it" "$(python3 -c 'import sys; a,b=sys.argv[1:]; print(int(a!="?" and b!="?" and float(b)>float(a)+1e-9))' "$W0" "$W1")" "c-r width $W0 -> $W1"
xcheck "the click makes the corridor's layer (F.Cu) active" "$([ "$L1" = BL_F_Cu ] && echo 1 || echo 0)" "active $L0 -> $L1"
# W with nothing selected: nothing changes
# nothing selected: a click off the board, left of it, where nothing of the plan is (Escape would end the plan tool)
S7=$(( (QX - PX) / 33 )); [ "$S7" -gt 0 ] || S7=10
X click "$(( PX - 22 * S7 > FX + 60 ? PX - 22 * S7 : FX + 60 ))" "$PY"; sleep 0.8; REV1=$(plan_field rev); shot q7_before_w; X key w; sleep 1.0; shot q7_after_w; REV2=$(plan_field rev)
BAND=$(python3 - "$OUT/shots/q7_before_w.png" "$OUT/shots/q7_after_w.png" "$FX" "$FY" "$FW" <<'PY'
import sys
import numpy as np
from PIL import Image
a, b = (np.asarray(Image.open(f).convert("RGB")).astype(int) for f in sys.argv[1:3]); fx, fy, fw = map(int, sys.argv[3:6])
# the infobar opens at the top of the canvas, under the toolbars (about 105-150 px into the frame)
band = (slice(fy + 100, fy + 160), slice(fx + 40, fx + fw - 40))
print(int((np.abs(a[band] - b[band]).sum(axis=2) > 60).sum()))
PY
)
xcheck "W with nothing selected says so on KiCad's infobar" "$([ "${BAND:-0}" -gt 1500 ] && echo 1 || echo 0)" "$BAND pixels changed at the top of the canvas (q7_before_w/q7_after_w)"
check "W with nothing selected changes nothing in the plan" "$([ "$REV1" != "?" ] && [ "$REV1" = "$REV2" ] && echo 1 || echo 0)" "revision $REV1 -> $REV2"
shot q7_nothing_selected
# Q8: Escape with nothing selected ends plan view's tool. From bp.13 batch 5 (b590534148) it leaves plan view too: the plan is no longer
# drawn and the board's layers come back. Batch 4 and earlier end only the tool and leave the plan drawn: told (XFAIL), not failed.
# Batch 5 has no capability of its own; PLAN_ESC=expect makes it count (set it once batch 5 is installed)
plan_px() { python3 - "$OUT/shots/$1.png" "$FX" "$FY" "$FW" "$FH" <<'PY'
import sys
import numpy as np
from PIL import Image
a = np.asarray(Image.open(sys.argv[1]).convert("RGB")).astype(int); fx, fy, fw, fh = map(int, sys.argv[2:6])
a = a[fy + 160:fy + fh - 40, fx + 40:fx + fw - 280]; r, g, b = a[..., 0], a[..., 1], a[..., 2]
orange = (r > 120) & (r - b > 70) & (g > 60) & (g < r - 10) & (g > b + 20); green = (g > 150) & (g - r > 60) & (g - b > 30)
print(int(orange.sum() + green.sum()))
PY
}
X key Prior; sleep 0.8                                             # F.Cu active again: its corridors drawn
shot q8_before_esc; P0=$(plan_px q8_before_esc)
X key Escape; sleep 1.2; shot q8_after_esc; P1=$(plan_px q8_after_esc)
ESC=${PLAN_ESC:-xfail}
echo "INFO Q8 ($ESC): plan pixels $P0 before Escape, $P1 after"
if [ "$ESC" = expect ] || [ "${P1:-1}" -lt $(( ${P0:-0} / 10 + 1 )) ]; then
  check "Escape with nothing selected leaves plan view (the plan no longer drawn)" "$([ "${P0:-0}" -gt 500 ] && [ "${P1:-1}" -lt $(( P0 / 10 + 1 )) ] && echo 1 || echo 0)" "plan pixels $P0 -> $P1 (q8_before_esc, q8_after_esc)"
  # and Ctrl+Shift+R starts it again
  hotkey; sleep 1.2; shot q8_again; P2=$(plan_px q8_again)
  check "Ctrl+Shift+R starts plan view again" "$([ "${P2:-0}" -gt $(( P0 / 2 )) ] && echo 1 || echo 0)" "plan pixels $P2"
else
  echo "XFAIL Escape with nothing selected leaves plan view (bp.13 batch 5): this build ends only the tool; the plan stays drawn ($P0 -> $P1)"
fi
finish_suite "embed_plan_person_e2e ($EMBED_MODE)"
