#!/bin/bash
# #1389's follow-up: a library editor KiCad is opening (Ctrl+E) is said in the pane's strip until it comes (Windows' first Symbol Editor
# waits 10-29 s on its libraries). The hub's test hook delays KiCad's editor request (BACKPLANE_TEST_EDITOR_REQUEST_DELAY_MS), so the wait
# is seen on any platform. Checked: within the wait, the placeholder tab (never selected) and the strip's text, the pane still on the frame
# it was on; after it, the editor on show with its tab. Then the person chooses another tab during the wait: the editor is not put on show,
# only its tab appears. Copies only (seed_project.py), fork mode.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_lib_opening_e2e: fork only"; exit 0; }
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PROJ="$OUT/proj"; PCB="$PROJ/ecc83-pp.kicad_pcb"; mkdir -p "$OUT/shots"
start_xvfb_and_wm
export BACKPLANE_KICAD_DEBUG=1
export BACKPLANE_TEST_EDITOR_REQUEST_DELAY_MS=6000
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
 {"click":"kicad-tab:PCB"},{"wait":2500},{"kicad":{"report":true,"tag":"placed"}},{"mark":"edit_ready"},{"wait":300000},{"mark":"done"}]
JSON
HOME="$OUT/fh" BACKPLANE_KICAD_DEBUG=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_HOLD=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 400 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"edit_ready","event":"mark"' 300 || { echo "FAIL the board never opened in KiCad"; tail -n 30 "$OUT/client.log"; exit 1; }
R=$(last_kicad_report "$OUT/client.log" placed)
read -r SOCK GPUI_XID KP FX FY FW FH < <(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); f=next((f for f in r.get("frames",[]) if f.get("xid")==r.get("shown")),{}); print(r.get("sock") or "-", hex(int(r.get("toplevel") or 0)), int(r.get("pid") or 0), f.get("root_x",0), f.get("root_y",0), f.get("w",0), f.get("h",0))' "$R")
[ "$KP" -gt 0 ] && track_pid "$KP"
wait_for_ipc "$SOCK" 80
RT="$HERE/lib/rt_input.py"
# the titles of KiCad's windows on screen now
titles() { python3 "$DRIVER_PY" list-windows --pid "$KP" 2>/dev/null | python3 -c 'import json,sys
print("|".join(sorted({(w.get("title") or "").split(" — ")[-1].split(" - ")[-1] for w in json.load(sys.stdin) if w.get("shown") and w.get("w",0)>=200})))' 2>/dev/null || echo "?"; }
last_tab() { grep -a '"kicad_tab"' "$OUT/client.log" | tail -1 | python3 -c 'import json,sys
l=sys.stdin.read(); print(json.loads(l.split("GPUI_LOG ",1)[1])["kicad_tab"]["to"] if l.strip() else "-")' 2>/dev/null || echo "?"; }
# one case: on frame TAB, the part REF selected, Ctrl+E in KiCad; the EDITOR (window title word) must be on show and its tab the client's
# the ids in the client's last drawn rects (GPUI_METRIC rects)
rects_now() { python3 -c '
import json, sys
last = None
for line in open(sys.argv[1], encoding="utf-8", errors="replace"):
    if "GPUI_METRIC " not in line or "\"rects\"" not in line: continue
    try: o = json.loads(line.split("GPUI_METRIC ", 1)[1])
    except Exception: continue
    r = o.get("rects")
    if isinstance(r, list): last = [x.get("id") for x in r]
print(" ".join(i for i in (last or []) if i))' "$OUT/client.log"; }
has_rect() { rects_now | tr ' ' '\n' | grep -qx "$1" && echo 1 || echo 0; }
# KiCad's Ctrl+E on part REF of frame TAB (its tab clicked first: KiCad has the keys)
press_ctrl_e() { local tab=$1 kind=$2 ref=$3
  ctl "kicad-tab:$tab" 3
  python3 "$RT" click $((FX + FW / 2)) $((FY + FH / 2)) >/dev/null; sleep 0.6; python3 "$RT" key Escape >/dev/null; sleep 0.4
  python3 "$IPC_PY" select-items "$SOCK" "$kind" "$ref" 1 >/dev/null 2>&1 || true; sleep 1
  python3 "$RT" key e ctrl >/dev/null
}
# 1. Symbol Editor: the wait is said, then the editor comes with its tab
press_ctrl_e SCH sch U1; sleep 2.5
R=$(rects_now); TB=$(last_tab); T=$(titles)
echo "INFO during the wait: tab $TB, windows [$T], opening rects: $(echo "$R" | tr ' ' '\n' | grep opening | tr '\n' ' ')"
check "while it opens: the placeholder tab is drawn (kicad-opening:SYMBOL_EDITOR)" "$(has_rect kicad-opening:SYMBOL_EDITOR)"
check "while it opens: the strip says so (kicad:opening)" "$(has_rect kicad:opening)"
check "while it opens: the pane stays on the schematic (tab $TB)" "$([ "$TB" = "SCH" ] && echo 1 || echo 0)"
shot "opening_wait"
sleep 6
TB=$(last_tab); T=$(titles)
echo "INFO after: tab $TB, windows [$T]"
check "then the Symbol Editor is on show ([$T])" "$(echo "$T" | grep -q "Symbol Editor" && echo 1 || echo 0)"
check "and its tab is selected ($TB)" "$([ "$TB" = "SYMBOL_EDITOR" ] && echo 1 || echo 0)"
check "the label is gone" "$([ "$(has_rect kicad:opening)" = 0 ] && echo 1 || echo 0)"
shot "opening_done"
# 2. Footprint Editor, and the person chooses the schematic during the wait: not put on show, its tab appears
L=$(wc -l <"$OUT/hub.log")
press_ctrl_e PCB pcb U1; sleep 2
check "the footprint editor's placeholder is drawn" "$(has_rect kicad-opening:FOOTPRINT_EDITOR)"
ctl "kicad-tab:SCH" 1
sleep 7
TB=$(last_tab); T=$(titles)
echo "INFO after the person's switch: tab $TB, windows [$T]"
tail -n +"$((L + 1))" "$OUT/hub.log" | grep -a "ready after\|not put on show\|the person chose" | sed 's/^/INFO hub: /' || true
check "the person's choice stays on show (tab $TB)" "$([ "$TB" = "SCH" ] && echo 1 || echo 0)"
check "the Footprint Editor was not put on show ([$T])" "$(echo "$T" | grep -q "Footprint Editor" && echo 0 || echo 1)"
check "its tab appears (kicad-tab:FOOTPRINT_EDITOR)" "$(has_rect kicad-tab:FOOTPRINT_EDITOR)"
check "the hub said why" "$(tail -n +"$((L + 1))" "$OUT/hub.log" | grep -aq "not put on show: the person chose" && echo 1 || echo 0)"
shot "opening_switched"
hub_rpc "[{\"method\":\"editor.kicad.close\",\"params\":{\"project\":\"$PROJ/ecc83-pp.kicad_pro\",\"save\":\"discard\"}}]" >/dev/null || true
finish_suite "embed_lib_opening_e2e ($EMBED_MODE)"
