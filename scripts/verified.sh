#!/bin/sh
# Records that a gate passed in full on exactly these inputs, so a later run
# on the same inputs (another worktree, a deploy after the merge) need not
# run it again. A record names the gate (check: scripts/check.sh's type-check
# and every proof; test: every test of scripts/test.sh, unsharded) and is
# keyed by scripts/inputs-key.sh over the checkout's sources, bend, and the
# gate's own scripts. The gate takes the key before it starts and records
# only if the inputs hash the same after it passed, so a file changed while
# it ran is never certified; any difference is another key, so the gate runs.
#   k=$(scripts/verified.sh key check|test)      before the gate
#   scripts/verified.sh mark check|test "$k"     after a full pass
#   scripts/verified.sh has check|test           0 when a record matches now
# BACKPLANE_VERIFIED: the records' folder (default
# ~/.cache/backplane-bend/verified; 0 = never record nor reuse).
set -u
cd "$(dirname "$0")/.." || exit 1
op=${1:-}
kind=${2:-}
case $kind in check|test) ;; *) echo "verified.sh: key|mark|has check|test" >&2; exit 2 ;; esac
dir=${BACKPLANE_VERIFIED-${XDG_CACHE_HOME:-$HOME/.cache}/backplane-bend/verified}
[ "$dir" = 0 ] && exit 1
key() {
  scripts/inputs-key.sh "gate:$kind" scripts/check.sh scripts/test.sh scripts/bendok.sh scripts/check-manifest.py scripts/test-shard.py scripts/inputs-key.sh scripts/verified.sh
}
case $op in
  key)
    key ;;
  mark)
    before=${3:-}
    [ -n "$before" ] || { echo "verified.sh: no key taken before the gate; nothing recorded" >&2; exit 1; }
    now=$(key) || { echo "verified.sh: the inputs could not be hashed; nothing recorded" >&2; exit 1; }
    [ "$now" = "$before" ] || { echo "verified.sh: the inputs changed while the gate ran; nothing recorded" >&2; exit 1; }
    mkdir -p "$dir" || exit 1
    printf '%s %s\n' "$kind" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >"$dir/$now.$kind.$$" && mv "$dir/$now.$kind.$$" "$dir/$now.$kind" ;;
  has)
    k=$(key) || exit 1
    [ -n "$k" ] && [ -f "$dir/$k.$kind" ] ;;
  *) echo "verified.sh: key|mark|has check|test" >&2; exit 2 ;;
esac
