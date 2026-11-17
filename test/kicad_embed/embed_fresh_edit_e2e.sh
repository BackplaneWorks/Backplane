#!/bin/bash
# The first Edit on a fresh home: no Backplane home, no KiCad settings (~/.config/kicad), no KiCad data (~/.local/share/kicad), no
# caches; Board, then Edit. KiCad's board editor is painted in the pane within FRESH_EDIT_S (30), FRESH_RUNS times in a row (10), each
# run a new home. The release check saw "Starting KiCad…" for over 180 s on Linux (bp.12.4, Xvfb, Nov 16: KiCad's socket up at 25 s, no
# ready), and 23 s on the same home again. At a stall (every 15 s past 15 s) the run keeps what tells why: the X tree (any KiCad
# top-level or dialog, mapped or not), a screenshot, the hub's last KiCad lines, KiCad's process and its threads' state.
# FRESH_AGENT=first|during: an agent's kicad_ipc on the project before the client starts (the hub's headless KiCad up first, holding
# the project's lock: the release check's order, its stall's cause) or while the editor's KiCad is starting: the editor still paints in
# time, with no KiCad dialog anywhere, one process holding the project's lock, and the agent's call answered.
# FRESH_SLOW_S (12 with FRESH_AGENT=during, else 0): KiCad's launcher waits that long first (a hard-linked copy of the runtime in this
# run's folder), as the release check's cold first start took 25 s: the window in which a headless KiCad took the project on main.
# fork only; under i3 and under mutter (KEMBED_WM); software rendering under Xvfb, the GPU with REAL_HARDWARE=1.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
if [ -z "${FRESH_RUN:-}" ]; then
  rc=0; ok=0; n=${FRESH_RUNS:-10}
  for i in $(seq "$n"); do if FRESH_RUN=$i OUT= bash "$0" "$@"; then ok=$((ok + 1)); else rc=1; fi; done
  echo "SUMMARY fresh-home first Edit: $ok of $n painted within ${FRESH_EDIT_S:-30} s"
  exit $rc
