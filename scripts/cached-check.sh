#!/bin/sh
# Standalone module PRE-CHECK ONLY, not scripts/check.sh's aggregate gate.
# Exit3 for scoped pass,4 for standalone failure; neither is aggregate green.
# Earlier aggregate fills/template instances can change a consumer's verdict;
# each missing module now starts as its own exact entry in a fresh process.
# Only the modules whose content key
# (the module, its full import closure, the compiler: scripts/content-cache.py)
# has no recorded pass. A module whose dependency changed is a miss, so a
# cached pass never covers changed inputs. PROOF.bend is not touched (see
# content-cache.py plan-proof). Not a replacement for check.sh: no
# workflow runs this. --all ignores every standalone entry; a nightly gate
# must still run the original aggregate check and the full proof suite.
#
#   scripts/cached-check.sh [--all]       advisory exit3; errors stay nonzero
# BP_CONTENT_CACHE: the local cache (see content-cache.py; no shared cache yet).
set -eu
cd "$(dirname "$0")/.."
. scripts/heavy-guard.sh
export BEND_NO_TELEMETRY=1 BUN_JSC_forceRAMSize=${BUN_JSC_forceRAMSize:-4294967296}
ulimit -s 1048576 2>/dev/null || true
export BUN_JSC_maxPerThreadStackUsage=${BUN_JSC_maxPerThreadStackUsage:-536870912}
. scripts/bendok.sh
all=""
case ${1:-} in --all) all=--all ;; "") ;; *) echo "usage: scripts/cached-check.sh [--all]" >&2; exit 2 ;; esac
work=$(mktemp -d .bend-check.XXXXXX)
trap 'rm -rf "$work"' 0 1 2 15
python3 scripts/laws-shape.py
python3 scripts/content-cache.py plan-check $all --plan "$work/plan.json" >"$work/plan.out"
misses=$(python3 -c 'import json,sys; print("\n".join(json.load(open(sys.argv[1]))["miss"]))' "$work/plan.out")
standalone_result() {
  _status=0
  python3 scripts/content-cache.py run-check --plan "$work/plan.json" || _status=$?
  case $_status in
    0|3) echo "cached-check: standalone pre-check only; aggregate scripts/check.sh remains required"; exit 3 ;;
    4) echo "cached-check: standalone failure/inconclusive; aggregate scripts/check.sh remains required"; exit 4 ;;
    *) exit "$_status" ;;
  esac
}
if [ -z "$misses" ]; then
  # Planning is not a gate: source/tools/entries may have changed since lookup.
  standalone_result
fi
# only work that needs the checker is heavy: a plan that is all hits runs none,
# and the guard (memory floor, lock) is never lowered for the rest
heavy_guard check "$@"
echo "cached-check: $(printf '%s\n' "$misses" | wc -l | tr -d ' ') standalone entries to check"
# bend runs, the verdict policy applies, and only passes with unchanged inputs
# are recorded, all inside content-cache.py (there is no way to record a pass
# from outside it)
standalone_result
