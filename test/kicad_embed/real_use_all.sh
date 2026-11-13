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
RUN_RC=0
one() {
  local bn=$1 bs=$2 rn=$3 rk=$4
  [ -d "$ROOT/src-$bn" ] || stage_board "$bs" "$ROOT/src-$bn"
  if [ "${KEMBED_ONLY_PLAN:-0}" != 1 ]; then
  echo "=== $bn on $rn"
  OUT="$ROOT/$bn-$rn" EMBED_MODE=fork BACKPLANE_KICAD_BIN="$rk" BACKPLANE_KICAD_CLI="$(dirname "$rk")/kicad-cli" KEMBED_PROJ_SRC="$ROOT/src-$bn" \
    timeout --foreground 1500 "$HERE/real_use_journeys.sh" >"$ROOT/$bn-$rn.log" 2>&1
  local journey_rc=$?
  echo "rc=$journey_rc" >>"$ROOT/$bn-$rn.log"
  [ "$journey_rc" = 0 ] || RUN_RC=1
  grep -h "^PERF\|^DIALOG\|^INFO\|^FAIL\|^DIAG\|rc=" "$ROOT/$bn-$rn.log"
  fi
  # a runtime with the floorplanner: the plan journey on a copy stripped of copper with the fork's demo plan beside it
  if [ "${KEMBED_PLAN:-1}" = 1 ] && plan_demo "$rk" "$ROOT/plan-demo-$rn.py"; then
    [ -d "$ROOT/src-$bn-plan" ] || { stage_board "$bs" "$ROOT/src-$bn-plan"
      local pcb; pcb=$(ls "$ROOT/src-$bn-plan"/*.kicad_pcb | head -1)
      python3 -I "$ROOT/plan-demo-$rn.py" "$pcb" --strip --out "$ROOT/plan-out-$bn" >"$ROOT/plan-demo-$bn.log" 2>&1
      cp "$ROOT/plan-out-$bn"/*.kicad_pcb "$ROOT/plan-out-$bn"/*.kicad_plan "$ROOT/src-$bn-plan/"; }
    echo "=== $bn on $rn: plan"
    OUT="$ROOT/$bn-$rn-plan" EMBED_MODE=fork BACKPLANE_KICAD_BIN="$rk" BACKPLANE_KICAD_CLI="$(dirname "$rk")/kicad-cli" KEMBED_PROJ_SRC="$ROOT/src-$bn-plan" \
      KEMBED_JOURNEYS=plan timeout --foreground 900 "$HERE/real_use_journeys.sh" >"$ROOT/$bn-$rn-plan.log" 2>&1
    local plan_rc=$?
    echo "rc=$plan_rc" >>"$ROOT/$bn-$rn-plan.log"
    [ "$plan_rc" = 0 ] || RUN_RC=1
    grep -h "^PERF\|^DIALOG\|^INFO\|^FAIL\|^DIAG\|rc=" "$ROOT/$bn-$rn-plan.log"
  fi
}
real_use_matrix one
python3 -I "$HERE/lib/real_use_table.py" "$ROOT"; RC=$?
[ "$RUN_RC" = 0 ] || RC=1
echo "artifacts: $ROOT"
exit "$RC"
