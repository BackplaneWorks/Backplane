#!/bin/bash
# The hub's runtime state for the presenter (`runtime` in the session info and `viewer.runtime`: kicad/embed.rs `runtime_picture`), one
# project through each state with a real client and KiCad (fork):
#   R1  hub start: support "supported" (read without starting KiCad)
#   R2  the thread shown: the warm-up starts KiCad: session starting|live, a generation, no holder (no pane)
#   R3  Edit: live, holder set (the client's connection), the same generation
#   R4  View (detach keeping the container): no holder
#   R5  the idle sweep (BACKPLANE_KICAD_IDLE_S=15): session idleStopped, stopReason idle, stoppedAt set
#   R6  a foreign KiCad lock on the board, a warm request: warmBlock externalLock with its words and warmAt; the session stays as it was
#   R7  the lock gone, warm again: started, a new generation, warmBlock none
#   R8  an agent's editor.kicad.close (a connection with no view): session cold, stopReason agentClosed
#   R9  warmed again and KiCad killed: session failed, stopReason crashed
# Copies only (seed_project.py), fork mode.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_runtime_state_e2e: fork only"; exit 0; }
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PROJ="$OUT/proj"; PRO=$(find "$PROJ" -maxdepth 1 -name "*.kicad_pro" | head -1); PCB="${PRO%.kicad_pro}.kicad_pcb"
# the warm-up's headroom off (a shared box), the idle close at 15 s
export BACKPLANE_KICAD_WARM_HEADROOM_MB=0 BACKPLANE_KICAD_IDLE_S=15
start_xvfb_and_wm; start_hub
for _ in $(seq 60); do hub_rpc "[]" >/dev/null 2>&1 && break; sleep 0.5; done

