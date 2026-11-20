#!/bin/bash
# #1344: after a start that waited on another program's lock (the viewer's kicad-cli), the board must be on show in the pane, not a blank
# pane until a tab press. Stand-in lock holders (lib/lock_holder.py: KiCad's ~<file>.lck with an exclusive flock, as kicad-cli takes it)
# make the timing deterministic. One case per run, FS_CASE:
#   pro_pcb:N:D    the .kicad_pro and .kicad_pcb held when Edit is pressed; the .kicad_pro let go N s after the press, the .kicad_pcb N+D s
#   retake:R:H     both held at the press, let go 4 s after it; then the .kicad_pcb taken again R s after KiCad was spawned, for H s (the
#                  viewer's next kicad-cli run, after the hub's lock check has passed)
#   ready_hold     the .kicad_pcb taken when KiCad is spawned and let go 0.3 s after its API answers (held over the start's first show)
#   none           no lock (KEMBED_PROJ_SRC a big board: a first show slower than its retries)
# Reports the hub's `first show of <frame>:` line (ok, or FAILED with why: held elsewhere (3a), refused by KiCad (3b), timed out (3c)) and
# checks the board is on show in the pane 10 s after it. Fork mode.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_first_show_e2e: fork only"; exit 0; }
CASE=${FS_CASE:-pro_pcb:3:0}
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PRO=$(find "$OUT/proj" -maxdepth 3 -name '*.kicad_pro' -not -path '*/.backplane/*' | sort | head -1)
PCB=${PRO%.kicad_pro}.kicad_pcb
[ -f "$PCB" ] || { echo "FAIL no board beside $PRO"; exit 1; }
start_xvfb_and_wm
export BACKPLANE_KICAD_DEBUG=1
start_hub
PRESS="$OUT/edit_pressed"; rm -f "$PRESS"
H=(python3 "$HERE/lib/lock_holder.py" "$OUT/hub.log" "$PRESS")
case "$CASE" in
  pro_pcb:*) IFS=: read -r _ N D <<<"$CASE"; R2=$(python3 -c "print($N+$D)")
    "${H[@]}" "$PRO" now "press+$N" "$PCB" now "press+$R2" >"$OUT/holder.log" 2>&1 & ;;
  retake:*) IFS=: read -r _ R HH <<<"$CASE"; E=$(python3 -c "print($R+$HH)")
    "${H[@]}" "$PRO" now press+4 "$PCB" now press+4 "$PCB" "phase:spawned+$R" "phase:spawned+$E" >"$OUT/holder.log" 2>&1 & ;;
  ready_hold) "${H[@]}" "$PCB" phase:spawned+0 phase:answering+0.3 >"$OUT/holder.log" 2>&1 & ;;
  none) : ;;
  *) echo "FAIL unknown FS_CASE $CASE"; exit 1 ;;
esac
[ "$CASE" = none ] || track_pid $!
sleep 1

cat >"$OUT/script.json" <<'JSON'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},{"mark":"edit_pressed"},{"wait":240000},{"mark":"done"}]
JSON
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_HOLD=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 300 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"edit_pressed","event":"mark"' 120 || { echo "FAIL Edit was never pressed"; exit 1; }
touch "$PRESS"; T_PRESS=$(date +%s%3N)
echo "INFO case $CASE: Edit pressed at $T_PRESS"
FIRST=""
for _ in $(seq 480); do FIRST=$(grep -a "first show of" "$OUT/hub.log" | head -1 || true); [ -n "$FIRST" ] && break; sleep 0.25; done
echo "INFO first show: ${FIRST:-none within 120 s}"
CLASS=none
case "$FIRST" in
  *"FAILED"*"open in another KiCad"*) CLASS="3a held elsewhere" ;;
  *"FAILED"*"KiCad answered"*) CLASS="3b refused by KiCad" ;;
  *"FAILED"*) CLASS="3c/other: ${FIRST##*FAILED}" ;;
  *": ok after"*|*"shown after"*) CLASS=ok ;;
esac
sleep 10
# the board editor's window on screen now (any KiCad window titled PCB Editor, viewable)
SHOWN=$(python3 "$DRIVER_PY" list-windows 2>/dev/null | python3 -c 'import json,sys
t=[w.get("title") or "" for w in json.load(sys.stdin) if w.get("shown") and w.get("w",0)>=200]
print("PCB" if any("PCB Editor" in x for x in t) else "-")' 2>/dev/null || echo "?")
grep -a "ShowFrame .* refused\|lock\b\|is held by\|let go\|start +" "$OUT/hub.log" | sed 's/^/INFO hub: /' | head -40 || true
grep -a "HOLDER" "$OUT/holder.log" 2>/dev/null | sed 's/^/INFO /' || true
echo "RESULT case=$CASE first_show=[$CLASS] board_on_show_10s_after=$SHOWN"
check "case $CASE: the board is on show 10 s after the start's first show ($SHOWN; first show: $CLASS)" "$([ "$SHOWN" = PCB ] && echo 1 || echo 0)"
hub_rpc "[{\"method\":\"editor.kicad.close\",\"params\":{\"project\":\"$PRO\",\"save\":\"discard\"}}]" >/dev/null || true
finish_suite "embed_first_show_e2e ($CASE)"
