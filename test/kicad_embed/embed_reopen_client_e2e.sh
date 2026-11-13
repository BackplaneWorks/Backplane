#!/bin/bash
# Backplane closed (or killed) while KiCad is up, then opened again on the same hub: Edit adopts the running KiCad's frames and shows the
# board, never a silent "Starting KiCad…" (lane K's clean-box journey: state ready, frames [], holder 0, the pane waiting for ever).
# Both ways the client goes: killed (SIGKILL: the X server moves KiCad's holder to the root through the save-set) and quit (SIGTERM).
# A re-adopt is a reparent and a show: the board, and the schematic frame the earlier client had shown, each painted in under
# REOPEN_MS (500) from the press. fork only. KEMBED_PROJ_SRC for another project.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_reopen_client_e2e: fork only"; exit 0; }
discover_bins; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
start_xvfb_and_wm; start_hub

script() { # $1 file, $2 tag
  python3 - "$1" "$2" <<'PY'
import json, sys
s = [{"select": 0}, {"wait": 2200}, {"click": "rail:Board"}, {"wait": 2500}, {"click": "ui:Edit"}, {"kicad": {"wait_painted": 60000, "kind": "PCB", "tag": sys.argv[2]}},
     {"wait": 800}, {"click": "kicad-tab:SCH"}, {"kicad": {"wait_painted": 60000, "kind": "SCH", "tag": sys.argv[2] + "_sch"}},
     {"wait": 800}, {"click": "kicad-tab:PCB"}, {"wait": 800}, {"mark": "done"}, {"wait": 600000}]
json.dump(s, open(sys.argv[1], "w"))
PY
}
run_client() { # $1 n, $2 tag: starts the client, waits for its mark; prints its pid
  script "$OUT/script$1.json" "$2"
  HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script$1.json" --exit-after 200 >"$OUT/client$1.log" 2>&1 &
  local p=$!; track_pid "$p"
  for _ in $(seq 1200); do grep -q '"name":"done","event":"mark"' "$OUT/client$1.log" 2>/dev/null && break; kill -0 "$p" 2>/dev/null || break; sleep 0.1; done
  echo "$p"
}
# a re-adopt is a reparent and a show: from the press (Edit, the Schematic tab) to that frame painted, at most this
REOPEN_MS=${REOPEN_MS:-500}
took() { # $1 log, $2 the press's control id, $3 tag: ms from the press to the frame's painted report
  python3 - "$1" "$2" "$3" <<'PY'
import json, re, sys
t = open(sys.argv[1]).read()
c = re.search(r'"id":"%s"[^\n]*?"event":"click","t_ms":([0-9.]+)' % re.escape(sys.argv[2]), t)
r = re.search(r'"tag":"%s","event":"kicad","t_ms":([0-9.]+)' % re.escape(sys.argv[3]), t)
print(int(float(r.group(1)) - float(c.group(1))) if c and r else -1)
PY
}
bounded() { # $1 log, $2 control, $3 tag, $4 what
  local ms; ms=$(took "$1" "$2" "$3"); echo "PERF ${3}_from_press ${ms}ms"
  check "$4 in under ${REOPEN_MS} ms from the press (${ms} ms)" "$([ "$ms" -ge 0 ] && [ "$ms" -lt "$REOPEN_MS" ] && echo 1 || echo 0)"
}
sch_verdict() { # $1 log, $2 tag, $3 what
  local R; R=$(last_kicad_report "$1" "${2}_sch")
  local P; P=$(python3 -c '
import json, sys
r = json.loads(sys.argv[1] or "{}")
f = next((f for f in r.get("frames", []) if f.get("xid") == r.get("shown") and f.get("kind") == "SCH"), {})
print(f.get("painted", 0))' "$R")
  check "$3: the schematic shown and painted" "$([ "${P:-0}" -ge 15 ] && echo 1 || echo 0)" "$(echo "$R" | head -c 200)"
}
verdict() { # $1 log, $2 tag, $3 what
  local R; R=$(last_kicad_report "$1" "$2")
  IFS=$'\t' read -r ST PAINTED WAIT SAY NFR < <(python3 -c '
import json, sys
r = json.loads(sys.argv[1] or "{}")
f = next((f for f in r.get("frames", []) if f.get("xid") == r.get("shown") and f.get("kind") == "PCB"), {})
print(r.get("state") or "-", f.get("painted", 0), r.get("wait_ms") or -1, json.dumps(r.get("say") or ""), len(r.get("frames") or []), sep="\t")' "$R")
  echo "PERF ${2}_painted ${WAIT}ms state=$ST frames=$NFR painted=$PAINTED say=$SAY"
  check "$3: the board shown and painted" "$([ "${PAINTED:-0}" -ge 15 ] && echo 1 || echo 0)" "state=$ST frames=$NFR say=$SAY wait=${WAIT}ms"
}

P1=$(run_client 1 first)
verdict "$OUT/client1.log" first "first Edit"
sch_verdict "$OUT/client1.log" first "first Edit"
PID_K=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1] or "{}").get("pid") or 0)' "$(last_kicad_report "$OUT/client1.log" first)")
kill -KILL "$P1" 2>/dev/null || true; sleep 2
check "KiCad kept running after the client was killed" "$(kill -0 "$PID_K" 2>/dev/null && echo 1 || echo 0)" "pid=$PID_K"
P2=$(run_client 2 after_kill)
shot after_kill
verdict "$OUT/client2.log" after_kill "Edit after the client was killed"
bounded "$OUT/client2.log" ui:Edit after_kill "after the kill, the board"
sch_verdict "$OUT/client2.log" after_kill "after the kill"
bounded "$OUT/client2.log" kicad-tab:SCH after_kill_sch "after the kill, the schematic"
kill -TERM "$P2" 2>/dev/null || true
for _ in $(seq 50); do kill -0 "$P2" 2>/dev/null || break; sleep 0.1; done
P3=$(run_client 3 after_quit)
shot after_quit
verdict "$OUT/client3.log" after_quit "Edit after the client quit"
bounded "$OUT/client3.log" ui:Edit after_quit "after the quit, the board"
sch_verdict "$OUT/client3.log" after_quit "after the quit"
bounded "$OUT/client3.log" kicad-tab:SCH after_quit_sch "after the quit, the schematic"
check "the same KiCad all along (adopted, not restarted)" "$(kill -0 "$PID_K" 2>/dev/null && echo 1 || echo 0)" "pid=$PID_K"
kill -TERM "$P3" 2>/dev/null || true
finish_suite "embed_reopen_client_e2e ($EMBED_MODE)"
