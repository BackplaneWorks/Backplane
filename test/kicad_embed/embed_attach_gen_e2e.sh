#!/bin/bash
# The default flip's PR B: what an automatic attach (Attach::Automatic, `{"kicad":{"attach":..,"why":"auto"}}`) leaves alone.
#   G1  a burst of automatic attaches for the design on show is one attach (the hub hears one editor.kicad.attach)
#   G2  an attach for a KiCad session that is not the one running (`generation` another than the runtime's) is answered stale and
#       changes nothing; the running one's generation is not
#   G3  KiCad quitting (the person's Quit: exit 0, no close asked; the runtime's closedByPerson) is its own event in the pane
#       (`kicad_closed_by_person`): the viewer comes back
#   G4  after it, an automatic attach of that project is refused ("closed by the person"); the person's own Edit opens it again
#   G5  KiCad killed (a crash) is not the person's close: no `kicad_closed_by_person`
# Copies only (seed_project.py), fork mode.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_attach_gen_e2e: fork only"; exit 0; }
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PROJ="$OUT/proj"; PRO="$PROJ/ecc83-pp.kicad_pro"
start_xvfb_and_wm
export BACKPLANE_KICAD_DEBUG=1
start_hub

cat >"$OUT/script.json" <<'JSON'
[{"select":0},{"press":"rail:Board"},{"kicad":{"attach":"board","why":"auto","tag":"a1"}},
 {"kicad":{"wait_painted":90000,"kind":"PCB","tag":"up"}},{"mark":"up"},
 {"kicad":{"attach":"board","why":"auto"}},{"kicad":{"attach":"board","why":"auto"}},{"kicad":{"attach":"board","why":"auto"}},{"kicad":{"attach":"board","why":"auto"}},
 {"wait":2500},{"mark":"burst"},
 {"wait":20000},{"mark":"after_quit"},
 {"kicad":{"attach":"board","why":"auto","tag":"refused"}},{"wait":2500},{"mark":"auto_after_quit"},
 {"kicad":{"attach":"board","why":"edit","tag":"edit"}},{"wait":3000},{"kicad":{"wait_painted":90000,"kind":"PCB","tag":"reopened"}},{"mark":"reopened"},
 {"wait":600000}]
JSON
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 900 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"

mark_line() { grep -n "\"name\":\"$1\",\"event\":\"mark\"" "$OUT/client.log" | head -1 | cut -d: -f1; }
# (wait_for_log's third argument is tries at 0.25 s)
# lines of $3 in the client log between marks $1 and $2 ("" = the end)
count_between() { local a b; a=$(mark_line "$1"); b=$([ -n "$2" ] && mark_line "$2" || wc -l <"$OUT/client.log"); [ -n "$a" ] && [ -n "$b" ] || { echo 0; return; }
  sed -n "${a},${b}p" "$OUT/client.log" | grep -c -- "$3" || true; }
