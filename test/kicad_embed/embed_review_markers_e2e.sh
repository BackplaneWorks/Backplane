#!/bin/bash
# Suite: the design review's findings as host review markers in the open KiCad (review/markers.rs; Backplane KiCad bp.13 SetHostMarkers).
#   M1  a review run (design_review, scope save) on a board with a too-narrow track puts its finding in the PCB editor as the marker set
#       "review" (GetMarkers include_host: type host, host.set review, the finding's id), at the track
#   M2  nothing is written to the design: the board file is byte-identical after the markers are shown
#   M3  a run with the track fixed clears the set
# Copies only (seed_project.py), the board open in the embedded editor. Fork mode; skips on runtimes without SetHostMarkers.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_review_markers_e2e: fork only"; exit 0; }
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
grep -qa "SetHostMarkers" "$(dirname "$KICAD_BIN")/../share/backplane/api.desc" 2>/dev/null || { echo "SKIP embed_review_markers_e2e: this KiCad has no SetHostMarkers (bp.13+)"; exit 0; }
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PROJ="$OUT/proj"; PCB="$PROJ/ecc83-pp.kicad_pcb"; mkdir -p "$OUT/shots"
# one track made 0.05 mm wide (under every fab's limit): the review's fab source anchors its finding to that track
python3 - "$PCB" <<'PY'
import re, sys
p = sys.argv[1]; t = open(p).read()
m = re.search(r"\(segment\b.*?\(width ([\d.]+)\)", t, re.S)
assert m, "no track"
t = t[:m.start(1)] + "0.05" + t[m.end(1):]
open(p, "w").write(t)
PY
git -C "$PROJ" commit -qam "a narrow track" 2>/dev/null || true
start_xvfb_and_wm
export BP_STREAM_N=1 BP_STREAM_MS=10
start_hub
cat >"$OUT/script.json" <<'JSON'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},
 {"kicad":{"wait_painted":60000,"kind":"PCB","tag":"ready"}},{"wait":1500},{"click":"kicad:full"},{"wait":1500},{"mark":"edit_ready"},{"wait":150000}]
JSON
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_HOLD=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 200 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"edit_ready","event":"mark"' 480 || { echo "FAIL the board never opened in KiCad"; tail -n 20 "$OUT/client.log"; exit 1; }
ask() {
  local N; N="$1-$(date +%s%N)"; local before; before=$(grep -c "^MCPCALL" "$OUT/standin.log" 2>/dev/null || echo 0)
  local text; text=$(python3 -c 'import json,sys; print(json.dumps("mcpcall:" + json.dumps({"name": sys.argv[1], "arguments": json.loads(sys.argv[2])})))' "$1" "$2")
  hub_rpc "[{\"method\":\"turn.start\",\"params\":{\"thread\":\"t1\",\"text\":$text,\"msg\":\"mmark$N\"}}]" >>"$OUT/rpc.log" 2>&1 || true
  for _ in $(seq 480); do [ "$(grep -c "^MCPCALL" "$OUT/standin.log" 2>/dev/null || echo 0)" -gt "$before" ] && break; sleep 0.25; done
  sleep 2
  grep "^MCPCALL $1 " "$OUT/standin.log" | tail -1 || true
}
host_markers() { ask kicad_ipc "{\"path\":\"$PCB\",\"type\":\"kiapi.backplane.embed.GetMarkers\",\"message\":{\"kind\":\"FK_PCB\",\"include_host\":true}}"; }
SHA0=$(sha256sum "$PCB" | cut -d' ' -f1)
R1=$(ask design_review '{"scope":"save"}'); echo "$R1" | cut -c1-300
ID=$(echo "$R1" | python3 -c 'import sys,re; t=sys.stdin.read().replace("\\\\\"","\"").replace("\\\"","\""); m=re.search(r"\"id\":\"(fab[^\"]*)\"[^{}]*?\"rule\":\"[^\"]*track", t) or re.search(r"\"id\":\"(fab[^\"]*)\"", t); print(m.group(1) if m else "")')
echo "INFO the fab finding: $ID"
sleep 2
M1=$(host_markers); echo "$M1" | cut -c1-500
check "the review's finding is a host marker in KiCad (set review)" "$(echo "$M1" | grep -q 'review' && echo "$M1" | grep -q 'host' && { [ -z "$ID" ] || echo "$M1" | grep -q "$ID"; } && echo 1 || echo 0)" "$(echo "$M1" | cut -c1-300)"
shot m1_markers
check "nothing is written to the board (byte-identical)" "$([ "$(sha256sum "$PCB" | cut -d' ' -f1)" = "$SHA0" ] && echo 1 || echo 0)"
# the track fixed (as a person would, then saved): the next run clears the set
python3 - "$PCB" <<'PY'
import sys; p = sys.argv[1]; t = open(p).read(); open(p, "w").write(t.replace("(width 0.05)", "(width 0.25)", 1))
PY
R2=$(ask design_review '{"scope":"save"}'); sleep 2
M2=$(host_markers); echo "$M2" | cut -c1-300
check "a run without the finding clears it from KiCad" "$({ [ -n "$ID" ] && ! echo "$M2" | grep -q "$ID"; } && echo 1 || echo 0)" "$(echo "$M2" | cut -c1-300)"
shot m2_cleared
finish_suite "embed_review_markers_e2e ($EMBED_MODE)"
