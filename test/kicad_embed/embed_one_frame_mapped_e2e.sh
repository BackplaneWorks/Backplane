#!/bin/bash
# Linux local-map: a fork show no longer hides KiCad's other frames in wx (embed.rs hides_others); the client unmaps the previous frame
# itself. Every road a show comes by leaves exactly one KiCad editor frame mapped in the pane, never two (one under or over the other):
#   O1  the pane's own: Schematic (a first show through the hub), 3D viewer (the same), Board (a local switch), Schematic again
#   O2  an agent's show through the hub (editor.kicad.show, no client cid: "agent"): the Schematic, then the Board
#   O3  a remount (the pane's re-show of a frame that stayed unmapped: editor.kicad.show remount)
#   O4  a KiCad dialog of the board (Find, Ctrl+F, modeless) open, the Schematic and back to the Board: the dialog on screen after as before
# After each road, the KiCad frames X has mapped are sampled ~40 times over a few seconds: O1-O3 pass when no sample has two mapped and the
# last has the frame asked for. KiCad's own switches (Ctrl+E) and the pre-warm's hidden frames are embed_ctrl_e_e2e's and
# embed_prewarm_e2e / embed_perf_e2e's. Fork mode, X11. Copies only.
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_one_frame_mapped_e2e: fork only"; exit 0; }
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
start_xvfb_and_wm; start_hub
for _ in $(seq 60); do hub_rpc "[]" >/dev/null 2>&1 && break; sleep 0.5; done
PRO=$(find "$OUT/proj" -maxdepth 1 -name "*.kicad_pro" | head -1)
echo '[{"select":0},{"wait":2200},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},{"kicad":{"wait_painted":60000,"kind":"PCB","tag":"ready"}},{"mark":"ready"},{"wait":900000}]' >"$OUT/script.json"
HOME="$OUT/fh" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 1000 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"ready","event":"mark"' 480 || { echo "FAIL the board never opened in KiCad"; exit 1; }
R=$(last_kicad_report "$OUT/client.log" ready)
KP=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1]).get("pid",0))' "$R")
sleep 2
ctl_at() { python3 - "$OUT/client.log" "$1" <<'PY'
import json, sys
last = None
for l in open(sys.argv[1], errors="ignore"):
    if "GPUI_METRIC " in l and '"controls":[' in l:
        try: last = json.loads(l.split("GPUI_METRIC ", 1)[1])["controls"]
        except Exception: pass
c = next((c for c in (last or []) if c.get("id") == sys.argv[2] and c.get("rect")), None)
print(f'{int(c["rect"]["x"] + c["rect"]["w"] / 2)} {int(c["rect"]["y"] + c["rect"]["h"] / 2)}' if c else "")
PY
}
# the KiCad editor frames X has mapped now: "PCB SCH" etc. (by title)
mapped() { python3 "$DRIVER_PY" list-windows --pid "$KP" 2>/dev/null | python3 -c 'import json,sys
k=lambda t: "SCH" if "Schematic Editor" in t else "PCB" if "PCB Editor" in t else "3D" if t.startswith("3D Viewer") else ""
print(" ".join(sorted(k(w.get("title") or "") for w in json.load(sys.stdin) if w.get("shown") and not w.get("override_redirect") and k(w.get("title") or ""))))'; }
# samples for 2.5 s: "<max mapped at once> <the last sample>"
watch_maps() { local most=0 last="" n; for _ in $(seq 40); do last=$(mapped); n=$(wc -w <<<"$last"); [ "$n" -gt "$most" ] && most=$n; sleep 0.03; done; echo "$most|$last"; }
ROADS=""; BAD=0
road() { # $1 name, $2 the frame asked for; the road itself already started
  local r most last; r=$(watch_maps); most=${r%%|*}; last=${r#*|}
  echo "INFO road $1: at most $most mapped at once, after: '$last'"
  ROADS="$ROADS $1:$most:${last// /+}"
  { [ "$most" -le 1 ] && [ "$last" = "$2" ]; } || BAD=$((BAD + 1))
}
click_rail() { local at; at=$(ctl_at "rail:$1"); [ -n "$at" ] && python3 "$XINPUT_PY" click $at; }
show() { hub_rpc "[{\"method\":\"editor.kicad.show\",\"timeout_ms\":15000,\"params\":{\"project\":\"$PRO\",\"frame\":\"$1\"$2}}]" >/dev/null 2>&1 & }

# O1 the pane's own
B0=$BAD
click_rail Schematic; road pane-sch-first SCH; sleep 3
click_rail 3D; road pane-3d-first 3D; sleep 4
click_rail Board; road pane-board-local PCB; sleep 2
click_rail Schematic; road pane-sch-local SCH; sleep 2
check "O1 the pane's own switches (first shows through the hub, local ones): one KiCad frame mapped at a time" "$([ "$BAD" = "$B0" ] && echo 1 || echo 0)" "$ROADS"
# O2 an agent's show through the hub
B0=$BAD; ROADS=""
show SCH ""; road agent-sch SCH; sleep 2
show PCB ""; road agent-pcb PCB; sleep 2
check "O2 an agent's show through the hub: one KiCad frame mapped at a time" "$([ "$BAD" = "$B0" ] && echo 1 || echo 0)" "$ROADS"
# O3 a remount
B0=$BAD; ROADS=""
show PCB ',"remount":true'; road remount-pcb PCB; sleep 2
check "O3 a remount: one KiCad frame mapped at a time" "$([ "$BAD" = "$B0" ] && echo 1 || echo 0)" "$ROADS"
# O4 a modeless dialog of the board, the Schematic and back
dialog_mapped() { python3 "$DRIVER_PY" list-windows --pid "$KP" 2>/dev/null | python3 -c 'import json,sys
print(sum(1 for w in json.load(sys.stdin) if w.get("shown") and not w.get("override_redirect") and (w.get("title") or "").startswith("Find")))'; }
read -r PX PY <<<"$(python3 "$DRIVER_PY" list-windows --pid "$KP" 2>/dev/null | python3 -c 'import json,sys
ws=[w for w in json.load(sys.stdin) if w.get("shown") and "PCB Editor" in (w.get("title") or "")]
print(ws[0]["root_x"]+ws[0]["w"]//2, ws[0]["root_y"]+ws[0]["h"]//2) if ws else print("640 400")')"
python3 "$XINPUT_PY" click "$PX" "$PY"; sleep 0.8; python3 "$XINPUT_PY" chord ctrl f; sleep 2
D0=$(dialog_mapped)
click_rail Schematic; sleep 2.5; D1=$(dialog_mapped)
click_rail Board; sleep 2.5; D2=$(dialog_mapped); M2=$(mapped)
echo "INFO the board's Find dialog mapped: opened $D0, on the Schematic $D1, back on the Board $D2; frames '$M2'"
check "O4 a board dialog open, Schematic and back: the dialog on screen again with the board" "$([ "$D0" -ge 1 ] && [ "$D2" -ge 1 ] && [ "$M2" = PCB ] && echo 1 || echo 0)" "opened $D0, schematic $D1, back $D2, frames '$M2'"
python3 "$XINPUT_PY" key Escape
kill "$CP" 2>/dev/null; wait "$CP" 2>/dev/null || true
finish_suite "embed_one_frame_mapped_e2e ($EMBED_MODE)"
