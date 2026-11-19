#!/bin/bash
# Follow agent in the embedded KiCad (host kicad/follow.rs `pane`, agents/design.rs). Board ecc83-pp; the agent's edits are kicad_edit
# moves of one part (R1, C1, U1) through the hub, into the open KiCad. Each check needs the edit to have landed (saved by the editor's KiCad).
#   P0  Follow off, Board on show: a board edit sends KiCad nothing (no SelectItems in the hub's log, KiCad's selection unchanged)
#   P1  Follow off, the rail's Schematic page: a board edit leaves the frame on show as it is (the edit's own view push is not sent
#       while the page holds KiCad; on main it switches to the Board) and sends KiCad nothing
#   P2  Follow on, Board on show: a board edit selects the moved part in KiCad (GetSelection) and fits the view to it (the hub's
#       SelectItems answered ok), and the frame on show stays the Board
#   P3  Follow on, Schematic on show: a board edit sends nothing (the hub says Elsewhere: the line names the board), the Schematic stays on
#       show, and its selection is unchanged (no cross-probe into it)
#   P4  the person's hand on KiCad's canvas pauses Follow (as on Backplane's viewer): the next edit is not followed; Resume, and the next is
#   (a modal: KiCad refuses the agent's edit itself while one is open, so no edit lands to hold; `decide`'s Hold is unit-tested)
#   P6  an agent's explicit viewer_show of the board still switches the pane from the Schematic to the Board
#   P7  (issue 1355) on the Board page with the strip's Schematic on show, an explicit viewer_show of the board brings the Board; the
#       agent's read-back names it
#   P8  the same with a board edit instead: the Schematic stays
# Copies only (seed_project.py), fork mode with SelectItems (bp.13+: canvas_input_ago_ms, tool_busy).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_follow_pane_e2e: fork only"; exit 0; }
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
DESC="$(dirname "$KICAD_BIN")/../share/backplane/api.desc"
{ [ -f "$DESC" ] && grep -aq "canvas_input_ago_ms" "$DESC"; } || { echo "embed_follow_pane_e2e: $KICAD_BIN has no canvas_input_ago_ms (bp.13): nothing to check"; exit 0; }
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PROJ="$OUT/proj"; PCB="$PROJ/ecc83-pp.kicad_pcb"; mkdir -p "$OUT/shots"
start_xvfb_and_wm
export BP_STREAM_N=1 BP_STREAM_MS=10 BP_STANDIN_MCPCALL_MAX=200000
start_hub

