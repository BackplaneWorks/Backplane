#!/bin/bash
# real_use_journeys.sh on each real board x runtime (lib/real_use.sh), one after another, then the table (real_use_table.py) and the budgets.
#   REAL_HARDWARE=1 test/kicad_embed/real_use_all.sh [OUT_ROOT]     KEMBED_BOARDS / KEMBED_RUNTIMES / KEMBED_JOURNEYS narrow it
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(cd "$HERE/../.." && pwd)
. "$REPO_ROOT/scripts/lib-bptmp.sh"
. "$HERE/lib/real_use.sh"
ROOT=${1:-$(mktemp -d "$BP_TMP/bp-realuse.XXXXXX")}
mkdir -p "$ROOT"
one() {
  local bn=$1 bs=$2 rn=$3 rk=$4
  [ -d "$ROOT/src-$bn" ] || stage_board "$bs" "$ROOT/src-$bn"
  echo "=== $bn on $rn"
  OUT="$ROOT/$bn-$rn" EMBED_MODE=fork BACKPLANE_KICAD_BIN="$rk" BACKPLANE_KICAD_CLI="$(dirname "$rk")/kicad-cli" KEMBED_PROJ_SRC="$ROOT/src-$bn" \
    timeout --foreground 1500 "$HERE/real_use_journeys.sh" >"$ROOT/$bn-$rn.log" 2>&1
  echo "rc=$?" >>"$ROOT/$bn-$rn.log"
  grep -h "^PERF\|^DIALOG\|^INFO\|^FAIL\|^DIAG\|rc=" "$ROOT/$bn-$rn.log"
}
real_use_matrix one
python3 -I "$HERE/lib/real_use_table.py" "$ROOT"
echo "artifacts: $ROOT"
