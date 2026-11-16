#!/bin/bash
# Suite: the embedded KiCad killed after it was ready (kill -9: a crash looks the same), with an unsaved change on the board.
#   C1  the hub says closed_unexpectedly, by how it ended (signal 9), never "could not start" nor KiCad's last output (Fontconfig ...)
#   C2  the pane says "KiCad closed unexpectedly" and offers one button, Open the editor again (no Try again, no Open the KiCad app)
#   C3  the button starts a new KiCad and the board editor is painted within 30 s of the press
#   C4  what became of the unsaved change: the board file on disk is as last saved; KiCad's autosave (_autosave-*, bp.13's local
#       history, 5 s after a change by default) is kept, not swept; whether the reopened KiCad offers it back is said (its dialog)
# Copies only (seed_project.py), fork mode.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_closed_unexpectedly_e2e: fork only"; exit 0; }
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PROJ="$OUT/proj"; PCB="$PROJ/ecc83-pp.kicad_pcb"; mkdir -p "$OUT/shots"
start_xvfb_and_wm
start_hub
cat >"$OUT/script.json" <<'JSON'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},
 {"kicad":{"wait_painted":60000,"kind":"PCB","tag":"ready"}},{"wait":1500},{"kicad":{"report":true,"tag":"placed"}},{"mark":"edit_ready"},
 {"wait":75000},{"kicad":{"report":true,"tag":"closed"}},{"mark":"closed_seen"},
 {"click":"kicad:open-again"},{"kicad":{"wait_painted":30000,"kind":"PCB","tag":"reopened"}},{"mark":"reopened"},{"wait":8000},{"kicad":{"report":true,"tag":"after"}},{"mark":"done"},{"wait":60000}]
JSON
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_HOLD=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 260 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"edit_ready","event":"mark"' 240 || { echo "FAIL the board never opened in KiCad"; exit 1; }
R=$(last_kicad_report "$OUT/client.log" placed)
read -r KPID SOCK < <(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); print(r.get("pid") or 0, r.get("sock") or "")' "$R")
SHA0=$(sha256sum "$PCB" | cut -d' ' -f1)
# an unsaved change, not saved: CLOSED_EDIT=agent (default) an agent's track committed through the API; person: the person's own edit,
# a click on a part in the editor and R (rotate), real X input
if [ "${CLOSED_EDIT:-agent}" = person ]; then
  read -r FX FY FW FH < <(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); f=next((f for f in r.get("frames",[]) if f.get("xid")==r.get("shown")),{}); print(f.get("root_x",0), f.get("root_y",0), f.get("w",0), f.get("h",0))' "$R")
  python3 "$XINPUT_PY" click "$((FX + FW / 2))" "$((FY + FH / 2))"; sleep 0.6; python3 "$XINPUT_PY" key r; sleep 1; python3 "$XINPUT_PY" key Escape
  echo "INFO unsaved change: the person's own rotate (click at the canvas centre, R)"
else
  ADD=$(python3 "$IPC_PY" agent-add-track "$SOCK" 120.0 100.0 140.0 100.0 0.3 0 2>&1 | tail -1); echo "INFO unsaved change: $(echo "$ADD" | cut -c1-120)"