calls() { local n; n=$(grep -c "^MCPCALL" "$OUT/standin.log" 2>/dev/null) || true; echo "${n:-0}"; }
ask() { local N; N="$1-$(date +%s%N)"; local before; before=$(calls)
  local text; text=$(python3 -c 'import json,sys; print(json.dumps("mcpcall:" + json.dumps({"name": sys.argv[1], "arguments": json.loads(sys.argv[2])})))' "$1" "$2")
  hub_rpc "[{\"method\":\"turn.start\",\"params\":{\"thread\":\"t1\",\"text\":$text,\"msg\":\"mcl$N\"}}]" >>"$OUT/rpc.log" 2>&1 || true
  for _ in $(seq 480); do [ "$(calls)" -gt "$before" ] && break; sleep 0.25; done; sleep 1.5
  { grep "^MCPCALL $1 " "$OUT/standin.log" || true; } | tail -1 | sed "s/^MCPCALL $1 //" | python3 -c 'import json,sys
try: o=json.loads(sys.stdin.read()); print(o["result"]["content"][0]["text"].replace("\n"," "))
except Exception as e: print("?")'; }
# an agent's saving kicad_edit: part $1 moved by $2 mm in x
edit_move() { ask kicad_edit "{\"path\":\"$PCB\",\"ops\":[{\"move\":\"$1\",\"by\":[$2,0]}]}"; }
landed() { echo "$1" | grep -q '"served_by":"embedded"' && echo "$1" | grep -q "\"saved\":\"$PCB\""; }
hub_lines() { wc -l <"$OUT/hub.log"; }
since() { tail -n +"$(( $1 + 1 ))" "$OUT/hub.log"; }
sel() { python3 "$IPC_PY" selection "$SOCK" "$1" 2>/dev/null || echo '{}'; }
has_ref() { python3 -c 'import json,sys; print(int(sys.argv[2] in json.loads(sys.argv[1]).get("refs",[])))' "$1" "$2" 2>/dev/null || echo 0; }
# the editor frames mapped on screen now (X: KiCad's PCB Editor / Schematic Editor windows that are viewable)
shown_frame() { python3 "$DRIVER_PY" list-windows --pid "$KP" 2>/dev/null | python3 -c 'import json,sys
t=[w.get("title") or "" for w in json.load(sys.stdin) if w.get("shown") and w.get("w",0)>=200]
print(",".join([k for k,s in (("PCB","PCB Editor"),("SCH","Schematic Editor")) if any(s in x for x in t)]) or "-")' 2>/dev/null || echo "?"; }
# a click on a control the window drew: its rect from the latest `controls` or `rects` metric (the Follow toggle is only a rect)
ctl() {
  local xy; xy=$(python3 - "$OUT/client.log" "$DRIVER_PY" "$GPUI_XID" "$1" "${3:-0}" <<'PY'
import json, subprocess, sys
log, drv, gx_s, cid, dx = sys.argv[1:6]
rect = None
for line in open(log, encoding="utf-8", errors="replace"):
    if "GPUI_METRIC " not in line or cid not in line: continue
    try: o = json.loads(line.split("GPUI_METRIC ", 1)[1])
    except Exception: continue
    for c in (o.get("controls") or []) + (o.get("rects") or []):
        if c.get("id") == cid: rect = c.get("rect") or c
if not rect: sys.exit(0)
gid = int(gx_s, 0); gx = gy = 0
for w in json.loads(subprocess.check_output([sys.executable, drv, "list-windows"], text=True)):
    if w["xid_int"] == gid: gx, gy = w["root_x"], w["root_y"]; break
print(int(gx + rect["x"] + rect["w"] / 2 + float(dx)), int(gy + rect["y"] + rect["h"] / 2))
PY
)
  [ -n "$xy" ] || { echo "DIAG ctl: '$1' not drawn"; return 1; }
  # shellcheck disable=SC2086
  python3 "$XINPUT_PY" click $xy; sleep "${2:-1.5}"
}
# the person back in Backplane's composer (left of its Send button): never a fixed point of the thread, where a file link to the board
# can come to sit as the agent's calls add rows (a click there opened the Board page)
person_in_chat() { ctl send 0.8 -200 || { python3 "$XINPUT_PY" click 400 500; sleep 0.8; }; }

cat >"$OUT/script.json" <<'JSON'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},
 {"kicad":{"wait_painted":90000,"kind":"PCB","tag":"ready"}},{"click":"kicad-tab:SCH"},{"kicad":{"wait_painted":90000,"kind":"SCH","tag":"sch"}},
 {"click":"kicad-tab:PCB"},{"wait":2500},{"kicad":{"report":true,"tag":"placed"}},{"mark":"edit_ready"},{"wait":900000},{"mark":"done"}]
JSON
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_HOLD=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 1000 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"edit_ready","event":"mark"' 300 || { echo "FAIL the board never opened in KiCad"; tail -n 30 "$OUT/client.log"; exit 1; }
R=$(last_kicad_report "$OUT/client.log" placed)
read -r SOCK GPUI_XID KP FX FY FW FH < <(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); f=next((f for f in r.get("frames",[]) if f.get("xid")==r.get("shown")),{}); print(r.get("sock") or "-", hex(int(r.get("toplevel") or 0)), int(r.get("pid") or 0), f.get("root_x",0), f.get("root_y",0), f.get("w",0), f.get("h",0))' "$R")
[ "$KP" -gt 0 ] && track_pid "$KP"
wait_for_ipc "$SOCK" 80
person_in_chat; sleep 2
echo "INFO frame on show at start: $(shown_frame)"

# P0: Follow off, Board on show
S0=$(sel pcb); L=$(hub_lines)
A=$(edit_move R1 0.5); echo "INFO P0 kicad_edit: $(echo "$A" | cut -c1-300)"; sleep 2
check "P0 the agent's board edit landed in the open KiCad" "$(landed "$A" && echo 1 || echo 0)" "$(echo "$A" | cut -c1-200)"
check "P0 Follow off: nothing sent to KiCad (no follow call), selection unchanged" "$(since "$L" | grep -q '^follow: ' && echo 0 || ([ "$(sel pcb)" = "$S0" ] && echo 1 || echo 0))" "$(since "$L" | grep '^follow: ' | head -3) sel $S0 -> $(sel pcb)"

