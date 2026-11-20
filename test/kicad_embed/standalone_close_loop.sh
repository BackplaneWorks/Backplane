#!/bin/bash
# The standalone board or schematic editor (single_top: STANDALONE=pcbnew|eeschema), not embedded: CYCLES times started on ecc83-pp's
# board or schematic and closed as the person would (the window manager's close, _NET_CLOSE_WINDOW, nothing modified), its exit status
# recorded: the close-teardown fix (the settings manager destroyed in OnPgmExit) changes single_top's exit too.
#   STANDALONE=pcbnew|eeschema  (pcbnew)   CYCLES=N (10)
# Prints a line per cycle and `standalone close loop: N cycles, C clean, X crashed, H hung`. Never on the desktop as a load: h@backplane.
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
discover_kicad
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PROJ="$OUT/proj"; WHICH=${STANDALONE:-pcbnew}; CYCLES=${CYCLES:-10}
DOC="$PROJ/ecc83-pp.$([ "$WHICH" = eeschema ] && echo kicad_sch || echo kicad_pcb)"
TITLE=$([ "$WHICH" = eeschema ] && echo "Schematic Editor" || echo "PCB Editor")
BIN="$(dirname "$KICAD_BIN")/$WHICH"
start_xvfb_and_wm
CLEAN=0; CRASHED=0; HUNG=0; OTHER=0
for i in $(seq "$CYCLES"); do
  HOME="$OUT/home" "$BIN" "$DOC" >"$OUT/sa-$i.log" 2>&1 &
  P=$!
  W=$(wait_for_kicad_win "$P" "$TITLE" 240 || true)
  if [ -z "$W" ]; then echo "cycle $i: no window"; OTHER=$((OTHER + 1)); kill -9 "$P" 2>/dev/null; wait "$P" 2>/dev/null; continue; fi
  sleep 4
  python3 "$KEMBED_LIB/wm_ctl.py" close "$W" >/dev/null 2>&1
  for _ in $(seq 120); do kill -0 "$P" 2>/dev/null || break; sleep 0.25; done
  if kill -0 "$P" 2>/dev/null; then
    HUNG=$((HUNG + 1)); echo "cycle $i: still running 30 s after its window was closed"; kill -9 "$P" 2>/dev/null; wait "$P" 2>/dev/null
  else
    wait "$P"; st=$?
    if [ "$st" = 0 ]; then CLEAN=$((CLEAN + 1)); echo "cycle $i: clean"
    else CRASHED=$((CRASHED + 1)); echo "cycle $i: ended with status $st; last: $(grep -v '^$' "$OUT/sa-$i.log" | tail -1)"; fi
  fi
  sleep 1
done
echo "standalone close loop ($WHICH): $CYCLES cycles, $CLEAN clean, $CRASHED crashed, $HUNG hung, $OTHER other ($(basename "$(dirname "$(dirname "$KICAD_BIN")")"))"
check "standalone $WHICH: every close ended cleanly" "$([ "$CLEAN" = "$CYCLES" ] && echo 1 || echo 0)" "$CLEAN of $CYCLES"
finish_suite "standalone_close_loop ($WHICH)"