fi
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_fresh_edit_e2e: fork only"; exit 0; }
[ "${1:-}" != "" ] && CLIENT_BIN=$1
[ "${2:-}" != "" ] && HUB_BIN=$2
discover_bins; [ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"; discover_kicad; init_isolated_dirs
SLOW=${FRESH_SLOW_S:-$([ "${FRESH_AGENT:-}" = during ] && echo 12 || echo 0)}
if [ "$SLOW" != 0 ]; then
  RT_ROOT=$(dirname "$(dirname "$KICAD_BIN")"); SLOW_RT="$OUT/rt-slow/$(basename "$RT_ROOT")"; mkdir -p "$OUT/rt-slow"; cp -al "$RT_ROOT" "$SLOW_RT"; rm "$SLOW_RT/bin/kicad"
  { head -1 "$RT_ROOT/bin/kicad"; echo "sleep $SLOW"; tail -n +2 "$RT_ROOT/bin/kicad"; } >"$SLOW_RT/bin/kicad"; chmod +x "$SLOW_RT/bin/kicad"
  KICAD_BIN="$SLOW_RT/bin/kicad"; KICAD_CLI="$SLOW_RT/bin/kicad-cli"; export KICAD_BIN KICAD_CLI BACKPLANE_KICAD_BIN="$KICAD_BIN" BACKPLANE_KICAD_CLI="$KICAD_CLI"
  echo "INFO KiCad starts $SLOW s late: $SLOW_RT"
fi
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
# fresh: none of the harness's KiCad seed (its settings, tables), nothing of KiCad's anywhere in this home
rm -rf "$OUT/fh/.config/kicad" "$OUT/fh/.local/share/kicad" "$OUT/fh/.cache/kicad" "$OUT/fh/AppData"
# FRESH_PROJECT=<a project folder>: that project instead of the fixture (the release check opened KiCad's pic_programmer demo)
if [ -n "${FRESH_PROJECT:-}" ]; then
  ( cd "$OUT/proj" && git rm -q -r --cached . >/dev/null && find . -mindepth 1 -maxdepth 1 ! -name .git -exec rm -rf {} + )
  python3 "$KEMBED_LIB/project_copy.py" "$FRESH_PROJECT" "$OUT/proj"
  echo "{\"project\": \"$(basename "$(ls "$OUT/proj"/*.kicad_pro | head -1)")\"}" >"$OUT/proj/.backplane.json"
  git -C "$OUT/proj" add -A >/dev/null 2>&1; git -C "$OUT/proj" commit -qm "the project" >/dev/null 2>&1 || true
fi
# FRESH_MANAGED=1: KiCad as setup leaves it, the hub's own managed runtime in this home (<home>/kicad/<version>, as kicad.install
# unpacks it; linked from the runtime asked for), no other KiCad (BACKPLANE_KICAD_NO_SYSTEM=1), nothing named to the hub
if [ "${FRESH_MANAGED:-}" = 1 ]; then
  RT_ROOT=$(dirname "$(dirname "$KICAD_BIN")"); RT_VER=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["version"])' "$RT_ROOT/manifest.json")
  mkdir -p "$OUT/home/kicad"; cp -al "$RT_ROOT" "$OUT/home/kicad/$RT_VER"
  unset BACKPLANE_KICAD_BIN BACKPLANE_KICAD_CLI; KICAD_CLI=""; export BACKPLANE_KICAD_NO_SYSTEM=1
  echo "INFO the managed runtime: $OUT/home/kicad/$RT_VER"
fi
LIMIT=${FRESH_EDIT_S:-30}
export BACKPLANE_KICAD_DEBUG=1
start_xvfb_and_wm; start_hub
PRO=$(find "$OUT/proj" -maxdepth 3 -name '*.kicad_pro' -not -path '*/.backplane/*' | sort | head -1)
# an agent's tool call through the stand-in agent (seed_project.py's thread t1), answered in standin.log
ask() {
  local N; N="$1-$(date +%s%N)"
  local text; text=$(python3 -c 'import json,sys; print(json.dumps("mcpcall:" + json.dumps({"name": sys.argv[1], "arguments": json.loads(sys.argv[2])})))' "$1" "$2")
  hub_rpc "[{\"method\":\"turn.start\",\"params\":{\"thread\":\"t1\",\"text\":$text,\"msg\":\"fresh$N\"}}]" >>"$OUT/rpc.log" 2>&1 || true
}
AGENT=${FRESH_AGENT:-}
if [ "$AGENT" = first ]; then
  ask kicad_ipc_status "{\"path\":\"$PRO\"}"
  for _ in $(seq 600); do grep -q "kicad ipc: .* up" "$OUT/hub.log" 2>/dev/null && break; sleep 0.1; done
  echo "INFO the headless KiCad before the client: $(grep -o 'kicad ipc: .* up' "$OUT/hub.log" | head -1)"
fi
echo '[{"select":0},{"wait":2200},{"click":"rail:Board"},{"wait":2500},{"mark":"edit"},{"click":"ui:Edit"},{"kicad":{"wait_painted":240000,"kind":"PCB","tag":"ready"}},{"mark":"ready"},{"wait":5000}]' >"$OUT/script.json"
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 300 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
if [ "$AGENT" = during ]; then
  for _ in $(seq 600); do grep -q "kicad embed: start +[0-9]*ms spawned" "$OUT/hub.log" 2>/dev/null && break; sleep 0.1; done
  ask kicad_ipc_status "{\"path\":\"$PRO\"}"
fi
wait_for_log "$OUT/client.log" '"name":"edit","event":"mark"' 600 || { echo "FAIL the client never got to Edit"; exit 1; }
T0=$(date +%s%3N)
stall() {  # what KiCad and the hub are doing now
  local tag=$1 kp
  mkdir -p "$OUT/shots"
  xwininfo -root -tree >"$OUT/shots/$tag.xtree.txt" 2>&1 || true
  screen_shot "$OUT/shots/$tag.png" 2>/dev/null || true
  kp=$(pgrep -f "kicad.real|bin/kicad " -n 2>/dev/null || true)
  echo "INFO $tag: KiCad windows: $(grep -iE 'kicad|pcb|schem|setting|librar|migrat|config|welcome|font' "$OUT/shots/$tag.xtree.txt" | head -8 | sed 's/^ *//' | tr '\n' '|' | cut -c1-600)"
  echo "INFO $tag: the hub's last KiCad lines: $(grep -E 'kicad embed' "$OUT/hub.log" | grep -v 'request kiapi\|publish' | tail -4 | tr '\n' '|' | cut -c1-600)"
  if [ -n "$kp" ]; then
    echo "INFO $tag: KiCad pid $kp state $(awk '{print $3}' /proc/$kp/stat 2>/dev/null) cpu $(ps -o %cpu= -p $kp 2>/dev/null) rss $(ps -o rss= -p $kp 2>/dev/null) KB; threads: $(for t in /proc/$kp/task/*; do cat $t/wchan 2>/dev/null; echo; done | sort | uniq -c | sort -rn | head -4 | tr '\n' ' ')"
    echo "INFO $tag: KiCad's open files of note: $(ls -l /proc/$kp/fd 2>/dev/null | grep -oE '/[^ ]*(kicad|font|3d|sym|fp)[^ ]*' | sort -u | head -6 | tr '\n' ' ')"
  else echo "INFO $tag: no KiCad process"; fi
}
PAINTED=0; next=15000
while :; do
  grep -q '"name":"ready","event":"mark"' "$OUT/client.log" 2>/dev/null && { PAINTED=1; break; }
  kill -0 "$CP" 2>/dev/null || break
  el=$(( $(date +%s%3N) - T0 )); [ "$el" -gt 240000 ] && break
  [ "$el" -ge "$next" ] && { stall "stall-$((next / 1000))s"; next=$((next + 15000)); }
  sleep 0.25
done
R=$(last_kicad_report "$OUT/client.log" ready)
# from the client's own Edit (its "edit" mark, just before the click) to its "ready" mark (the board painted)
MS=$(python3 - "$OUT/client.log" <<'PY'
import json, sys
at = {}
for l in open(sys.argv[1], errors="ignore"):
    for n in ("edit", "ready"):
        if f'"name":"{n}","event":"mark"' in l and n not in at: at[n] = int(json.loads(l.split("GPUI_METRIC ", 1)[1])["epoch_ms"])
print(at["ready"] - at["edit"] if len(at) == 2 else -1)
PY
)
echo "INFO run $FRESH_RUN: first Edit painted in $MS ms (painted $PAINTED); the hub: $(grep -oE 'kicad embed: (start \+[0-9]+ms [a-z ]+|warming [^ ]+)' "$OUT/hub.log" | tr '\n' '|' | cut -c1-500)"
check "run $FRESH_RUN: the first Edit on a fresh home paints KiCad's board editor within $LIMIT s" "$([ "$PAINTED" = 1 ] && [ "$MS" -ge 0 ] && [ "$MS" -le $((LIMIT * 1000)) ] && echo 1 || echo 0)" "$MS ms"
if [ -n "$AGENT" ]; then
  # who holds the project's lock now (KiCad's flock on ~<name>.kicad_pro.lck): one process, the editor's
  LCK="$(dirname "$PRO")/~$(basename "$PRO").lck"
  HOLDERS=$(lslocks -n -o PID,PATH 2>/dev/null | awk -v p="$LCK" '$2 == p {print $1}' | sort -u | tr '\n' ' ')
  DLG=$(xwininfo -root -tree 2>/dev/null | grep -c '"File Open Warning"' || true)
  for _ in $(seq 120); do grep -q "^MCPCALL kicad_ipc_status " "$OUT/standin.log" 2>/dev/null && break; sleep 0.5; done
  ANS=$(grep "^MCPCALL kicad_ipc_status " "$OUT/standin.log" 2>/dev/null | tail -1 | cut -c1-300)
  echo "INFO agent ($AGENT): $ANS"
  echo "INFO the project's lock is held by: ${HOLDERS:-nobody}; KiCad processes: $(pgrep -u "$(id -u)" -f "$OUT/home/kicad|kicad.real|kicad-cli" -a 2>/dev/null | grep -c "$(basename "$PRO")" || true)"
  check "run $FRESH_RUN: no KiCad File Open Warning (the agent's KiCad call $AGENT the editor's start)" "$([ "${DLG:-0}" = 0 ] && echo 1 || echo 0)" "$DLG"
  check "run $FRESH_RUN: one process holds the project's lock" "$([ "$(echo $HOLDERS | wc -w)" = 1 ] && echo 1 || echo 0)" "$HOLDERS"
  check "run $FRESH_RUN: the agent's kicad_ipc_status was answered" "$([ -n "$ANS" ] && echo 1 || echo 0)" "$ANS"
fi
kill "$CP" 2>/dev/null || true
finish_suite "embed_fresh_edit_e2e ($EMBED_MODE, run $FRESH_RUN)"
