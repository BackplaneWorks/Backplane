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
# a fresh home's first run asks for the global library tables (a modal dialog that held every close: all of the first loop's cycles):
# empty tables in the config the editor reads, as the macOS check gives it
for v in "" 10.0 10.99; do c="${XDG_CONFIG_HOME:-$OUT/fh/.config}/kicad/$v"; mkdir -p "$c"
  printf '(sym_lib_table\n  (version 7)\n)\n' >"$c/sym-lib-table"; printf '(fp_lib_table\n  (version 7)\n)\n' >"$c/fp-lib-table"
  printf '(design_block_lib_table\n  (version 7)\n)\n' >"$c/design-block-lib-table"; done
# the copy upgraded to the current file format first: an older board opens modified ("converted to the new format when saved"), so
# every close asked "Save Changes?" and waited (the second loop: all pcbnew cycles, base and fix alike)
CLI="$(dirname "$KICAD_BIN")/kicad-cli"
HOME="$OUT/fh" "$CLI" pcb upgrade "$PROJ/ecc83-pp.kicad_pcb" >"$OUT/upgrade.log" 2>&1 || echo "INFO pcb upgrade: $(tail -1 "$OUT/upgrade.log")"
HOME="$OUT/fh" "$CLI" sch upgrade "$PROJ/ecc83-pp.kicad_sch" >>"$OUT/upgrade.log" 2>&1 || echo "INFO sch upgrade: $(tail -1 "$OUT/upgrade.log")"
start_xvfb_and_wm
CLEAN=0; CRASHED=0; HUNG=0; OTHER=0
for i in $(seq "$CYCLES"); do
  HOME="$OUT/fh" "$BIN" "$DOC" >"$OUT/sa-$i.log" 2>&1 &
  P=$!
  W=$(wait_for_kicad_win "$P" "$TITLE" 240 || true)
  if [ -z "$W" ]; then echo "cycle $i: no window"; OTHER=$((OTHER + 1)); kill -9 "$P" 2>/dev/null; wait "$P" 2>/dev/null; continue; fi
  sleep 4
  python3 "$KEMBED_LIB/wm_ctl.py" close "$W" >/dev/null 2>&1
  # the PCB editor opens this board modified even after the upgrade (its "Save Changes?" held every close): answered Save, its default
  # button, as the person's Return would (the copy is ours); the cycle then measures the save-and-close exit
  ASKED=0
  for _ in $(seq 12); do
    D=$(xwininfo -root -tree 2>/dev/null | grep -m1 '"Save Changes?"' | awk '{print $1}')
    if [ -n "$D" ]; then python3 "$KEMBED_LIB/wm_ctl.py" activate "$D" >/dev/null 2>&1; sleep 0.4; python3 "$XINPUT_PY" key Return >/dev/null 2>&1; ASKED=1; break; fi
    kill -0 "$P" 2>/dev/null || break; sleep 0.25
  done
  for _ in $(seq 120); do kill -0 "$P" 2>/dev/null || break; sleep 0.25; done
  if kill -0 "$P" 2>/dev/null; then
    # what is on screen of its own (a dialog that held the close is not a hang of its exit)
    WINS=$(xwininfo -root -tree 2>/dev/null | grep -E '"[^"]+"' | grep -iE "kicad|pcb|schematic|library|footprint|symbol" | sed 's/^ *//' | cut -c1-80 | tr '\n' ';')
    HUNG=$((HUNG + 1)); echo "cycle $i: still running 30 s after its window was closed; on screen: $WINS"
    # CLOSE_HANG_STACK=attach: the editor (started through bin/ptracer-any, a -ptr runtime) read where it hangs, every thread's stack
    if [ "${CLOSE_HANG_STACK:-0}" = attach ]; then
      RP=$(pgrep -P "$P" | head -1); RP=${RP:-$P}
      timeout 120 gdb -p "$RP" -batch -ex "set pagination off" -ex "info threads" -ex "thread apply all bt" >"$OUT/hang-stack-$i.txt" 2>&1
      echo "INFO cycle $i: hung editor's stacks in hang-stack-$i.txt ($(grep -c '^Thread ' "$OUT/hang-stack-$i.txt") threads)"
    fi
    kill -9 "$P" 2>/dev/null; wait "$P" 2>/dev/null
  else
    wait "$P"; st=$?
    if [ "$st" = 0 ]; then CLEAN=$((CLEAN + 1)); echo "cycle $i: clean$([ "$ASKED" = 1 ] && echo " (Save Changes? answered Save)")"
    else CRASHED=$((CRASHED + 1)); echo "cycle $i: ended with status $st; last: $(grep -v '^$' "$OUT/sa-$i.log" | tail -1)"; fi
  fi
  sleep 1
done
echo "standalone close loop ($WHICH): $CYCLES cycles, $CLEAN clean, $CRASHED crashed, $HUNG hung, $OTHER other ($(basename "$(dirname "$(dirname "$KICAD_BIN")")"))"
check "standalone $WHICH: every close ended cleanly" "$([ "$CLEAN" = "$CYCLES" ] && echo 1 || echo 0)" "$CLEAN of $CYCLES"
finish_suite "standalone_close_loop ($WHICH)"
