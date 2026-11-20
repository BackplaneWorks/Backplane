#!/bin/bash
# How often KiCad's close ends in a crash (bp.13.3 row 1m / 3x): CYCLES times (default 30), with product defaults: Edit on the board
# (ready and drawn; the schematic pre-warmed hidden after 2 s), then `editor.kicad.close {save:"discard"}`, then KiCad's exit: clean (exit 0)
# or how it ended (a signal, a code) with its last output line. One hub for the run; a client (and so a KiCad) per cycle.
#   CYCLES=N                 cycles (30)
#   CLOSE_PREWARM=0          no schematic pre-warm (the hub refuses it: BACKPLANE_KICAD_PREWARM_FREE_MB above any machine's memory)
#   CLOSE_VIA=kicad          KiCad's own Close on its socket (the person's Quit), not the hub's editor.kicad.close
#   CLOSE_VIA=quit           KiCad's Quit action (the menu's); CLOSE_VIA=frames: every frame closed, the board last
#   CLOSE_SAVE=1             (with CLOSE_VIA=kicad) the board modified, then Close{CM_SAVE}; the saved file checked whole (row 1n)
#   CLOSE_CORES=1            core dumps allowed (a backtrace of each crash)
#   CLOSE_FOREIGN_LOCK=0     no lock file on the project from another KiCad (removed before each start); 1: one left by a KiCad "elsewhere"
# Prints a line per cycle and `close loop: N cycles, C clean, X crashed (...)`; each crash's hub log lines are kept in $OUT/crashes.txt.
# Copies only (seed_project.py), fork mode. Measured on h@backplane (test/kicad_embed/remote.sh), never as a load on the desktop.
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_close_loop: fork only"; exit 0; }
CYCLES=${CYCLES:-30}
# a slow KiCad (under valgrind): CLOSE_SLOW=1 gives each step minutes, not seconds
if [ "${CLOSE_SLOW:-0}" = 1 ]; then READY_TRIES=7200; PAINT_MS=1500000; EXIT_TRIES=2400; CLIENT_S=3000; export BACKPLANE_KICAD_START_WINDOW_S=1500
else READY_TRIES=600; PAINT_MS=120000; EXIT_TRIES=240; CLIENT_S=400; fi
# CLOSE_CORES=1: core dumps allowed for KiCad (apport keeps them in /var/lib/apport/coredump), for a backtrace of a crash
[ "${CLOSE_CORES:-0}" = 1 ] && ulimit -c unlimited
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PROJ="$OUT/proj"; PRO="$PROJ/ecc83-pp.kicad_pro"
start_xvfb_and_wm
export BACKPLANE_KICAD_DEBUG=1
[ "${CLOSE_PREWARM:-1}" = 0 ] && export BACKPLANE_KICAD_PREWARM_FREE_MB=100000000
start_hub
for _ in $(seq 60); do hub_rpc "[]" >/dev/null 2>&1 && break; sleep 0.5; done

cat >"$OUT/script.json" <<'JSON'
[{"select":0},{"wait":1500},{"click":"rail:Board"},{"wait":1500},{"click":"ui:Edit"},
 {"kicad":{"wait_painted":PAINT_MS,"kind":"PCB","tag":"up"}},{"wait":4000},{"kicad":{"report":true,"tag":"ready"}},{"mark":"ready"},{"wait":600000}]
