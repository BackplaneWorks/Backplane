#!/bin/bash
# #1470 (finding 1, Carbon's Windows r1): an Edit while the hub is closing this project's KiCad and its process has not ended is never
# attached to that process. The KiCad that never ends is made here: it is stopped (SIGSTOP) the moment the hub logs its idle close and
# never let go (Carbon's Windows r2 on main: it answered the idle Close and never exited).
#   C1  the Edit meanwhile: the hub answers "closing" ("not reused" in its log), the pane says KiCad is closing, no second KiCad starts
#       and the old pid is still the only one
#   C2  at the bound after the hub's idle Close (15 s) the hub ends it as a hang (logged with the pid and how long), and the pane
#       attaches again by itself: a fresh KiCad (another pid) on show in it
# On main the Edit attaches to the stopped process: no "not reused", no kicad_attach_closing, and the pane waits on a KiCad that is gone.
# Fork only; copies (seed_project.py).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_closing_attach_e2e: fork only"; exit 0; }
discover_bins; [ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT" >/dev/null 2>&1
PRO=$(find "$OUT/proj" -maxdepth 3 -name '*.kicad_pro' -not -path '*/.backplane/*' | sort | head -1)
export BACKPLANE_KICAD_IDLE_S=15 BACKPLANE_KICAD_WARM_HEADROOM_MB=0
start_xvfb_and_wm; start_hub
kpids() { pgrep -u "$(id -u)" -f -- "--embedded.*--parent-pid.*$PRO" 2>/dev/null | tr '\n' ' ' || true; }
python3 - "$OUT/script.json" <<'PY'
import json, sys
s = [{"select": 0}, {"wait": 2200}, {"click": "rail:Board"}, {"wait": 2500}, {"click": "ui:Edit"}, {"kicad": {"wait_painted": 90000, "kind": "PCB", "tag": "edit"}},
     {"wait": 2000}, {"click": "ui:View"}, {"mark": "viewed"},
     {"wait": 22000}, {"mark": "edit2"}, {"click": "ui:Edit"}, {"wait": 3000}, {"kicad": {"report": True, "tag": "closing"}}, {"mark": "closing"},
     {"wait": 15000}, {"kicad": {"wait_painted": 90000, "kind": "PCB", "tag": "fresh"}}, {"mark": "fresh"}, {"wait": 1000}]
json.dump(s, open(sys.argv[1], "w"))
PY
HOME="$OUT/fh" BACKPLANE_KICAD_DEBUG=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 300 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
mark() { wait_for_log "$OUT/client.log" "\"name\":\"$1\",\"event\":\"mark\"" "${2:-1800}"; }
mark viewed || echo "INFO the editor did not come"
OLD=$(kpids); OLD=${OLD%% *}; echo "INFO KiCad $OLD, viewed: waiting for the idle close"
STOPPED=0
for _ in $(seq 600); do grep -q "kicad embed: idle, closing" "$OUT/hub.log" && { [ -n "$OLD" ] && kill -STOP "$OLD" && STOPPED=1; break; }; sleep 0.05; done
echo "INFO idle close seen; KiCad $OLD stopped: $STOPPED"
mark closing || true
NOW=$(kpids); NOTREUSED=$(grep -c "is closing (idle): not reused, answered closing" "$OUT/hub.log" || true)
SAID=$(grep -c '"kicad_attach_closing"' "$OUT/client.log" || true)
R=$(last_kicad_report "$OUT/client.log" closing); SAY=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1] or "{}").get("say") or "")' "${R:-{\}}")
echo "INFO during the close: KiCads [$NOW], hub not-reused $NOTREUSED, client closing $SAID, pane says: $SAY"
check "C1 an Edit while its KiCad is closing: not attached to it (answered closing), the pane says so, no second KiCad" "$([ "$STOPPED" = 1 ] && [ "$(echo $NOW)" = "$OLD" ] && [ "${NOTREUSED:-0}" -ge 1 ] && [ "${SAID:-0}" -ge 1 ] && echo "$SAY" | grep -q "closing" && echo 1 || echo 0)" "kicads [$NOW] old $OLD; not-reused $NOTREUSED; said $SAID; '$SAY'"
mark fresh || true
NEW=$(kpids); OVER=$(grep -c '"kicad_closing_over"' "$OUT/client.log" || true)
F=$(last_kicad_report "$OUT/client.log" fresh); FPID=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1] or "{}").get("pid") or "")' "${F:-{\}}")
SHOWN=$(python3 -c 'import json,sys; r=json.loads(sys.argv[1] or "{}"); print(int(bool(r.get("shown"))))' "${F:-{\}}")
echo "INFO after: KiCads [$NEW], the pane's pid $FPID, shown $SHOWN, closing over $OVER"
HANG=$(grep -o "a hang after its idle close: pid $OLD still running [0-9]* ms after the hub's Close, killed" "$OUT/hub.log" | head -1 || true)
echo "INFO hub: ${HANG:-no hang logged}"
check "C2 the KiCad that never ends: ended at the bound as a hang (logged), and the pane's Edit gets a fresh KiCad on show" "$([ -n "$HANG" ] && [ -n "$FPID" ] && [ "$FPID" != "$OLD" ] && [ "$SHOWN" = 1 ] && [ "${OVER:-0}" -ge 1 ] && ! kill -0 "$OLD" 2>/dev/null && echo 1 || echo 0)" "old $OLD new $FPID shown $SHOWN over $OVER; $HANG"
grep -E "kicad embed: .*(idle, closing|not reused|a hang|not killed|ended|exited)" "$OUT/hub.log" | cut -c1-200 | sed 's/^/INFO   /' || true
kill -CONT "$OLD" 2>/dev/null || true
wait "$CP" 2>/dev/null || true
finish_suite "embed_closing_attach_e2e ($EMBED_MODE)"
