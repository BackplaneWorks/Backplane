#!/bin/bash
# Suite: the person's edits in KiCad's own plan view (Backplane KiCad bp.13: Alt+P, click to select, A accept, R reject, L lock, W wider)
# reach the hub as the person's: the agent's plan_get `changes` says what they did, the Plan panel shows it live, and the agent that asked
# for approval is woken with their decision. Real X input (XTest) into the embedded editor on a copy of the fixture board; fork only.
#   Q1  the agent's plan (two wide corridors on F.Cu either side of the board's centre) is in KiCad, awaiting approval
#   Q2  in KiCad's plan view the person selects c-p, accepts it (A), locks it (L), widens it (W); selects c-q and rejects it (R)
#   Q3  plan_get `changes`: c-p approved, locked, edited by the person, a constraint by the person; c-q rejected
#   Q4  the Plan panel (mgmt.plan, the hub's data) shows c-p approved and locked, c-q rejected, without a refresh
#   Q5  the agent that asked is woken with the person's decision
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_plan_person_e2e: fork only"; exit 0; }
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
grep -qa "planAccept\|Plan.accept" "$(dirname "$KICAD_BIN")/_pcbnew.kiface" 2>/dev/null || { echo "SKIP embed_plan_person_e2e: this KiCad has no plan view edits (bp.13+)"; exit 0; }
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
start_xvfb_and_wm
export BP_STREAM_N=1 BP_STREAM_MS=10
start_hub
PROJ="$OUT/proj"; PCB="$PROJ/ecc83-pp.kicad_pcb"; mkdir -p "$OUT/shots"
cat >"$OUT/script.json" <<'JSON'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},
 {"kicad":{"wait_painted":60000,"kind":"PCB","tag":"ready"}},{"wait":1500},{"click":"kicad:full"},{"wait":2500},{"kicad":{"report":true,"tag":"placed"}},{"mark":"edit_ready"},{"wait":200000}]
JSON
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_HOLD=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 260 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"edit_ready","event":"mark"' 480 || { echo "FAIL the board never opened in KiCad"; tail -n 20 "$OUT/client.log"; exit 1; }
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
PLAN='{"layers":[{"layer":"F.Cu","role":"signal","status":"proposed"}],"groups":[{"id":"g-k","name":"Cathodes","color":"#e8a33d","nets":["Net-(U1A-K)","Net-(U1B-K)"],"status":"proposed"}],"corridors":[{"id":"c-p","group":"g-k","layer":"F.Cu","order":1,"path":[[122,113.3],[140,113.3]],"width":12.0,"status":"proposed"},{"id":"c-q","group":"g-k","layer":"F.Cu","order":2,"path":[[155,113.3],[173,113.3]],"width":12.0,"status":"proposed"}],"planes":[{"id":"p-gnd","net":"GND","layer":"B.Cu","outline":[[[122,91],[173,91],[173,136],[122,136]]],"priority":1,"status":"proposed"}]}'
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
# the keyboard to KiCad: a click on the canvas (its centre is the board's, where nothing of the plan is), then Escape
X click "$((FX + FW / 2))" "$((FY + FH / 2))"; X key Escape
# plan view: KiCad's own action (RunAction pcbnew.Plan.togglePlanView, every build); the hotkey only when the action is refused
# (bp.13 build #2: Alt+P; batch 3 on: Ctrl+Shift+R; the build is told by what it does, never by its version string)
RA=$(ask kicad_ipc "{\"path\":\"$PCB\",\"type\":\"kiapi.common.commands.RunAction\",\"message\":{\"action\":\"pcbnew.Plan.togglePlanView\"}}")
echo "INFO RunAction togglePlanView: $(echo "$RA" | grep -o 'RAS_[A-Z_]*' | head -1)"
if echo "$RA" | grep -q 'RAS_OK'; then VIA=action
elif grep -qa "Routing Plan View" "$(dirname "$KICAD_BIN")/_pcbnew.kiface" 2>/dev/null; then VIA="Ctrl+Shift+R"; X chord ctrl shift r
else VIA="Alt+P"; X chord alt p
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
finish_suite "embed_plan_person_e2e ($EMBED_MODE)"