JSON
sed -i "s/PAINT_MS/$PAINT_MS/" "$OUT/script.json"
info_now() { hub_rpc "[]" | grep '^INFO ' | grep '"key":"editor.kicad.' | tail -1 | cut -c6- | python3 -c 'import json,sys
try: v=json.load(sys.stdin).get("value") or {}; print(v.get("state","-"), len(v.get("frames") or []), (v.get("runtime") or {}).get("session","-"))
except Exception: print("- 0 -")' 2>/dev/null || echo "- 0 -"; }

SAVED_OK=0; SAVED_BAD=0
CLEAN=0; CRASHED=0; OTHER=0   # OTHER: never came up, still running after the close, or killed by the hub's 800 ms; SUMMARY=()
: >"$OUT/crashes.txt"
for i in $(seq "$CYCLES"); do
  # the project's lock: removed (no other KiCad), or left by a KiCad elsewhere (another host), as the default run found it
  case "${CLOSE_FOREIGN_LOCK:-}" in
    0) rm -f "$PROJ"/~ecc83-pp.kicad_pro.lck "$PROJ"/.~lock.* "$PROJ"/*.lck 2>/dev/null ;;
    1) printf '{"username":"someone","hostname":"elsewhere"}' >"$PROJ/~ecc83-pp.kicad_pro.lck" ;;
  esac
  N0=$(wc -l <"$OUT/hub.log")
  CLOG="$OUT/client-$i.log"
  HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" \
    --script "$OUT/script.json" --exit-after "$CLIENT_S" >"$CLOG" 2>&1 &
  CP=$!; track_pid "$CP"
  if ! wait_for_log "$CLOG" '"name":"ready","event":"mark"' "$READY_TRIES"; then
    echo "cycle $i: KiCad never came up ($(info_now))"; OTHER=$((OTHER + 1)); kill "$CP" 2>/dev/null; wait "$CP" 2>/dev/null; continue
  fi
  R=$(last_kicad_report "$CLOG" ready)
  KPID=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1]).get("pid") or 0)' "$R")
  read -r STATE FRAMES _ < <(info_now)
  # CLOSE_VIA=kicad: KiCad's own Close on its socket (the person's Quit as the hub sees it, as embed_attach_gen_e2e G3), not the hub's close
  if [ "${CLOSE_VIA:-hub}" = kicad ]; then
    SOCK=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1]).get("sock") or "")' "$R")
    # CLOSE_SAVE=1 (row 1n, no data loss): the board modified first (an agent's track through KiCad's API, `modified` read true),
    # then Close{CM_SAVE}; the file is checked after the process ended, however it ended
    MODE=discard
    if [ "${CLOSE_SAVE:-0}" = 1 ]; then
      MODE=save; SEG0=$(grep -c "(segment" "$PROJ/ecc83-pp.kicad_pcb" || true)
      HIST0=$(git -C "$PROJ/.history" rev-list --all --count 2>/dev/null || echo 0)
      python3 "$IPC_PY" agent-add-track "$SOCK" 100 $((100 + i)) 104 $((100 + i)) 0.25 3 >"$OUT/track-$i.out" 2>&1 || true
      MOD=$(python3 "$IPC_PY" modified "$SOCK" pcb 2>/dev/null | python3 -c 'import json,sys; print(json.load(sys.stdin).get("modified"))' 2>/dev/null || echo "?")
    fi
    for _ in $(seq 60); do python3 "$IPC_PY" close "$SOCK" "$MODE" >"$OUT/close-$i.out" 2>&1 && grep -q '"exiting": true' "$OUT/close-$i.out" && break; sleep 1; done
  elif [ "${CLOSE_VIA:-hub}" = quit ]; then
    # the person's Quit from KiCad's menu: its action (common.Control.quit) in the board editor
    SOCK=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1]).get("sock") or "")' "$R")
    python3 "$IPC_PY" run-action "$SOCK" common.Control.quit >"$OUT/close-$i.out" 2>&1 || true
  elif [ "${CLOSE_VIA:-hub}" = frames ]; then
    # the person closing every frame, the last one last (the pre-warmed schematic, then the board): KiCad ends with its last frame
    SOCK=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1]).get("sock") or "")' "$R")
    { python3 "$IPC_PY" close "$SOCK" discard sch; python3 "$IPC_PY" close "$SOCK" discard pcb; } >"$OUT/close-$i.out" 2>&1 || true
  else
  # (KiCad refuses a close while one of its tasks runs: the editor's automatic DRC or ERC. Asked again each second, as a person waits)
  for _ in $(seq 60); do
    hub_rpc "[{\"method\":\"editor.kicad.close\",\"timeout_ms\":60000,\"params\":{\"project\":\"$PRO\",\"save\":\"discard\"}}]" >"$OUT/close-$i.out" 2>&1 || true
    grep -q '"closed":true' "$OUT/close-$i.out" && break
    grep -q 'a task is running' "$OUT/close-$i.out" || break
    sleep 1
  done
  fi
  TC=$(date +%s%N)
  for _ in $(seq "$EXIT_TRIES"); do [ "$KPID" -gt 0 ] && kill -0 "$KPID" 2>/dev/null || break; sleep 0.25; done
  EXIT_MS=$(( ($(date +%s%N) - TC) / 1000000 ))
  sleep 1.5
  ENDED=$(tail -n +"$((N0 + 1))" "$OUT/hub.log" | grep -m1 -E " ended (while closing |after its close )?\(" || true)
  if [ -n "$ENDED" ] && echo "$ENDED" | grep -q "after its close (signal 9"; then
    # the hub's own kill: KiCad still alive 800 ms after it answered Close (not an abort; counted apart)
    OTHER=$((OTHER + 1)); echo "cycle $i: killed by the hub 800 ms after its Close (a slow exit) frames=$FRAMES"
  elif [ -n "$ENDED" ]; then
    CRASHED=$((CRASHED + 1)); HOW=$(echo "$ENDED" | sed -E 's/.* ended (while closing |after its close )?\((.*)\); its last output:.*/\2/')
    LAST=$(tail -n +"$((N0 + 1))" "$OUT/hub.log" | sed -n -E '/ ended (while closing |after its close )?\(/,/^kicad embed: publish/p' | grep -v '^kicad embed' | tail -1)
    echo "cycle $i: CRASHED ($HOW) frames=$FRAMES; last: $LAST"; SUMMARY+=("$HOW: $LAST")
    { echo "== cycle $i (frames $FRAMES)"; tail -n +"$((N0 + 1))" "$OUT/hub.log" | sed -n -E '/ ended (while closing |after its close )?\(/,/^kicad embed: publish/p' | head -60; } >>"$OUT/crashes.txt"
  elif kill -0 "$KPID" 2>/dev/null; then
    echo "cycle $i: KiCad still running after the close ($(cat "$OUT/close-$i.out" | tail -c 200))"; OTHER=$((OTHER + 1))
    # CLOSE_HANG_STACK=1: a KiCad running under gdb (the -gdb runtimes) is interrupted, so gdb writes every thread's stack, then killed
    if [ "${CLOSE_HANG_STACK:-0}" = 1 ]; then kill -INT "$KPID" 2>/dev/null; sleep 15; fi
    kill -9 "$KPID" 2>/dev/null
  else
    CLEAN=$((CLEAN + 1)); echo "cycle $i: clean (frames=$FRAMES, exited ${EXIT_MS} ms after the close answered)"
  fi
  if [ "${CLOSE_SAVE:-0}" = 1 ]; then
    # the saved board: complete and loadable (kicad-cli reads it and exports), the track there, no temp file as its only copy
    PCB="$PROJ/ecc83-pp.kicad_pcb"; SIZE=$(stat -c %s "$PCB" 2>/dev/null || echo 0)
    SEG1=$(grep -c "(segment" "$PCB" || true)
    LOADS=$("$(dirname "$KICAD_BIN")/kicad-cli" pcb export svg --layers F.Cu -o "$OUT/save-$i.svg" "$PCB" >"$OUT/load-$i.out" 2>&1 && [ -s "$OUT/save-$i.svg" ] && echo 1 || echo 0)
    TEMPS=$(ls -a "$PROJ" | grep -E '^\.?ecc83-pp\.kicad_pcb.+' | grep -v '\.backplane\.json$' | tr '\n' ' ')
    OK=$([ "$SIZE" -gt 0 ] && [ "$LOADS" = 1 ] && [ "$SEG1" = $((SEG0 + 1)) ] && echo 1 || echo 0)
    [ "$OK" = 1 ] && SAVED_OK=$((SAVED_OK + 1)) || SAVED_BAD=$((SAVED_BAD + 1))
    HIST1=$(git -C "$PROJ/.history" rev-list --all --count 2>/dev/null || echo 0)
    echo "cycle $i save: history commits $HIST0->$HIST1 modified=$MOD size=$SIZE segments $SEG0->$SEG1 loads=$LOADS temps=[${TEMPS}] -> $([ "$OK" = 1 ] && echo intact || echo BAD)"
  fi
  kill "$CP" 2>/dev/null; wait "$CP" 2>/dev/null
  sleep 1
done
echo "close loop: $CYCLES cycles, $CLEAN clean, $CRASHED crashed, $OTHER other (close via ${CLOSE_VIA:-hub}, pre-warm ${CLOSE_PREWARM:-1}, foreign lock ${CLOSE_FOREIGN_LOCK:-as found}; $(basename "$(dirname "$(dirname "$KICAD_BIN")")"))"
printf '%s\n' "${SUMMARY[@]}" | sort | uniq -c | sed 's/^/  /'
[ "${CLOSE_SAVE:-0}" = 1 ] && { echo "save loop: $SAVED_OK intact, $SAVED_BAD bad, of $CYCLES saves (crashed after the save: $CRASHED)"; check "save loop: every saved board complete, loadable and holding its edit" "$([ "$SAVED_BAD" = 0 ] && [ "$SAVED_OK" = "$CYCLES" ] && echo 1 || echo 0)" "$SAVED_OK of $CYCLES"; }
check "close loop: no crash in $CYCLES closes" "$([ "$CRASHED" = 0 ] && echo 1 || echo 0)" "$CRASHED crashed"
finish_suite "embed_close_loop ($EMBED_MODE)"
