#!/bin/bash
# A container is never destroyed with KiCad's holder in it (the fork embed thread under gdb, Nov 16: the X server destroyed KiCad's own
# windows with it, and GDK's next request on one got BadWindow: KiCad _exit(1) on bp.13, SIGSEGV on bp.12.4). Every way the editor
# leaves the pane or the window goes, at every moment; KiCad alive 20 s after, the same process (its unsaved edit with it), and the next
# Edit adopts and paints it:
#   CX_EXIT=close     the window's close (_NET_CLOSE_WINDOW, as its title bar's X)
#   CX_EXIT=kill      the window killed (kill -9: the X server's save-set moves the holder out)
#   CX_EXIT=view      View (the container kept, unmapped, KiCad in it), then the window closed
#   CX_EXIT=detach    View the other way (BACKPLANE_GPUI_KICAD_NO_KEEP: the container destroyed, as for another window), then closed
#   CX_EXIT=viewer    the viewer closed (its X: the pane goes), then the window closed
#   CX_AT=pcb | sch | switch (just after the Schematic tab press, a warm switch in flight) | pending (just after the first, cold one)
# CX_RUNS (10) runs on one hub and one KiCad (re-adopted by each new window); pending: a new hub each run (a cold schematic each time).
# The unsaved edit: an agent's track (CreateItems, not saved) in that KiCad: the same KiCad alive is the edit kept (bp.13 sets no
# modified flag for an API commit: said, not judged).
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
EXITHOW=${CX_EXIT:-close}; AT=${CX_AT:-sch}; N=${CX_RUNS:-10}
if [ "$AT" = pending ] && [ -z "${CX_ONE:-}" ]; then
  ok=0; for i in $(seq "$N"); do if CX_ONE=$i CX_RUNS=1 OUT= bash "$0" "$@"; then ok=$((ok + 1)); fi; done
  echo "SUMMARY client $EXITHOW at $AT: KiCad survived and came back in $ok of $N runs"; [ "$ok" = "$N" ]; exit $?