fi
MODS=$(python3 "$IPC_PY" modified "$SOCK" pcb 2>&1 | tail -1); echo "INFO KiCad says the board is modified: $MODS"
sleep 45   # two autosave intervals
AUTOSAVE=$(ls -d "$PROJ"/_autosave-* "$PROJ"/.history 2>/dev/null | head -3 | tr '\n' ' ' || true)
echo "INFO autosave files before the kill: ${AUTOSAVE:-none}"
kill -KILL "$KPID" 2>/dev/null || true
echo "INFO killed KiCad $KPID with SIGKILL"
wait_for_log "$OUT/client.log" '"name":"closed_seen","event":"mark"' 120 || true
RC=$(last_kicad_report "$OUT/client.log" closed)
ST=$(python3 -c 'import json,sys; r=json.loads(sys.argv[1] or "{}"); print(r.get("state",""), "|", r.get("reason",""))' "$RC")
echo "INFO the hub after the kill: $ST"
check "the hub says closed_unexpectedly, not a failed start" "$(echo "$ST" | grep -q '^closed_unexpectedly' && echo 1 || echo 0)" "$ST"
check "its reason is how KiCad ended (signal 9), not KiCad's output" "$(echo "$ST" | grep -q 'signal 9' && ! echo "$ST" | grep -qi 'fontconfig\|could not start' && echo 1 || echo 0)" "$ST"
SAYS=$(python3 -c 'import json,sys; r=json.loads(sys.argv[1] or "{}"); print(r.get("say") or "")' "$RC")
echo "INFO the pane says: $SAYS"
check "the pane says KiCad closed unexpectedly, not that it could not start" "$(echo "$SAYS" | grep -q '^KiCad closed unexpectedly' && ! echo "$SAYS" | grep -qi 'could not start' && echo 1 || echo 0)" "$SAYS"
check "its words about unsaved work match what is on disk" "$( { [ -n "$(ls -d "$PROJ"/_autosave-* "$PROJ"/.history 2>/dev/null || true)" ] && echo "$SAYS" | grep -q 'kept a copy'; } || { [ -z "$(ls -d "$PROJ"/_autosave-* "$PROJ"/.history 2>/dev/null || true)" ] && echo "$SAYS" | grep -q 'were not kept'; } && echo 1 || echo 0)" "autosaves $(ls "$PROJ"/_autosave-* 2>/dev/null | wc -l); $SAYS"
shot c2_closed
SAY=$(grep -a 'GPUI_METRIC {"controls"' "$OUT/client.log" | grep -c '"kicad:open-again"' || true)
check "the pane offers Open the editor again" "$([ "${SAY:-0}" -gt 0 ] && echo 1 || echo 0)" "kicad:open-again listed $SAY times"
check "and no Try again nor Open the KiCad app beside it" "$(grep -a 'GPUI_METRIC {"controls"' "$OUT/client.log" | tail -1 | grep -q '"kicad:retry"\|"kicad:app"' && echo 0 || echo 1)"
check "the board file on disk is as last saved" "$([ "$(sha256sum "$PCB" | cut -d' ' -f1)" = "$SHA0" ] && echo 1 || echo 0)"
AUTO2=$(ls "$PROJ"/_autosave-* "$PROJ"/*autosave* 2>/dev/null | head -3 | tr '\n' ' ' || true)
echo "INFO autosave files after the kill: ${AUTO2:-none}"
echo "INFO the project folder after the kill: $(ls -A "$PROJ" | tr '\n' ' ')"
find "$PROJ" -newer "$OUT/script.json" -type f ! -path '*/.git/*' 2>/dev/null | head -12 | sed 's/^/INFO written since the start: /' 
[ -z "$AUTOSAVE" ] || check "KiCad's autosave is kept (not swept)" "$([ -n "$AUTO2" ] && echo 1 || echo 0)" "before: $AUTOSAVE"
# C3: the press, and the editor painted within 30 s
wait_for_log "$OUT/client.log" '"name":"reopened","event":"mark"' 120 || true
MS=$(python3 - "$OUT/client.log" <<'PY'
import re, sys
t = open(sys.argv[1]).read()
c = re.search(r'"id":"kicad:open-again"[^\n]*?"event":"click","t_ms":([0-9.]+)', t); r = re.search(r'"tag":"reopened","event":"kicad","t_ms":([0-9.]+)', t)
print(int(float(r.group(1)) - float(c.group(1))) if c and r else -1)
PY
)
RR=$(last_kicad_report "$OUT/client.log" reopened)
PAINTED=$(python3 -c 'import json,sys; r=json.loads(sys.argv[1] or "{}"); print(next((f.get("painted",0) for f in r.get("frames",[]) if f.get("xid")==r.get("shown") and f.get("kind")=="PCB"),0), r.get("state"), r.get("pid"))' "$RR")
echo "INFO reopened: painted/state/pid $PAINTED, ${MS} ms after the press"
# the right outcome is the board painted, or KiCad asking to recover what it autosaved (the person answers it): the pane names the question
ASK=$(python3 -c 'import json,sys; r=json.loads(sys.argv[1] or "{}"); print(next((d for d in (r.get("dialogs") or []) if "Recover" in d), ""), "|", r.get("say") or "")' "$RR")
echo "INFO after the press: $ASK"
if echo "$ASK" | grep -q '^Recover'; then
  check "Open the editor again: KiCad asks to recover the unsaved changes, and the pane names its question" "$(echo "$ASK" | grep -q 'KiCad is asking: Recover' && echo 1 || echo 0)" "$ASK"
else
  check "Open the editor again starts a new KiCad and the board is painted within 30 s" "$([ "$MS" -ge 0 ] && [ "$MS" -lt 30000 ] && [ "$(echo "$PAINTED" | cut -d' ' -f1)" -ge 15 ] && echo 1 || echo 0)" "$PAINTED, ${MS} ms"
fi
shot c3_reopened
wait_for_log "$OUT/client.log" '"name":"done","event":"mark"' 60 || true
RA=$(last_kicad_report "$OUT/client.log" after)
echo "INFO KiCad's dialogs after the reopen: $(python3 -c 'import json,sys; print(json.loads(sys.argv[1] or "{}").get("dialogs"))' "$RA")"
grep -a "ended (signal\|ended (code" "$OUT/hub.log" | head -2 | cut -c1-200 | sed 's/^/INFO hub log: /'
shot c4_after
finish_suite "embed_closed_unexpectedly_e2e ($EMBED_MODE)"
