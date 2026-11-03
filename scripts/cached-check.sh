#!/bin/sh
# scripts/check.sh's type-check, but only for the modules whose content key
# (the module, its full import closure, the compiler: scripts/content-cache.py)
# has no recorded pass. A module whose dependency changed is a miss, so a
# cached pass never covers changed inputs. PROOF.bend is not touched (see
# content-cache.py plan-proof). Not a replacement for check.sh: the nightly
# main run uses `scripts/cached-check.sh --all`, which ignores every entry.
#
#   scripts/cached-check.sh [--all]       exit 0 only when every module passed
# BP_CONTENT_CACHE, BP_CONTENT_CACHE_SHARED: see content-cache.py.
# BP_CONTENT_CACHE_SHARED_WRITE=1 (trusted main builds only) also files the
# passes in the first shared cache.
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
  echo "cached-check: all $total modules cached ($(( $(date +%s) - start ))s)"
  exit 0
fi
# only work that needs the checker is heavy: a plan that is all hits runs none,
# and the guard (memory floor, lock) is never lowered for the rest
heavy_guard check "$@"
echo "cached-check: $(printf '%s\n' "$misses" | wc -l | tr -d ' ') of $total modules to check"
shared=""; [ "${BP_CONTENT_CACHE_SHARED_WRITE:-0}" = 1 ] && shared=--shared-write
passed=""
rest=$(printf '%s\n' "$misses" | grep -v '^LAWS.bend$' || true)
if [ -n "$rest" ]; then
  mkdir "$work/m"
  # shellcheck disable=SC2086
  python3 scripts/check-manifest.py generate "$work/m" 1 $rest
  python3 scripts/check-manifest.py verify "$work/m" $rest
  rc=0; out=$(nice -n 19 bend "$work/m/manifest-0.bend" --check-only 2>&1) || rc=$?
  bend_verdict "$rc" "$out" || { echo "manifest (status $rc):"; echo "$out"; exit 1; }
  passed=$rest
fi
if printf '%s\n' "$misses" | grep -q '^LAWS.bend$'; then
  rc=0; out=$(bend LAWS.bend --check-only 2>&1) || rc=$?
  case $out in
    *TODO*) ;; # as check.sh: LAWS.bend may carry TODO
    *) bend_verdict "$rc" "$out" || { echo "LAWS.bend (status $rc):"; echo "$out"; exit 1; } ;;
  esac
  passed="$passed
LAWS.bend"
fi
# only now, after every check passed, and only for inputs unchanged since the plan
# shellcheck disable=SC2086
python3 scripts/content-cache.py record-check $shared --plan "$work/plan.json" $passed
echo "cached-check: ok, $(( $(date +%s) - start ))s"