# the runtime picture of the project as the hub publishes it: "generation session"
runtime_now() { hub_rpc "[]" | grep '^INFO ' | grep '"key":"editor.kicad.' | tail -1 | cut -c6- | python3 -c 'import json,sys
try: r=(json.load(sys.stdin).get("value") or {}).get("runtime") or {}; print(r.get("generation", 0), r.get("session", "-"))
except Exception: print(0, "-")' 2>/dev/null || echo "0 -"; }

wait_for_log "$OUT/client.log" '"name":"up","event":"mark"' 240 || { echo "FAIL the board never opened in KiCad"; exit 1; }
R=$(last_kicad_report "$OUT/client.log" up)
read -r KPID SOCK < <(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); print(r.get("pid") or 0, r.get("sock") or "")' "$R")
[ "$KPID" -gt 0 ] && track_pid "$KPID"

# G1: the burst
wait_for_log "$OUT/client.log" '"name":"burst","event":"mark"' 240 || true
ATT=$(grep -c '"kicad_rpc_sent":"editor.kicad.attach"' "$OUT/client.log" || true)
check "G1 five automatic attaches of the design on show are one attach" "$([ "$ATT" = 1 ] && echo 1 || echo 0)" "$ATT editor.kicad.attach sent"
check "G1 and the first one said auto (no person's touch)" "$(grep -q '"kicad_attach":"board","why":"Automatic"' "$OUT/client.log" && echo 1 || echo 0)"

# G2: a late attach for another generation is stale and changes nothing; the running one's is not
read -r GEN SESSION < <(runtime_now)
check "G2 the runtime names the running session (generation, live)" "$([ "$GEN" -gt 0 ] && [ "$SESSION" = live ] && echo 1 || echo 0)" "generation $GEN, session $SESSION"
hub_rpc "[{\"method\":\"editor.kicad.attach\",\"params\":{\"project\":\"$PRO\",\"frame\":\"PCB\",\"wait\":false,\"auto\":true,\"generation\":$((GEN + 7))}}]" >"$OUT/stale.out" 2>&1 || true
check "G2 an attach for generation $((GEN + 7)) is answered stale" "$(grep -q '"stale":true' "$OUT/stale.out" && echo 1 || echo 0)" "$(grep -o '"stale":[a-z]*[^}]*' "$OUT/stale.out" | head -1)"
hub_rpc "[{\"method\":\"editor.kicad.show\",\"params\":{\"project\":\"$PRO\",\"frame\":\"PCB\",\"generation\":$GEN}}]" >"$OUT/current.out" 2>&1 || true
check "G2 the running generation's show is not stale" "$(grep -q '"stale":true' "$OUT/current.out" && echo 0 || echo 1)"
read -r GEN2 _ < <(runtime_now)
check "G2 and the stale attach changed nothing (same KiCad, same generation)" "$([ "$GEN2" = "$GEN" ] && kill -0 "$KPID" 2>/dev/null && echo 1 || echo 0)" "generation $GEN2, pid $KPID"

# G3: KiCad quits by itself (Close discard on its own socket: exit 0 with no close asked through the hub, as the person's Quit)
python3 "$IPC_PY" close "$SOCK" discard >"$OUT/quit.out" 2>&1 || true
for _ in $(seq 60); do kill -0 "$KPID" 2>/dev/null || break; sleep 0.25; done
for _ in $(seq 40); do read -r _ SESSION < <(runtime_now); [ "$SESSION" = closedByPerson ] && break; sleep 0.25; done
check "G3 KiCad's own quit is the person's close in the runtime (closedByPerson)" "$([ "$SESSION" = closedByPerson ] && echo 1 || echo 0)" "session $SESSION; $(cat "$OUT/quit.out")"
wait_for_log "$OUT/client.log" '"kicad_closed_by_person"' 10 || true
check "G3 the pane says so as its own event (kicad_closed_by_person)" "$(grep -q '"kicad_closed_by_person"' "$OUT/client.log" && echo 1 || echo 0)"

# G4: an automatic attach is refused now; the person's Edit opens it again
wait_for_log "$OUT/client.log" '"name":"auto_after_quit","event":"mark"' 240 || true
check "G4 an automatic attach after the person's close is refused" "$([ "$(count_between after_quit auto_after_quit '"kicad_attach":"refused","why":"closed by the person"')" -ge 1 ] && echo 1 || echo 0)"
check "G4 and sends no attach" "$([ "$(count_between after_quit auto_after_quit '"kicad_rpc_sent":"editor.kicad.attach"')" = 0 ] && echo 1 || echo 0)"
wait_for_log "$OUT/client.log" '"name":"reopened","event":"mark"' 720 || true
R=$(last_kicad_report "$OUT/client.log" reopened)
read -r KPID2 SOCK2 STATE2 < <(python3 -c 'import json,sys; r=json.loads(sys.argv[1] or "{}"); print(r.get("pid") or 0, r.get("sock") or "-", r.get("state") or "-")' "$R")
check "G4 the person's Edit opens it again (a new KiCad, ready)" "$([ "$KPID2" -gt 0 ] && [ "$KPID2" != "$KPID" ] && [ "$STATE2" = ready ] && echo 1 || echo 0)" "pid $KPID2, state $STATE2"
[ "$KPID2" -gt 0 ] && track_pid "$KPID2"

# G5: a crash is not the person's close
BEFORE=$(grep -c '"kicad_closed_by_person"' "$OUT/client.log" || true)
[ "${KPID2:-0}" -gt 0 ] && { kill -9 "$KPID2" 2>/dev/null || true; }   # (never pid 0: that is the whole process group)
sleep 6
read -r _ SESSION < <(runtime_now)
AFTER=$(grep -c '"kicad_closed_by_person"' "$OUT/client.log" || true)
check "G5 KiCad killed is not the person's close (no kicad_closed_by_person; runtime $SESSION)" "$([ "$AFTER" = "$BEFORE" ] && [ "$SESSION" != closedByPerson ] && echo 1 || echo 0)" "$BEFORE -> $AFTER"
finish_suite "embed_attach_gen_e2e ($EMBED_MODE)"
