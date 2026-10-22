#!/bin/sh
# The gates a change can affect, and only those: the tests whose inputs
# (scripts/ci-keys.py: import closure, driver, fixtures, declared inputs, the
# test scripts) contain a file the diff touches, the type-check of every file
# whose closure does, and the proof when its closure does. Passes recorded for
# byte-identical inputs are reused; the rest run on the warm Boat slots, as
# local CI's do (scripts/ci-fast.py). What the diff cannot reach is taken as
# BASE left it: run local CI (bp-coord's local-ci.sh) for the whole gate.
#
#   scripts/focus.sh BASE          this working tree (uncommitted and untracked
#                                  files included) against BASE
#   scripts/focus.sh BASE HEAD     the commit HEAD against BASE, in a detached
#                                  worktree of its own
# FOCUS_STEPS (default check,proof,tests; add js for the js build and smoke),
# FOCUS_LOGS (default ~/.cache/backplane-bend/focus/logs/<time>), and
# ci-fast.py's settings (LOCAL_CI_FULL=1, LOCAL_CI_SHARDS, --local via
# FOCUS_LOCAL=1). Exit 0 only when everything it ran passed.
set -eu
cd "$(dirname "$0")/.."
base=${1:?usage: scripts/focus.sh BASE [HEAD]}
git rev-parse -q --verify "$base^{commit}" >/dev/null || { echo "focus.sh: $base is not a commit" >&2; exit 2; }
top=${XDG_CACHE_HOME:-$HOME/.cache}/backplane-bend/focus
logs=${FOCUS_LOGS:-$top/logs/$(date +%Y%m%d-%H%M%S)-$$}
w=$PWD
if [ -n "${2:-}" ]; then
  sha=$(git rev-parse "$2^{commit}") || { echo "focus.sh: $2 is not a commit" >&2; exit 2; }
  w=$top/wt/$(echo "$sha" | cut -c1-12)
  if [ ! -d "$w" ]; then
    mkdir -p "$top/wt"
    git worktree add -q --detach "$w" "$sha"
  fi
  [ "$(git -C "$w" rev-parse HEAD)" = "$sha" ] && [ -z "$(git -C "$w" status --porcelain)" ] \
    || { echo "focus.sh: $w is not a clean checkout of $sha" >&2; exit 2; }
fi
set -- --logs "$logs" --focus "$base" --steps "${FOCUS_STEPS:-check,proof,tests}"
[ "${FOCUS_LOCAL:-0}" = 1 ] && set -- "$@" --local
echo "focus: $(git -C "$w" rev-parse --short HEAD)$( [ "$w" = "$PWD" ] && [ -n "$(git status --porcelain)" ] && echo ' + working tree') against $(git rev-parse --short "$base"), logs $logs"
start=$(date +%s)
rc=0
python3 "$w/scripts/ci-fast.py" "$@" || rc=$?
echo "focus: $( [ $rc = 0 ] && echo PASS || echo FAIL) in $(( $(date +%s) - start ))s"
exit $rc