# the project's session info (its runtime picture, state, pid), from a connection of its own
info() { hub_rpc "[]" 2>/dev/null | grep '^INFO ' | grep '"key":"editor.kicad.' | tail -1 | cut -c6- || true; }
rt() { info | python3 -c 'import json,sys
try: v=json.load(sys.stdin).get("value") or {}
except Exception: v={}
print(json.dumps(v.get("runtime") or {}))'; }
field() { python3 -c 'import json,sys
try: v=json.loads(sys.argv[1]) or {}
except Exception: v={}
x=v.get(sys.argv[2]) if isinstance(v, dict) else None
print("" if x is None else x)' "$1" "$2" 2>/dev/null || true; }
until_rt() { # until_rt KEY VALUE [tries]: the picture once KEY reads VALUE (or the last read)
  local r=""; for _ in $(seq "${3:-120}"); do r=$(rt || true); [ "$(field "$r" "$1")" = "$2" ] && break; sleep 0.5; done; [ -n "$r" ] || r="{}"; echo "$r"; return 0; }

# R1: support read at hub start, before any KiCad ran (its log line comes before the first "warming")
for _ in $(seq 40); do grep -q "kicad embed: runtime support:" "$OUT/hub.log" && break; sleep 0.25; done
SUP=$(grep -o "kicad embed: runtime support: [a-z]*" "$OUT/hub.log" | head -1 | awk '{print $NF}' || true)
BEFORE=$(awk '/kicad embed: runtime support:/{print "yes"; exit} /kicad embed: warming/{print "no"; exit}' "$OUT/hub.log")
echo "INFO hub start: support $SUP (before any warm-up: ${BEFORE:-?})"

# the client: the thread shown (its warm-up), Board, Edit, View; then left open (its leaving would close KiCad: appClosed)
python3 - "$OUT/script.json" <<'PY'
import json, sys
s = [{"select": 0}, {"wait": 6000}, {"mark": "shown"}, {"wait": 3000}, {"click": "rail:Board"}, {"wait": 2500}, {"click": "ui:Edit"},
     {"kicad": {"wait_painted": 60000, "kind": "PCB", "tag": "edit"}}, {"wait": 3000}, {"mark": "edited"}, {"wait": 4000},
     {"click": "ui:View"}, {"wait": 2000}, {"mark": "viewed"}, {"wait": 170000}]
json.dump(s, open(sys.argv[1], "w"))
PY
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 200 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
mark() { for _ in $(seq 900); do grep -q "\"name\":\"$1\",\"event\":\"mark\"" "$OUT/client.log" 2>/dev/null && return 0; kill -0 "$CP" 2>/dev/null || return 1; sleep 0.1; done; return 1; }

mark shown || true
R=$(until_rt session live 60); echo "INFO R2 $R"
G1=$(field "$R" generation)
check "R1 support is supported at hub start, before any KiCad, and in the picture" "$([ "$SUP" = supported ] && [ "$BEFORE" = yes ] && [ "$(field "$R" support)" = supported ] && echo 1 || echo 0)" "start $SUP before $BEFORE; $R"
check "R2 the warm-up: live, a generation, no holder" "$([ "$(field "$R" session)" = live ] && [ -n "$G1" ] && [ -z "$(field "$R" holder)" ] && echo 1 || echo 0)" "$R"

mark edited || true
R=$(rt || true); echo "INFO R3 $R"
check "R3 Edit: live, held by the client's window, the same generation" "$([ "$(field "$R" session)" = live ] && [ -n "$(field "$R" holder)" ] && [ "$(field "$R" generation)" = "$G1" ] && echo 1 || echo 0)" "$R"

mark viewed || true
R=$(rt || true); echo "INFO R4 $R"
check "R4 View: no holder" "$([ "$(field "$R" session)" = live ] && [ -z "$(field "$R" holder)" ] && echo 1 || echo 0)" "$R"

R=$(until_rt session idleStopped 90); echo "INFO R5 $R"
check "R5 the idle sweep: idleStopped, stopReason idle, stoppedAt" "$([ "$(field "$R" session)" = idleStopped ] && [ "$(field "$R" stopReason)" = idle ] && [ -n "$(field "$R" stoppedAt)" ] && echo 1 || echo 0)" "$R"

# R6: another KiCad's lock on the board
LCK="$(dirname "$PCB")/~$(basename "$PCB").lck"; echo '{"hostname":"elsewhere","username":"someone"}' >"$LCK"
hub_rpc "[{\"method\":\"editor.kicad.warm\",\"timeout_ms\":30000,\"params\":{\"project\":\"$PRO\",\"display\":\"${DISPLAY:-}\"}}]" >"$OUT/warm6.out" 2>&1 || true
R=$(until_rt warmBlock externalLock 20); echo "INFO R6 $R"
check "R6 a foreign lock: warmBlock externalLock, its words, warmAt; still idleStopped" "$([ "$(field "$R" warmBlock)" = externalLock ] && [ "$(field "$R" warmWhy)" = "the project is open in another KiCad" ] && [ -n "$(field "$R" warmAt)" ] && [ "$(field "$R" session)" = idleStopped ] && echo 1 || echo 0)" "$R"
rm -f "$LCK"

hub_rpc "[{\"method\":\"editor.kicad.warm\",\"timeout_ms\":60000,\"params\":{\"project\":\"$PRO\",\"display\":\"${DISPLAY:-}\"}}]" >"$OUT/warm7.out" 2>&1 || true
R=$(until_rt session live 120); echo "INFO R7 $R"
G2=$(field "$R" generation)
check "R7 warmed again: live, a new generation, warmBlock none" "$([ "$(field "$R" session)" = live ] && [ -n "$G2" ] && [ "$G2" != "$G1" ] && [ "$(field "$R" warmBlock)" = none ] && echo 1 || echo 0)" "$R"

hub_rpc "[{\"method\":\"editor.kicad.close\",\"timeout_ms\":30000,\"params\":{\"project\":\"$PRO\",\"save\":\"discard\"}}]" >"$OUT/close8.out" 2>&1 || true
R=$(until_rt stopReason agentClosed 40); echo "INFO R8 $R"
check "R8 an agent's close: cold, stopReason agentClosed" "$([ "$(field "$R" session)" = cold ] && [ "$(field "$R" stopReason)" = agentClosed ] && echo 1 || echo 0)" "$R"

hub_rpc "[{\"method\":\"editor.kicad.warm\",\"timeout_ms\":60000,\"params\":{\"project\":\"$PRO\",\"display\":\"${DISPLAY:-}\"}}]" >"$OUT/warm9.out" 2>&1 || true
R=$(until_rt session live 120)
PID=$(info | python3 -c 'import json,sys
try: print((json.load(sys.stdin).get("value") or {}).get("pid") or "")
except Exception: print("")')
[ -n "$PID" ] && [ "$PID" != 0 ] && kill -9 "$PID" 2>/dev/null || true
R=$(until_rt session failed 40); echo "INFO R9 $R (killed $PID)"
check "R9 KiCad killed: failed, stopReason crashed" "$([ "$(field "$R" session)" = failed ] && [ "$(field "$R" stopReason)" = crashed ] && echo 1 || echo 0)" "$R"

kill "$CP" 2>/dev/null || true; wait "$CP" 2>/dev/null || true
finish_suite "embed_runtime_state_e2e ($EMBED_MODE)"
