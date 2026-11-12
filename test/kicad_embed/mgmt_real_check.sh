#!/bin/bash
# Manual check of the BOM and Lifecycle panels on a COPY of a real project with the real parts search (pcbparts.dev): not in run_all
# (it leaves the machine, and takes minutes for the lookups). Prints the BOM's numbers and leaves screenshots in $OUT/shots.
#   KEMBED_PROJ_SRC=~/Documents/Nordic/Hubble_DK PRO_NAME=Hubble_DK test/kicad_embed/mgmt_real_check.sh
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/env_xvfb.sh
. "$HERE/lib/env_xvfb.sh"
: "${KEMBED_PROJ_SRC:?the project to copy}" "${PRO_NAME:?the project file name without .kicad_pro}"
WAIT_S=${WAIT_S:-420}
discover_bins
[ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PRO="$OUT/proj/$PRO_NAME.kicad_pro"
mkdir -p "$OUT/shots"
start_xvfb_and_wm
start_hub
info() { hub_rpc "[{\"method\":\"mgmt.watch\",\"params\":{\"project\":\"$PRO\"}}]" | python3 -c 'import json,sys
for l in sys.stdin:
    try: o=json.loads(l)
    except Exception: continue
    print(json.dumps(((o.get("result") or {}).get("data") or {}).get(sys.argv[1], {})))' "$1"; }
cat >"$OUT/script.json" <<'EOF'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":4000},{"click":"mgmt:bom"},{"wait":2000},{"mark":"bom_open"},{"wait":900000}]
EOF
HOME="$OUT/fh" BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_HOLD=1 \
  "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 1000 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
wait_for_log "$OUT/client.log" '"name":"bom_open","event":"mark"' 120 || { echo "FAIL the panel never opened"; exit 1; }
T0=$(date +%s)
while :; do B=$(info bom); C=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1]).get("checking", -1))' "$B"); [ "$C" = 0 ] && break; [ $(( $(date +%s) - T0 )) -gt "$WAIT_S" ] && break; sleep 5; done
echo "lookups done in $(( $(date +%s) - T0 )) s"
python3 - "$B" <<'PY'
import json, sys, collections
v = json.loads(sys.argv[1]); ls = v.get("lines", [])
st = collections.Counter((l.get("stock") or {}).get("status", "none") for l in ls)
cl = collections.Counter((l.get("stock") or {}).get("class", "") for l in ls if l.get("stock"))
print(f"source={v.get('source')} parts={v.get('parts')} lines={len(ls)} boards={v.get('boards')} missing_mpn={v.get('missing_mpn')} out={v.get('out')} badge={v.get('badge')}")
print(f"stock: {dict(st)}  class: {dict(cl)}  cost a board: ${v.get('cost')} over {v.get('priced')} of {v.get('lookups')} lines")
for l in ls:
    s = l.get("stock") or {}
    if s.get("status") in ("out", "low", "unknown"): print(f"  {', '.join(l['refs'])[:30]:30} {l.get('mpn') or l.get('fab'):28} {s.get('status'):8} {s.get('stock', '')} {s.get('why', '')}")
PY
python3 "$XINPUT_PY" move 10 10 >/dev/null 2>&1 || true
shot "real_bom"
GX=$(python3 "$DRIVER_PY" list-windows --pid "$CP" | python3 -c 'import json,sys; w=sorted([x for x in json.load(sys.stdin) if x.get("shown")], key=lambda x: -x.get("w",0)*x.get("h",0)); print(w[0]["xid"] if w else "")')
click_gpui_control "$OUT/client.log" "$GX" "mgmt:life" || true
sleep 3; python3 "$XINPUT_PY" move 10 10 >/dev/null 2>&1 || true; shot "real_life"
L=$(info life); python3 -c 'import json,sys; v=json.loads(sys.argv[1]); print("lifecycle:", v.get("counts"), "samples:", v.get("samples"))' "$L"
echo "artifacts: $OUT"
