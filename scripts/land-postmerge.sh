#!/bin/sh
# The Windows gate after a merge that did not need it to block (scripts/land.sh: the PR touched nothing the hub's Windows build or tests read):
# the same gate (scripts/land-windows.sh, under bp-win-gate) on the merged main, started detached by land.sh right after the merge.
#   usage: land-postmerge.sh MAIN_SHA [PR_NUMBER] [REPO_DIR]     (land.sh runs it from a private copy, which it removes)
# It tests the merged main in a detached worktree of its own (removed at the end), and appends one line per run to the log
# (LAND_POSTMERGE_LOG, default ~/.cache/bp-land-windows/postmerge.log): "<UTC> main <sha> PR #<n> PASSED|FAILED|HELD <time>". On FAILED it
# also appends to postmerge-failures.log beside it, says it on the PR (as i2cjak: what failed, and that the owner fixes forward within the hour or
# reverts through land.sh), and runs LAND_POSTMERGE_NOTIFY (a command; the message in $LAND_POSTMERGE_MSG) when set.
# LAND_POSTMERGE_TEST replaces the gate's command (tests); LAND_GH, LAND_REPO as land.sh's.
set -u
sha=${1:?usage: land-postmerge.sh MAIN_SHA [PR] [REPO_DIR]}; pr=${2:-}; dir=${3:-$(pwd)}
log=${LAND_POSTMERGE_LOG:-$HOME/.cache/bp-land-windows/postmerge.log}; fails=$(dirname "$log")/postmerge-failures.log; mkdir -p "$(dirname "$log")"
me=$0; short=$(printf %s "$sha" | cut -c1-10)
cd "$dir" || exit 1
wt=$(mktemp -d "${TMPDIR:-/tmp}/land-postmerge-wt.XXXXXX")
cleanup() { git worktree remove --force "$wt" 2>/dev/null || rm -rf "$wt"; git worktree prune 2>/dev/null; case "$me" in */land-postmerge.??????) rm -f "$me";; esac; }
trap cleanup EXIT; trap 'exit 130' INT; trap 'exit 143' TERM; trap 'exit 129' HUP
line() { printf '%s main %s%s %s\n' "$(date -u +%FT%TZ)" "$short" "${pr:+ PR #$pr}" "$*" >> "$log"; }
git worktree add -q --detach "$wt" "$sha" 2>/dev/null || { line "HELD (could not check main out)"; exit 1; }
wlog=$(dirname "$log")/postmerge-$short.log
t0=$(date +%s); rc=0
(cd "$wt" && LAND_WIN_LOG=$wlog sh "$wt/scripts/bp-lock.sh" bp-win-gate -- sh -c "${LAND_POSTMERGE_TEST:-sh '$wt/scripts/land-windows.sh'}") > "$wlog.out" 2>&1 || rc=$?
dt=$(( $(date +%s) - t0 )); took="$((dt / 60))m$((dt % 60))s"
case "$rc" in
  0) line "PASSED $took";;
  75) line "HELD $took ($(grep -a 'land: HELD' "$wlog.out" | tail -n 1 | cut -c1-300))";;
  *)
    what=$(grep -a -A40 'not known to:' "$wlog.out" | grep -a '^    ' | head -n 20 | sed 's/^ *//' | tr '\n' ' ')
    [ -n "$what" ] || what=$(grep -a 'FAILED on' "$wlog.out" | tail -n 1)
    line "FAILED $took: ${what:-see $wlog.out}"
    msg="Post-merge Windows gate FAILED on main $short${pr:+ (after PR #$pr)}: ${what:-see $wlog.out}. The owner fixes forward within the hour, or reverts (git revert, landed through scripts/land.sh)."
    printf '%s %s\n' "$(date -u +%FT%TZ)" "$msg" >> "$fails"
    if [ -n "$pr" ]; then gh=${LAND_GH:-gh}; repo=${LAND_REPO:-$(git remote get-url origin | sed -E 's#^.*github\.com[:/]([^/]+/[^/]+)$#\1#; s#\.git$##')}
      if GH_TOKEN=$("$gh" auth token --user i2cjak 2>/dev/null) && [ -n "$GH_TOKEN" ] && export GH_TOKEN && [ "$("$gh" api user --jq .login 2>/dev/null)" = i2cjak ]; then
        "$gh" pr comment "$pr" -R "$repo" --body "$msg" >/dev/null 2>&1 || line "(could not comment on PR #$pr)"
      else line "(no i2cjak gh token: PR #$pr not told)"; fi; fi
    [ -z "${LAND_POSTMERGE_NOTIFY:-}" ] || LAND_POSTMERGE_MSG=$msg sh -c "$LAND_POSTMERGE_NOTIFY" >/dev/null 2>&1 || true;;
esac
exit 0
