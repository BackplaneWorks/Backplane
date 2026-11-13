#!/bin/bash
# Suite: the design review from the Fab panel (native/host/src/review/, fab/panel.rs, screens/mgmt_fab.rs) into the one Checks list.
#   R1  the Fab panel's Review runs a full review: .backplane/design-review.json is written with findings by source, review.md beside it
#   R2  the review's sources are in the Checks list (kicad.checks summary: fab, bom, outputs, assembly, mech beside drc and erc)
#   R3  Findings opens the Checks panel; screenshots of both panels
#   R4  a save of the design (here: its fab file rewritten) runs the quick review by itself (scope "save") within seconds
# Copies only (seed_project.py; KEMBED_PROJ_SRC=<a real project> for a real board), on a private Xvfb + i3. Fork mode (kicad-cli for DRC).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
[ "${1:-}" != "" ] && CLIENT_BIN=$1
[ "${2:-}" != "" ] && HUB_BIN=$2
[ "${3:-}" != "" ] && OUT=$(mkdir -p "$3" && cd "$3" && pwd)
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
[ "$EMBED_MODE" != "spike" ] && discover_kicad
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
start_xvfb_and_wm
start_hub
PROJ="$OUT/proj"
PRO=$(python3 -c 'import glob,sys; print(sorted(glob.glob(sys.argv[1] + "/*.kicad_pro"))[0])' "$PROJ")
mkdir -p "$OUT/shots"
WAIT=${REVIEW_WAIT_MS:-60000}
cat >"$OUT/script.json" <<EOF
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":6000},{"mark":"board"},{"wait":800},
 {"click":"mgmt:fab"},{"wait":2500},{"mark":"fab_open"},
 {"click":"mgmt:fab:review"},{"wait":$WAIT},{"mark":"reviewed"},{"wait":1500},
 {"click":"mgmt:fab:checks"},{"wait":3000},{"mark":"checks_open"},{"wait":1500}]
EOF
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_HOLD=1 \
"$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after $(( WAIT / 1000 + 120 )) >"$OUT/client.log" 2>&1 &
CP=$!
track_pid "$CP"

# (wait_for_log counts quarter seconds)
wait_for_log "$OUT/client.log" '"name":"fab_open","event":"mark"' 720 || { echo "FAIL the Fab panel never opened"; tail -n 30 "$OUT/client.log"; exit 1; }
sleep 0.5; shot "r0_fab_open"
wait_for_log "$OUT/client.log" '"name":"reviewed","event":"mark"' $(( (WAIT / 1000 + 120) * 4 )) || true
sleep 0.5; shot "r1_reviewed"
RUN="$PROJ/.backplane/design-review.json"
check "Review wrote the run with findings by source" "$(python3 -c 'import json,sys; v=json.load(open(sys.argv[1])); print(int(v["scope"]=="full" and isinstance(v["by_source"], dict) and "fab" in v["by_source"]))' "$RUN" 2>/dev/null || echo 0)"
check "review.md is beside it" "$([ -s "$PROJ/.backplane/review.md" ] && echo 1 || echo 0)"
python3 -c 'import json,sys; v=json.load(open(sys.argv[1])); print("REVIEW", json.dumps(v["counts"]), json.dumps({k: c for k, c in v["by_source"].items() if any(c.values())}), "skipped:", len(v["skipped"]), "took_ms:", v["took_ms"])' "$RUN" 2>/dev/null || true
hub_rpc "[{\"method\":\"kicad.checks.watch\",\"params\":{\"project\":\"$PRO\"}}]" >"$OUT/checks.out" 2>&1 || true
check "The review's sources are in the Checks list" "$(python3 - "$OUT/checks.out" <<'PY'
import json, sys
ok = 0
for line in open(sys.argv[1], errors="replace"):
    try: o = json.loads(line)
    except Exception: continue
    s = (o.get("result") or {}).get("sources") or {}
    ok = int(all(k in s for k in ("fab", "bom", "outputs", "assembly", "mech")))
print(ok)
PY
)" "$(head -c 400 "$OUT/checks.out")"
wait_for_log "$OUT/client.log" '"name":"checks_open","event":"mark"' 240 || true
sleep 0.5; shot "r2_checks_open"
hub_rpc "[{\"method\":\"mgmt.watch\",\"params\":{\"project\":\"$PRO\"}}]" >"$OUT/mgmt.out" 2>&1 || true
check "Findings opened the Checks panel" "$(python3 -c 'import json,sys; print(int(any((json.loads(l).get("result") or {}).get("state",{}).get("open")=="checks" for l in open(sys.argv[1]) if l.startswith("{"))))' "$OUT/mgmt.out" 2>/dev/null || echo 0)"
# R4: a save reviews by itself (the file checks, debounced)
FAB=$(python3 -c 'import glob,sys; print((sorted(glob.glob(sys.argv[1] + "/*.fab.json")) + [""])[0])' "$PROJ")
if [ -n "$FAB" ]; then
  BEFORE=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["at"])' "$RUN" 2>/dev/null || echo 0)
  sleep 1.2; python3 -c 'import sys; p=sys.argv[1]; t=open(p).read(); open(p,"w").write(t.rstrip()+"\n\n")' "$FAB"
  for _ in $(seq 40); do sleep 0.5; S=$(python3 -c 'import json,sys; v=json.load(open(sys.argv[1])); print(v["scope"], v["at"])' "$RUN" 2>/dev/null || true); case "$S" in "save "*) [ "${S#save }" -gt "$BEFORE" ] && break ;; esac; done
  check "A save of the design ran the quick review by itself ($S)" "$(case "$S" in "save "*) [ "${S#save }" -gt "$BEFORE" ] && echo 1 || echo 0 ;; *) echo 0 ;; esac)"
fi
finish_suite "embed_review_e2e ($EMBED_MODE)"