# P1: Follow off, the rail's Schematic page: the edit's implicit view push (to the board) is not sent while the page holds KiCad, so
# the frame does not switch (on main it opens the Board page and its frame)
ctl "rail:Schematic" 4; person_in_chat; F1=$(shown_frame); SS1=$(sel sch); L=$(hub_lines)
A=$(edit_move R1 -0.5); sleep 3
check "P1 the agent's board edit landed" "$(landed "$A" && echo 1 || echo 0)" "$(echo "$A" | cut -c1-200)"
check "P1 Follow off: the Schematic stays on show after a board edit ($F1 -> $(shown_frame))" "$([ "$F1" = SCH ] && [ "$(shown_frame)" = SCH ] && echo 1 || echo 0)"
check "P1 Follow off: nothing sent to KiCad, the schematic's selection unchanged" "$(since "$L" | grep -q '^follow: ' && echo 0 || ([ "$(sel sch)" = "$SS1" ] && echo 1 || echo 0))" "$(since "$L" | grep '^follow: ' | head -3)"

# P2: Follow on, Board on show
ctl "rail:Board" 4
ctl "follow-agent" 2
person_in_chat
L=$(hub_lines)
A=$(edit_move C1 0.5); sleep 2
S2=$(sel pcb)
check "P2 the agent's board edit landed" "$(landed "$A" && echo 1 || echo 0)" "$(echo "$A" | cut -c1-200)"
check "P2 Follow on: the hub sent SelectItems and KiCad took it" "$(since "$L" | grep -q '^follow: SelectItems t1 .*FK_PCB.*C1.*zoom_box_nm.* -> ok' && echo 1 || echo 0)" "$(since "$L" | grep '^follow: ' | head -3 | cut -c1-300)"
check "P2 KiCad's selection is the moved part ($S2)" "$(has_ref "$S2" C1)"
check "P2 the Board is still the frame on show ($(shown_frame))" "$([ "$(shown_frame)" = PCB ] && echo 1 || echo 0)"
shot p2_followed

# P3: Follow on, Schematic on show: a board edit is said, not shown
ctl "kicad-tab:SCH" 3; person_in_chat
SS3=$(sel sch); L=$(hub_lines)
A=$(edit_move U1 0.5); sleep 2.5
check "P3 the agent's board edit landed" "$(landed "$A" && echo 1 || echo 0)" "$(echo "$A" | cut -c1-200)"
check "P3 the hub decided Elsewhere and sent no SelectItems" "$(since "$L" | grep -q '^follow: Elsewhere t1' && ! since "$L" | grep -q '^follow: SelectItems' && echo 1 || echo 0)" "$(since "$L" | grep '^follow: ' | head -3 | cut -c1-300)"
# the line under Follow agent says where the edit was (the client's drawn text, logged as it changes)
for _ in $(seq 40); do LINE3=$(grep -o '"follow_line":"[^"]*","pane":"[^"]*"' "$OUT/client.log" | tail -1 || true); echo "$LINE3" | grep -q '"pane":"elsewhere"' && break; sleep 0.25; done
check "P3 the status line says the edit was on the board ($LINE3)" "$(echo "$LINE3" | grep -q '"follow_line":"Following: Board · edited U1","pane":"elsewhere"' && echo 1 || echo 0)"
check "P3 the Schematic stays on show ($(shown_frame))" "$([ "$(shown_frame)" = SCH ] && echo 1 || echo 0)"
check "P3 the Schematic's selection is unchanged (no cross-probe): $SS3 -> $(sel sch)" "$([ "$(sel sch)" = "$SS3" ] && echo 1 || echo 0)"
shot p3_elsewhere

