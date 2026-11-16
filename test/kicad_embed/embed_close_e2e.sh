#!/bin/bash
# KiCad's own Close (Ctrl+W) in the embedded board editor: the pane follows KiCad, never "Starting KiCad…" for ever (lane M, Cmd+W).
#   C1  nothing unsaved: Ctrl+W closes the PCB frame; the pane leaves Edit for View (Edit offered again); Edit opens a fresh frame
#   C2  an unsaved edit, Ctrl+W, KiCad's prompt answered Cancel (Escape): the editor stays, still modified, the pane in Edit
#   C3  the same, the prompt answered Save (Return, its default): the board is written, the frame closes, the pane goes to View
# XTest as a person types; fork only.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_close_e2e: fork only"; exit 0; }
[ "${1:-}" != "" ] && CLIENT_BIN=$1
[ "${2:-}" != "" ] && HUB_BIN=$2
[ "${3:-}" != "" ] && OUT=$(mkdir -p "$3" && cd "$3" && pwd)
discover_bins; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PCB=$(ls "$OUT"/proj/*.kicad_pcb | head -1)
start_xvfb_and_wm; start_hub
echo '[{"select":0},{"wait":2200},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},{"kicad":{"wait_painted":60000,"kind":"PCB","tag":"ready"}},{"mark":"ready"},{"wait":200000}]' >"$OUT/script.json"
HOME="$OUT/fh" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 240 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"ready","event":"mark"' 480 || { echo "FAIL the board never opened in KiCad"; exit 1; }
R=$(last_kicad_report "$OUT/client.log" ready)
SOCK=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1]).get("sock",""))' "$R"); KP=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1]).get("pid",0))' "$R")
sleep 2

# the PCB frame now (the hub's frames, from the pane's report is only taken at script steps: X's own list here)
pcb_frame() { python3 "$DRIVER_PY" list-windows --match "PCB Editor" 2>/dev/null | python3 -c 'import json,sys; ws=[w for w in json.load(sys.stdin) if w.get("shown") and w["w"]>300]; print(ws[0]["xid"], ws[0]["root_x"], ws[0]["root_y"], ws[0]["w"], ws[0]["h"]) if ws else print("")'; }
# the client's controls now (the latest snapshot): whether this one is drawn, and its middle
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
modified() { python3 "$IPC_PY" modified "$SOCK" pcb 2>/dev/null | python3 -c 'import json,sys; print(json.load(sys.stdin).get("modified"))' 2>/dev/null || echo "?"; }
keys_to_kicad() { read -r F FX FY FW FH <<<"$(pcb_frame)"; [ -n "${F:-}" ] && python3 "$XINPUT_PY" click $((FX + FW / 2)) $((FY + FH - 120)); sleep 0.8; }
edit_again() {  # Edit pressed again: a fresh PCB frame shown and painted
  local at; at=$(ctl_at ui:Edit); [ -n "$at" ] || { echo ""; return; }
  python3 "$XINPUT_PY" click $at; for _ in $(seq 120); do [ -n "$(pcb_frame)" ] && break; sleep 0.25; done; sleep 2; pcb_frame
}

# C1
N=$(wc -l <"$OUT/client.log"); keys_to_kicad
python3 "$XINPUT_PY" chord ctrl w; sleep 3
check "C1 Ctrl+W with nothing unsaved closes KiCad's board editor" "$([ -z "$(pcb_frame)" ] && echo 1 || echo 0)"
check "C1 the pane leaves Edit for View (Edit offered again), no wait for the frame" "$(tail -n +"$N" "$OUT/client.log" | grep -q '"kicad_frame_closed":"PCB"' && [ -n "$(ctl_at ui:Edit)" ] && echo 1 || echo 0)" "$(tail -n +"$N" "$OUT/client.log" | grep -o '"kicad_frame_closed[^}]*' | head -1); Edit at $(ctl_at ui:Edit)"
F=$(edit_again)
check "C1 Edit again opens a fresh board editor in the pane" "$([ -n "$F" ] && echo 1 || echo 0)" "$F"

# C2: an unsaved edit, Ctrl+W, Cancel
python3 "$IPC_PY" agent-add-track "$SOCK" 100 100 104 100 0.25 3 >/dev/null 2>&1 || true; sleep 1
echo "INFO modified before Ctrl+W: $(modified)"
N=$(wc -l <"$OUT/client.log"); keys_to_kicad
python3 "$XINPUT_PY" chord ctrl w; sleep 1.5
PROMPT=$(python3 "$DRIVER_PY" list-windows --pid "$KP" 2>/dev/null | python3 -c 'import json,sys; ws=[w for w in json.load(sys.stdin) if w.get("shown") and not w.get("override_redirect") and w["w"]>200 and "Editor" not in (w.get("title") or "")]; print(ws[0]["xid"] if ws else "")')
echo "INFO KiCad's save prompt: ${PROMPT:-none}"
python3 "$XINPUT_PY" key Escape; sleep 1.5
check "C2 Cancel in KiCad's save prompt keeps the editor: the frame shown, still modified, the pane in Edit" "$([ -n "$(pcb_frame)" ] && [ "$(modified)" = True ] && ! tail -n +"$N" "$OUT/client.log" | grep -q '"kicad_frame_closed"' && echo 1 || echo 0)" "prompt ${PROMPT:-none}, modified $(modified)"

# C3: Ctrl+W, Save
SHA0=$(sha256sum "$PCB" | cut -d' ' -f1)
N=$(wc -l <"$OUT/client.log"); keys_to_kicad
python3 "$XINPUT_PY" chord ctrl w; sleep 1.5
python3 "$XINPUT_PY" key Return; sleep 3
check "C3 Save in KiCad's prompt writes the board and closes the editor; the pane goes to View" "$([ "$(sha256sum "$PCB" | cut -d' ' -f1)" != "$SHA0" ] && [ -z "$(pcb_frame)" ] && tail -n +"$N" "$OUT/client.log" | grep -q '"kicad_frame_closed":"PCB"' && [ -n "$(ctl_at ui:Edit)" ] && echo 1 || echo 0)" "board written $([ "$(sha256sum "$PCB" | cut -d' ' -f1)" != "$SHA0" ] && echo yes || echo no)"
kill "$CP" 2>/dev/null || true; wait "$CP" 2>/dev/null || true
finish_suite "embed_close_e2e ($EMBED_MODE)"
