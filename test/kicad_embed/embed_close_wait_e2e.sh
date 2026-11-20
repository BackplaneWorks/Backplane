#!/bin/bash
# The hub's close waits for KiCad's own exit (CLOSE_EXIT_WAIT, 15 s) instead of killing it 800 ms after its Close answered:
#   W1  a close: the session reads stopped at once (the pane goes back to the viewer within 2 s), and KiCad is not killed
#   W2  a KiCad whose exit is slowed (SIGSTOP right after its Close answered: it cannot end by itself) is still there 3 s later (the old
#       800 ms kill would have taken it), and an Edit meanwhile waits for it ("waiting for the KiCad that answered its close") instead of
#       starting a second KiCad into its lock
#   W3  past the bound it is ended ("did not end in 15000 ms … killed"), and the waiting Edit then starts the next KiCad (another pid)
#   W4  a close with nothing slowed: KiCad ends by itself within the bound (no "answered its close" line)
# Copies only (seed_project.py), fork mode.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_close_wait_e2e: fork only"; exit 0; }
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PROJ="$OUT/proj"; PRO="$PROJ/ecc83-pp.kicad_pro"
start_xvfb_and_wm
export BACKPLANE_KICAD_DEBUG=1
start_hub
for _ in $(seq 60); do hub_rpc "[]" >/dev/null 2>&1 && break; sleep 0.5; done

cat >"$OUT/script.json" <<'JSON'
[{"select":0},{"wait":1500},{"click":"rail:Board"},{"wait":1500},{"click":"ui:Edit"},
 {"kicad":{"wait_painted":120000,"kind":"PCB","tag":"up"}},{"wait":4000},{"kicad":{"report":true,"tag":"ready"}},{"mark":"ready"},{"wait":600000}]
JSON
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" \
  --script "$OUT/script.json" --exit-after 600 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"ready","event":"mark"' 600 || { echo "FAIL the board never opened in KiCad"; exit 1; }
KPID=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1]).get("pid") or 0)' "$(last_kicad_report "$OUT/client.log" ready)")
[ "$KPID" -gt 0 ] && track_pid "$KPID"

close_now() {   # the hub's close (discard), asked again while one of KiCad's tasks (its automatic DRC) runs
  for _ in $(seq 60); do
    hub_rpc "[{\"method\":\"editor.kicad.close\",\"timeout_ms\":60000,\"params\":{\"project\":\"$PRO\",\"save\":\"discard\"}}]" >"$OUT/close.out" 2>&1 || true
    grep -q '"closed":true' "$OUT/close.out" && return 0
    grep -q 'a task is running' "$OUT/close.out" || return 1
    sleep 1
  done; return 1; }
info_lines() { grep -c '"kicad_info":"[0-9a-f]*","gone":false,"state":"stopped"' "$OUT/client.log" || true; }

# W1 + W2: close, then KiCad's exit slowed at once (it cannot end by itself while stopped)
B0=$(info_lines); H0=$(wc -l <"$OUT/hub.log")
close_now; T0=$(date +%s%N)
kill -STOP "$KPID" 2>/dev/null || true
STOPPED_IN_TIME=$(kill -0 "$KPID" 2>/dev/null && echo 1 || echo 0)
for _ in $(seq 20); do [ "$(info_lines)" -gt "$B0" ] && break; sleep 0.1; done
T1=$(date +%s%N)
check "W1 the session reads stopped at once after the close (the pane goes back)" "$([ "$(info_lines)" -gt "$B0" ] && [ $(( (T1 - T0) / 1000000 )) -le 2000 ] && echo 1 || echo 0)" "$(( (T1 - T0) / 1000000 )) ms"
check "W2 (precondition) KiCad was still there to slow: stopped right after its Close answered" "$STOPPED_IN_TIME" "pid $KPID"
sleep 3
check "W2 3 s later KiCad is still there (not killed 800 ms after its close)" "$(kill -0 "$KPID" 2>/dev/null && echo 1 || echo 0)"
# an Edit meanwhile (the hub's attach, as the pane's Edit asks): it waits for the old KiCad
hub_rpc "[{\"method\":\"editor.kicad.attach\",\"params\":{\"project\":\"$PRO\",\"frame\":\"PCB\",\"wait\":false}}]" >"$OUT/attach.out" 2>&1 || true
sleep 1
check "W2 an Edit meanwhile waits for it (no second KiCad into its lock)" "$(tail -n +"$((H0 + 1))" "$OUT/hub.log" | grep -q "waiting for the KiCad that answered its close (pid $KPID)" && echo 1 || echo 0)"
check "W2 and no other KiCad is running for the project yet" "$(pgrep -f "ecc83-pp.kicad_pro" | grep -vx "$KPID" | xargs -r ps -o comm= -p 2>/dev/null | grep -qi kicad && echo 0 || echo 1)"

# W3: past the bound it is ended; the waiting Edit then starts the next KiCad
for _ in $(seq 120); do kill -0 "$KPID" 2>/dev/null || break; sleep 0.25; done
check "W3 past the bound the slowed KiCad is ended" "$(kill -0 "$KPID" 2>/dev/null && echo 0 || echo 1)" "$(( ($(date +%s%N) - T0) / 1000000 )) ms after the close"
check "W3 the hub says how ('did not end in 15000 ms')" "$(tail -n +"$((H0 + 1))" "$OUT/hub.log" | grep -q "the KiCad that answered its close did not end in 15000 ms" && echo 1 || echo 0)"
NEXT=0
for _ in $(seq 240); do
  NEXT=$(hub_rpc "[]" | grep '^INFO ' | grep '"key":"editor.kicad.' | tail -1 | cut -c6- | python3 -c 'import json,sys
try: v=json.load(sys.stdin).get("value") or {}; print(v.get("pid") or 0 if v.get("state") in ("starting","ready") else 0)
except Exception: print(0)' 2>/dev/null || echo 0)
  [ "${NEXT:-0}" -gt 0 ] && [ "$NEXT" != "$KPID" ] && break; sleep 0.5
done
check "W3 the waiting Edit then starts the next KiCad" "$([ "${NEXT:-0}" -gt 0 ] && [ "$NEXT" != "$KPID" ] && echo 1 || echo 0)" "pid $NEXT"
[ "${NEXT:-0}" -gt 0 ] && track_pid "$NEXT"

# W4: a close with nothing slowed ends by itself within the bound
for _ in $(seq 240); do read -r ST < <(hub_rpc "[]" | grep '^INFO ' | grep '"key":"editor.kicad.' | tail -1 | cut -c6- | python3 -c 'import json,sys
try: print((json.load(sys.stdin).get("value") or {}).get("state","-"))
except Exception: print("-")' 2>/dev/null || echo -); [ "$ST" = ready ] && break; sleep 0.5; done
H1=$(wc -l <"$OUT/hub.log")
close_now; T2=$(date +%s%N)
for _ in $(seq 80); do kill -0 "$NEXT" 2>/dev/null || break; sleep 0.25; done
check "W4 a plain close: KiCad ends by itself within the bound" "$(kill -0 "$NEXT" 2>/dev/null && echo 0 || echo 1)" "$(( ($(date +%s%N) - T2) / 1000000 )) ms"
check "W4 and the hub had nothing to end (no 'answered its close' line)" "$(tail -n +"$((H1 + 1))" "$OUT/hub.log" | grep -q "answered its close" && echo 0 || echo 1)"
finish_suite "embed_close_wait_e2e ($EMBED_MODE)"