fi
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_client_exit_e2e: fork only"; exit 0; }
[ "${1:-}" != "" ] && CLIENT_BIN=$1
[ "${2:-}" != "" ] && HUB_BIN=$2
discover_bins; [ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT" >/dev/null 2>&1
PRO=$(find "$OUT/proj" -maxdepth 3 -name '*.kicad_pro' -not -path '*/.backplane/*' | sort | head -1); PCB=${PRO%.kicad_pro}.kicad_pcb
start_xvfb_and_wm; start_hub
echo "INFO hub on $HPORT, project $PRO"
kpid() { pgrep -u "$(id -u)" -f -- "--embedded.*--parent-pid.*$(basename "$PRO")" 2>/dev/null | tail -1; }
ask() { local want=$2 before n; before=$(grep -c "^MCPCALL" "$OUT/standin.log" 2>/dev/null); before=${before:-0}
  local text; text=$(python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$1")
  hub_rpc "[{\"method\":\"turn.start\",\"params\":{\"thread\":\"t1\",\"text\":$text,\"msg\":\"mcx$(date +%s%N)\"}}]" >>"$OUT/rpc.log" 2>&1
  for _ in $(seq 360); do n=$(grep -c "^MCPCALL" "$OUT/standin.log" 2>/dev/null); [ "${n:-0}" -ge $((before + want)) ] && break; sleep 0.25; done; sleep 1; }
script() { # $1 file
  python3 - "$1" "$AT" "$EXITHOW" <<'PY'
import json, sys
at, how = sys.argv[2], sys.argv[3]
s = [{"select": 0}, {"wait": 2200}, {"click": "rail:Board"}, {"wait": 2500}, {"click": "ui:Edit"}, {"kicad": {"wait_painted": 90000, "kind": "PCB", "tag": "ready"}}, {"mark": "ready"}, {"wait": 6000}]
if at == "sch": s += [{"click": "kicad-tab:SCH"}, {"kicad": {"wait_painted": 90000, "kind": "SCH", "tag": "sch"}}, {"wait": 1500}]
elif at in ("switch", "pending"): s += [{"click": "kicad-tab:SCH"}]
if how in ("view", "detach"): s += [{"click": "ui:View"}, {"wait": 1000}]
elif how == "viewer": s += [{"click": "ui:CloseViewer"}, {"wait": 1000}]
s += [{"mark": "go"}, {"wait": 600000}]
json.dump(s, open(sys.argv[1], "w"))
PY
}
close_window() { local wid; wid=$(xwininfo -root -tree 2>/dev/null | grep '"Backplane GPUI' | head -1 | awk '{print $1}')
  [ -n "$wid" ] && python3 "$KEMBED_LIB/wm_ctl.py" close "$wid" >/dev/null 2>&1
  for _ in $(seq 40); do kill -0 "$CP" 2>/dev/null || return 0; sleep 0.25; done; echo "INFO the window did not close: killed"; kill -9 "$CP" 2>/dev/null; }
OK=0; PID0=""; EDITED=0; CAME=0
for i in $(seq "$N"); do
  script "$OUT/s$i.json"
  NOKEEP=(); [ "$EXITHOW" = detach ] && NOKEEP=(BACKPLANE_GPUI_KICAD_NO_KEEP=1)
  env ${NOKEEP[@]+"${NOKEEP[@]}"} HOME="$OUT/fh" BACKPLANE_KICAD_DEBUG=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/s$i.json" --exit-after 500 >"$OUT/c$i.log" 2>&1 &
  CP=$!; track_pid "$CP"
  if ! wait_for_log "$OUT/c$i.log" '"name":"ready","event":"mark"' 900; then echo "INFO run $i: the editor did not come"; break; fi
  [ "$i" -gt 1 ] && CAME=$((CAME + 1))
  [ -z "$PID0" ] && PID0=$(kpid)
  if [ "$EDITED" = 0 ]; then # the unsaved edit: an agent's track, not saved
    TRACK="{\"@type\":\"type.googleapis.com/kiapi.board.types.Track\",\"start\":{\"x_nm\":\"60000000\",\"y_nm\":\"60000000\"},\"end\":{\"x_nm\":\"70000000\",\"y_nm\":\"60000000\"},\"width\":{\"value_nm\":\"300000\"},\"layer\":\"BL_F_Cu\"}"
    DOC="{\"type\":\"DOCTYPE_PCB\",\"board_filename\":\"$(basename "$PCB")\"}"
    ask "mcpcall:{\"name\":\"kicad_ipc\",\"arguments\":{\"path\":\"$PCB\",\"type\":\"kiapi.common.commands.CreateItems\",\"message\":{\"header\":{\"document\":$DOC},\"items\":[$TRACK]}}}" 1
    LOCK=$(grep "^MCPCALL kicad_ipc" "$OUT/standin.log" | tail -1 | grep -o 'lock_acquire kicad-[A-Za-z0-9_-]*' | head -1 | cut -d' ' -f2)
    [ -n "$LOCK" ] && ask "mcpseq:[{\"name\":\"lock_acquire\",\"arguments\":{\"name\":\"$LOCK\"}},{\"name\":\"kicad_ipc\",\"arguments\":{\"path\":\"$PCB\",\"type\":\"kiapi.common.commands.CreateItems\",\"message\":{\"header\":{\"document\":$DOC},\"items\":[$TRACK]}}}]" 2
    EDITED=1; echo "INFO the unsaved edit: $(grep '^MCPCALL kicad_ipc' "$OUT/standin.log" | tail -1 | cut -c1-140)"
  fi
  wait_for_log "$OUT/c$i.log" '"name":"go","event":"mark"' 300
  case "$EXITHOW" in kill) kill -9 "$CP" 2>/dev/null ;; *) close_window ;; esac
  sleep 20
  PID=$(kpid); ENDED=$(grep -c 'kicad embed: .* ended (' "$OUT/hub.log")
  echo "INFO run $i ($EXITHOW at $AT): KiCad $PID0 -> ${PID:-gone} 20 s after; ended $ENDED; lost parent $(grep -c 'lost its parent' "$OUT/hub.log"); holder left in a container $(grep -c kicad_holder_left_in "$OUT/c$i.log")"
  if [ -n "$PID" ] && [ "$PID" = "$PID0" ] && [ "${ENDED:-0}" = 0 ]; then OK=$((OK + 1)); else grep -o 'kicad embed: .* ended (.*' "$OUT/hub.log" | head -2 | cut -c1-200 | sed 's/^/INFO   /'; break; fi
done
# the re-attach after the last run
if [ "$OK" = "$N" ]; then
  script "$OUT/sz.json"; HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/sz.json" --exit-after 200 >"$OUT/cz.log" 2>&1 &
  CP=$!; track_pid "$CP"; wait_for_log "$OUT/cz.log" '"name":"ready","event":"mark"' 900 && CAME=$((CAME + 1)); kill -9 "$CP" 2>/dev/null
fi
[ "$AT" = pending ] || echo "SUMMARY client $EXITHOW at $AT: KiCad survived in $OK of $N runs, re-attach painted $CAME of $N"
check "client $EXITHOW at $AT: KiCad alive 20 s after, the same process, $N of $N" "$([ "$OK" = "$N" ] && echo 1 || echo 0)" "$OK of $N"
check "client $EXITHOW at $AT: the next Edit adopts and paints it, $N of $N" "$([ "$CAME" = "$N" ] && echo 1 || echo 0)" "$CAME of $N"
finish_suite "embed_client_exit_e2e ($EMBED_MODE, $EXITHOW, $AT)"
