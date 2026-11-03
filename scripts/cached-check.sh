#!/bin/sh
# scripts/check.sh's type-check, but only for the modules whose content key
# (the module, its full import closure, the compiler: scripts/content-cache.py)
# has no recorded pass. A module whose dependency changed is a miss, so a
# cached pass never covers changed inputs. PROOF.bend is not touched (see
# content-cache.py plan-proof). Not a replacement for check.sh: no
# workflow runs this yet. Proposed, not wired: the nightly main run would use
# `scripts/cached-check.sh --all`, which ignores every entry.
#
#   scripts/cached-check.sh [--all]       exit 0 only when every module passed
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
start=$(date +%s)
python3 scripts/content-cache.py plan-check $all --plan "$work/plan.json" >"$work/plan.out"
misses=$(python3 -c 'import json,sys; print("\n".join(json.load(open(sys.argv[1]))["miss"]))' "$work/plan.out")
total=$(python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); print(len(d["hit"])+len(d["miss"]))' "$work/plan.out")
if [ -z "$misses" ]; then
  # Planning is not a gate: source/tools/entries may have changed since lookup.
  python3 scripts/content-cache.py run-check --plan "$work/plan.json"
  echo "cached-check: all $total modules cached ($(( $(date +%s) - start ))s)"
  exit 0
fi
# only work that needs the checker is heavy: a plan that is all hits runs none,
# and the guard (memory floor, lock) is never lowered for the rest
heavy_guard check "$@"
echo "cached-check: $(printf '%s\n' "$misses" | wc -l | tr -d ' ') of $total modules to check"
# bend runs, the verdict policy applies, and only passes with unchanged inputs
# are recorded, all inside content-cache.py (there is no way to record a pass
# from outside it)
python3 scripts/content-cache.py run-check --plan "$work/plan.json"
echo "cached-check: ok, $(( $(date +%s) - start ))s"
