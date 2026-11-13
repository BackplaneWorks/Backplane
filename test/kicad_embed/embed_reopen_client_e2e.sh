#!/bin/bash
# Backplane closed (or killed) while KiCad is up, then opened again on the same hub: Edit adopts the running KiCad's frames and shows the
# board, never a silent "Starting KiCad…" (lane K's clean-box journey: state ready, frames [], holder 0, the pane waiting for ever).
# Both ways the client goes: killed (SIGKILL: the X server moves KiCad's holder to the root through the save-set) and quit (SIGTERM).
# fork only. KEMBED_PROJ_SRC for another project.
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
     {"mark": "done"}, {"wait": 600000}]
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
PID_K=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1] or "{}").get("pid") or 0)' "$(last_kicad_report "$OUT/client1.log" first)")
kill -KILL "$P1" 2>/dev/null || true; sleep 2
check "KiCad kept running after the client was killed" "$(kill -0 "$PID_K" 2>/dev/null && echo 1 || echo 0)" "pid=$PID_K"
P2=$(run_client 2 after_kill)
shot after_kill
verdict "$OUT/client2.log" after_kill "Edit after the client was killed"
kill -TERM "$P2" 2>/dev/null || true
for _ in $(seq 50); do kill -0 "$P2" 2>/dev/null || break; sleep 0.1; done
P3=$(run_client 3 after_quit)
shot after_quit
verdict "$OUT/client3.log" after_quit "Edit after the client quit"
check "the same KiCad all along (adopted, not restarted)" "$(kill -0 "$PID_K" 2>/dev/null && echo 1 || echo 0)" "pid=$PID_K"
kill -TERM "$P3" 2>/dev/null || true
finish_suite "embed_reopen_client_e2e ($EMBED_MODE)"