# P4: the person's hand on the canvas pauses it; Resume starts it again
ctl "kicad-tab:PCB" 3
MX=$((FX + FW / 2)); MY=$((FY + FH / 2))
L0=$(hub_lines)
python3 "$XINPUT_PY" click "$MX" "$MY"; sleep 0.4; python3 "$XINPUT_PY" key Escape; sleep 1.5
person_in_chat
S4=$(sel pcb); L=$(hub_lines)
A=$(edit_move R1 0.5); sleep 2
check "P4 the agent's board edit landed" "$(landed "$A" && echo 1 || echo 0)" "$(echo "$A" | cut -c1-200)"
check "P4 the person's click on the canvas paused Follow: no SelectItems, selection unchanged" "$(! since "$L" | grep -q '^follow: SelectItems' && [ "$(sel pcb)" = "$S4" ] && echo 1 || echo 0)" "$(since "$L" | grep '^follow: ' | head -3 | cut -c1-300)"
check "P4 the pause is the viewer's own (the hub paused thread t1)" "$(since "$L0" | grep -q "^follow: the person's input in KiCad pauses t1\|^follow: Pause t1" && echo 1 || echo 0)" "$(since "$L0" | grep '^follow: ' | head -3 | cut -c1-200)"
# Follow agent pressed while paused turns it off, pressed again on: the viewer's own Resume (on clears the pause)
ctl follow-agent 2; ctl follow-agent 2
person_in_chat; L=$(hub_lines)
A=$(edit_move R1 -0.5); sleep 2
check "P4 resumed: the next edit is followed (R1 selected)" "$(landed "$A" && since "$L" | grep -q '^follow: SelectItems t1 .* -> ok' && [ "$(has_ref "$(sel pcb)" R1)" = 1 ] && echo 1 || echo 0)" "$(since "$L" | grep '^follow: ' | head -3 | cut -c1-300)"

# P6: an agent's explicit viewer_show still switches the pane
# (the rail's Schematic page: a viewer_show of the board then changes the page; the strip's tab over the Board page is the page's own)
ctl "rail:Schematic" 4; person_in_chat
echo "INFO P6 before viewer_show: $(shown_frame)"
V=$(ask viewer_show "{\"path\":\"$PCB\"}"); echo "INFO P6 viewer_show: $(echo "$V" | cut -c1-200)"
for _ in $(seq 40); do [ "$(shown_frame)" = PCB ] && break; sleep 0.25; done
check "P6 an explicit viewer_show of the board switches the pane to the Board ($(shown_frame))" "$([ "$(shown_frame)" = PCB ] && echo 1 || echo 0)"

# P7 (issue 1355): Board page, the person picks the Schematic on the strip; an agent's explicit viewer_show of the board brings the Board
ctl "kicad-tab:SCH" 3; person_in_chat
echo "INFO P7 before: $(shown_frame)"
V=$(ask viewer_show "{\"path\":\"$PCB\",\"wait\":true}"); echo "INFO P7 viewer_show: $(echo "$V" | cut -c1-300)"
for _ in $(seq 40); do [ "$(shown_frame)" = PCB ] && break; sleep 0.25; done
check "P7 an explicit viewer_show of the board on the Board page brings KiCad from the strip's Schematic to the Board ($(shown_frame))" "$([ "$(shown_frame)" = PCB ] && echo 1 || echo 0)"
check "P7 the pane did it as a tab press (kicad_bring PCB in the client's log)" "$(grep -q '"kicad_bring":"PCB"' "$OUT/client.log" && echo 1 || echo 0)"
check "P7 the agent's read-back names the frame on show (kicadFrame PCB)" "$(echo "$V" | grep -q '"kicadFrame":"PCB"' && echo 1 || echo 0)" "$(echo "$V" | grep -o '"kicadFrame":"[A-Z_]*"')"
# P8: the same, but a board edit (its implicit view push): the Schematic stays
ctl "kicad-tab:SCH" 3; person_in_chat
A=$(edit_move C1 -0.5); sleep 3
check "P8 the agent's board edit landed" "$(landed "$A" && echo 1 || echo 0)" "$(echo "$A" | cut -c1-200)"
check "P8 a board edit (no explicit viewer_show) leaves the strip's Schematic on show ($(shown_frame))" "$([ "$(shown_frame)" = SCH ] && echo 1 || echo 0)"

hub_rpc "[{\"method\":\"editor.kicad.close\",\"params\":{\"project\":\"$PROJ/ecc83-pp.kicad_pro\",\"save\":\"discard\"}}]" >/dev/null || true
finish_suite "embed_follow_pane_e2e ($EMBED_MODE)"
