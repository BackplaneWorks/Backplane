#!/bin/sh
# Lands the branch checked out here on main through a pull request: main takes no direct push (ruleset 27493875: pull_request with one approving review).
# usage: scripts/land.sh [--resume] [--dry-merge]        run from your lane's worktree, on your branch, with everything committed (no bp-lock.sh wrapper)
#   1-3, OUTSIDE any lock (many lanes gate at once): git fetch origin main; git rebase origin/main (a conflict stops here: rebase by hand, then land
#      again); the authors (scripts/check-author.sh, and i2cjak <build@amemb.com> for the lane's own commits); free space (LAND_MIN_FREE_GB, 10); the gate:
#      scripts/land-gate.sh (LAND_TEST overrides the command), the PR's crates compiled and their touched unit tests, on a Boat box; skipped when this
#      exact tree passed it already (<git-dir>/land-tested). No Windows, macOS, e2e, screenshot or KiCad embed suite runs at a merge: they are the
#      release-candidate pass (docs/release-playbook.md).
#   4-5, INSIDE the land lock, which land.sh takes itself (scripts/bp-lock.sh land) once the gate is green and holds for seconds: git fetch origin
#      main; git rebase origin/main, with no gate again (the PR's own changes passed it, and main is main); a conflict releases the lock and stops with
#      a line (resolve, then land again: the gate runs again outside the lock). Then the branch goes to origin (--force-with-lease), its pull request is
#      opened or reused (gh as i2cjak: GH_TOKEN from `gh auth token --user i2cjak`, never printed or stored), and the PR's state decides:
#        mergeable        gh pr merge --rebase --match-head-commit <the rebased head> (the merged tree is that head), wait for MERGED, then fast-forward
#                         local main and print the main hash;                                                                                         exit 0
#        GitHub refuses, or BEHIND/DIRTY (main moved in that instant): rebase and try again, still in the lock, up to LAND_TRIES (3)
#        waiting for an approving review     "land: PR #N open, gate green on <sha>, waiting for an approving review"; then scripts/land.sh --resume   exit 3
#      A land.sh already inside `scripts/bp-lock.sh land -- ...` (the old way) does not take the lock again (BP_LOCK_NAME=land): drop the wrapper.
#   Main is never gated as a whole at a merge: main-build.sh keeps building main, and when main breaks, its owner fixes forward at once or reverts.
#   --resume   the branch must already have an open PR
#   --dry-merge   (or LAND_DRY_MERGE=1) everything up to the merge, which is only said ("would merge"); nothing is merged or fast-forwarded
# Env: LAND_GH (the gh program), LAND_REPO (owner/name; default from origin), LAND_TRIES, LAND_TEST, LAND_MIN_FREE_GB, LAND_POLL_S (seconds between PR state reads, 2).
# Any step that cannot be done fails loudly with "land: FAILED ..." and a non-zero exit; it never leaves local main ahead of origin.
set -eu
# sh reads a script as it runs: an edit, a rebase or this script's own fast-forward of a checkout would change what a running land reads next.
# So it runs from a private copy (and `here` stays the real scripts dir).
if [ -z "${LAND_COPY:-}" ]; then
  real=$(cd "$(dirname "$0")" && pwd)
  copy=$(mktemp "${TMPDIR:-/tmp}/land-run.XXXXXX") && cp "$0" "$copy" || { echo "land: FAILED: could not copy $0 to a temporary file" >&2; exit 1; }
  LAND_COPY=$copy LAND_HERE=$real exec sh "$copy" "$@"
fi
trap 'rm -f "$LAND_COPY"' EXIT
cd "$(git rev-parse --show-toplevel)"
here=$LAND_HERE
die() { echo "land: FAILED: $*" >&2; exit 1; }
say() { echo "land: $*" >&2; }
# A full disk fails a build or a dozen unrelated tests ("No space left on device"); say so first, plainly
need_gb=${LAND_MIN_FREE_GB:-10}
free_check() {
  for d in . "${TMPDIR:-/tmp}" "$HOME/.cache"; do
    [ -d "$d" ] || continue
    set -- $(df -Pk "$d" 2>/dev/null | awk 'NR == 2 { print $4, $6 }')
    [ -n "${1:-}" ] || continue
    gb=$(( $1 / 1048576 ))
    [ "$gb" -ge "$need_gb" ] || die "only $gb GB free on $2, need $need_gb: clean up (old cargo targets, test scratch in ~/.cache/bp-tmp)"
  done
}
resume=0; dry=${LAND_DRY_MERGE:-0}
for a in "$@"; do case "$a" in --resume) resume=1;; --dry-merge) dry=1;; *) die "unknown argument $a (usage: scripts/land.sh [--resume] [--dry-merge])";; esac; done
br=$(git symbolic-ref --short -q HEAD) || die "HEAD is detached; land from your lane's branch"
[ "$br" != main ] || die "you are on main; commit on your lane's branch (from origin/main) and land that"
[ -z "$(git status --porcelain --untracked-files=no)" ] || die "uncommitted changes in $(pwd); commit them first"
git remote get-url origin > /dev/null 2>&1 || die "no origin remote"
mark=$(git rev-parse --git-dir)/land-tested
[ -z "${LAND_WINDOWS:-}" ] || say "LAND_WINDOWS is ignored: no Windows gate runs at a merge any more (the release-candidate pass runs it: docs/release-playbook.md)"
tries=${LAND_TRIES:-3}; n=0; poll=${LAND_POLL_S:-2}
gh_prog=${LAND_GH:-gh}
repo=${LAND_REPO:-$(git remote get-url origin | sed -E 's#^.*github\.com[:/]([^/]+/[^/]+)$#\1#; s#\.git$##')}
# gh as i2cjak, whatever account is active here (never switch it): the token is read into the environment of each gh call only
ghi() { GH_TOKEN=$("$gh_prog" auth token --user i2cjak 2>/dev/null) || GH_TOKEN=; [ -n "$GH_TOKEN" ] || die "no gh login for i2cjak (gh auth login as i2cjak; the active account is not used)"; export GH_TOKEN
  # a write (a pull request opened or merged) only as i2cjak: ask GitHub whose token this is, in this very environment, before it runs
  case "$1 $2" in "pr create"|"pr merge"|"pr close"|"pr comment"|"pr edit")
    who=$("$gh_prog" api user --jq .login 2>/dev/null) || who=
    [ "$who" = i2cjak ] || die "refusing 'gh $1 $2': the token it would use is ${who:-unknown} on GitHub, not i2cjak (gh auth token --user i2cjak must be i2cjak's; never gh auth switch or login)";; esac
  "$gh_prog" "$@"; }
"$gh_prog" auth token --user i2cjak >/dev/null 2>&1 || die "no gh login for i2cjak (gh auth login as i2cjak; the active account is not used)"
pr_number() { ghi pr list -R "$repo" --head "$br" --state open --json number -q '.[0].number' 2>/dev/null || true; }
# A stale copy of this script (a worktree made before main's land.sh changed) must not run: an older land.sh pushed straight to main. Refused when
# origin/main's scripts/land.sh differs from this one AND this one is a version main has had (a copy ahead of main, a change being landed, is fine).
git fetch -q origin main || die "git fetch origin main failed (offline?); nothing pushed"
if mine=$(git hash-object "$LAND_COPY") && cur=$(git rev-parse -q --verify origin/main:scripts/land.sh) && [ "$mine" != "$cur" ]; then
  for c in $(git log --format=%H origin/main -- scripts/land.sh); do
    [ "$(git rev-parse -q --verify "$c:scripts/land.sh" 2>/dev/null)" = "$mine" ] && die "this scripts/land.sh is older than origin/main's: rebase on origin/main (git rebase origin/main), then run it again"
  done
fi
if [ "$resume" = 1 ]; then [ -n "$(pr_number)" ] || die "--resume: no open pull request for $br on $repo (run scripts/land.sh)"; fi
phase=${LAND_PHASE:-gate}
# 1-3 the gate, outside any lock (lanes gate side by side): rebased on main as it is now, checked, gated
rebase_or_stop() { # rebase_or_stop WHEN: onto origin/main as it is now; a conflict stops the land (the lock, if held, goes with this process)
  git fetch -q origin main || die "git fetch origin main failed (offline?); nothing pushed"
  if ! git rebase -q origin/main; then git rebase --abort 2>/dev/null || true
    die "$br conflicts with main $(git rev-parse --short origin/main) ($1): resolve it (git rebase origin/main), then land again (the gate runs again outside the lock); nothing pushed"; fi
  [ -n "$(git rev-list -1 origin/main..HEAD)" ] || { say "nothing to land: $br is already on origin/main"; exit 0; }
  "$here/check-author.sh" origin/main..HEAD || die "a commit has the wrong author or committer (see above)"
  # the lane's own commits are always i2cjak <build@amemb.com>, author and committer (check-author.sh also lets the other contributor's through; a land does not)
  odd=$(git log --format='%h %an <%ae> / %cn <%ce>' origin/main..HEAD | grep -v -x '[0-9a-f]* i2cjak <build@amemb.com> / i2cjak <build@amemb.com>' || true)
  [ -z "$odd" ] || die "commits not made as i2cjak <build@amemb.com>:
$odd
fix: git config user.name i2cjak && git config user.email build@amemb.com (this repo's config), then re-author them: git rebase --exec 'git commit --amend --no-edit --reset-author' origin/main"; }
if [ "$phase" = gate ]; then
  rebase_or_stop "before the gate"
  free_check
  tree=$(git rev-parse 'HEAD^{tree}'); gated=$(git rev-parse HEAD)   # what the gate sees: the merge phase lands this head, nothing committed after
  if [ "$(cat "$mark" 2>/dev/null)" = "$tree" ]; then say "tests already passed on this tree ($(printf %s "$tree" | cut -c1-10))"
  else
    say "testing $(git rev-parse --short HEAD) on origin/main $(git rev-parse --short origin/main) (no lock held: other lanes gate at the same time)"
    g0=$(date +%s); env -u LAND_COPY -u LAND_HERE -u LAND_PHASE -u LAND_GATED -u LAND_L0 sh -c "${LAND_TEST:-bash '$here/land-gate.sh'}" || die "the gate failed (above); nothing pushed"
    say "gate passed in $(( $(date +%s) - g0 ))s"
    printf '%s\n' "$tree" > "$mark"
  fi
  # 4-5 under the land lock, held for seconds: this same run (its private copy) again, as the merge phase. A land.sh already run inside
  # `scripts/bp-lock.sh land -- ...` (the old way) holds it: it goes on here, and says the wrapper is not needed
  if [ "${BP_LOCK_NAME:-}" = land ]; then LAND_GATED=$gated; say "already inside the land lock (scripts/bp-lock.sh land -- scripts/land.sh): drop the wrapper, land.sh takes the lock itself, for the merge alone"
  else
    say "gate green: taking the land lock for the rebase, push and merge (seconds once it is ours)"
    l0=$(date +%s); export LAND_PHASE=merge LAND_GATED=$gated LAND_L0=$l0
    "$here/bp-lock.sh" land -- sh "$LAND_COPY" "$@"; exit $?   # a child, not exec: this shell's trap still removes the private copy
  fi
fi
[ -z "${LAND_L0:-}" ] || say "the land lock is ours (waited $(( $(date +%s) - LAND_L0 ))s)"
# only what the gate passed lands: a commit made in this worktree since (while it gated, or waited for the lock) needs a gate of its own
[ -z "${LAND_GATED:-}" ] || [ "$(git rev-parse HEAD)" = "$LAND_GATED" ] || die "$br changed since the gate (its head is $(git rev-parse --short HEAD), the gate passed $(printf %s "$LAND_GATED" | cut -c1-10)): land again; nothing pushed"
h0=$(date +%s)
# inside the lock: rebased onto main as it is now (no gate again: the PR's own changes passed it; main is main), pushed, merged on that head
while :; do
  n=$((n + 1))
  rebase_or_stop "in the land lock"
  sha=$(git rev-parse HEAD)
  # main moved since the gate: the catalogs generated from i18n/*.msg again (a second; a .msg change of main's merges cleanly into a branch whose
  # catalogs were made without it: Nov 20, 5,890 lines of Msg.kt), on the rebased tree
  if [ "$sha" != "${LAND_GATED:-$sha}" ] && [ -f "$(git rev-parse --show-toplevel)/scripts/i18n-gate.sh" ]; then
    bash "$(git rev-parse --show-toplevel)/scripts/i18n-gate.sh" > /dev/null || die "after the rebase onto main in the land lock, $br's i18n catalogs are stale (above): run the generator it names, commit, and land again; nothing pushed"
  fi
  # the branch: forced with a lease (a rebase rewrote it; nothing else pushes to a lane's branch)
  git fetch -q origin "$br" 2>/dev/null || true
  [ "$br" != main ] && [ "$br" != master ] || die "refusing to push $br: land.sh never pushes to main, only a lane's branch (main takes the pull request's merge)"
  git push -q --force-with-lease origin "HEAD:refs/heads/$br" || die "could not push $br to origin (see above); nothing landed"
  num=$(pr_number)
  if [ -z "$num" ]; then
    url=$(ghi pr create -R "$repo" --fill --base main --head "$br") || die "could not open the pull request for $br"
    num=$(pr_number); [ -n "$num" ] || num=${url##*/}
    say "opened PR #$num $url"
  else say "PR #$num is open for $br"; fi
  # its state: GitHub works out mergeability a moment after a push, and its head must be the sha pushed
  st=; for _ in $(seq 60); do
    st=$(ghi pr view "$num" -R "$repo" --json state,mergeStateStatus,reviewDecision,headRefOid,mergeable -q '[.state,.mergeStateStatus,(if (.reviewDecision // "") == "" then "NONE" else .reviewDecision end),.headRefOid,.mergeable] | join(" ")' 2>/dev/null) || st=
    set -- $st; pstate=${1:-}; merge_st=${2:-}; review=${3:-}; head=${4:-}; mergeable=${5:-}
    [ "$pstate" = MERGED ] && break
    [ -n "$pstate" ] && [ "$head" = "$sha" ] && [ "$mergeable" != UNKNOWN ] && [ "$merge_st" != UNKNOWN ] && break
    sleep "$poll"
  done
  [ -n "${pstate:-}" ] || die "could not read the state of PR #$num (gh failed); the branch is pushed"
  [ "$pstate" = MERGED ] && { say "PR #$num is already merged"; break; }
  [ "$pstate" = OPEN ] || die "PR #$num is $pstate, not open; reopen it or land again from a new branch"
  [ "$head" = "$sha" ] || die "PR #$num's head is ${head:-unknown}, not the pushed $sha (GitHub has not taken the push); run scripts/land.sh --resume"
  case "$merge_st" in
    BEHIND|DIRTY)
      [ "$n" -lt "$tries" ] || die "PR #$num is still $merge_st after $n tries (main keeps moving, or it conflicts); nothing merged"
      say "main moved ($merge_st): rebasing again, in the lock ($((n + 1))/$tries)"; continue;;
    CLEAN|UNSTABLE|HAS_HOOKS) ;;
    *) # BLOCKED, DRAFT: what blocks it is said, and a lane stops here instead of retrying
      case "$review" in
        REVIEW_REQUIRED) say "PR #$num open, gate green on $(git rev-parse --short "$sha"), waiting for an approving review"; exit 3;;
        NONE) say "PR #$num open, gate green on $(git rev-parse --short "$sha"), blocked by a rule with no review asked for ($merge_st): not merged (gh pr view $num names nothing more; see the ruleset)"; exit 3;;
        CHANGES_REQUESTED) say "PR #$num open, gate green on $(git rev-parse --short "$sha"), changes requested by a reviewer (answer them, push, then land again)"; exit 3;;
        *) say "PR #$num open, gate green on $(git rev-parse --short "$sha"), blocked ($merge_st, review $review): a required check or another rule has not passed"; exit 3;;
      esac;;
  esac
  if [ "$dry" = 1 ]; then say "dry run: PR #$num is mergeable ($merge_st) at $(git rev-parse --short "$sha"); would run: gh pr merge $num --rebase --match-head-commit $sha, wait for MERGED, then fast-forward local main"; exit 0; fi
  # the merged tree is the rebased head pushed above: --match-head-commit; main moved in the instant between (GitHub refuses): rebase again, in the lock
  if ! ghi pr merge "$num" -R "$repo" --rebase --match-head-commit "$sha"; then
    [ "$n" -lt "$tries" ] || die "GitHub refused the merge of PR #$num after $n tries (see above); left open"
    say "GitHub refused the merge of PR #$num (main moved?): rebasing again, in the lock ($((n + 1))/$tries)"; continue
  fi
  for _ in $(seq 90); do
    pstate=$(ghi pr view "$num" -R "$repo" --json state -q .state 2>/dev/null || true); [ "$pstate" = MERGED ] && break; sleep "$poll"
  done
  [ "$pstate" = MERGED ] || die "PR #$num was merged but GitHub does not say MERGED yet; run scripts/land.sh --resume"
  break
done
say "the land lock was held $(( $(date +%s) - h0 ))s for the merge"
# After the merge nothing may turn the result non-zero but a failed fast-forward of local main (and that says the merge is done).
if ! git fetch -q origin main; then say "the merge of ${num:+PR #$num }is DONE, but git fetch origin main failed here; local main is not updated (run: git fetch origin main, then fast-forward main)"; exit 0; fi
# local main follows what GitHub made of the commits (a rebase merge rewrites them: the branch's own shas are not main's): fast-forward it in the checkout that has it
tip=$(git rev-parse origin/main)
mainwt=$(git worktree list --porcelain | awk '/^worktree /{w=substr($0,10)} /^branch refs\/heads\/main$/{print w}')
if git merge-base --is-ancestor main "$tip" 2>/dev/null; then
  if [ -n "$mainwt" ]; then git -C "$mainwt" merge -q --ff-only "$tip" || { say "the merge is DONE (main is $(git rev-parse --short "$tip") on origin), but local main in $mainwt cannot fast-forward (its working tree is in the way); run: git -C $mainwt merge --ff-only origin/main"; exit 1; }
  else git update-ref refs/heads/main "$tip"; fi
else say "the merge is DONE (main is $(git rev-parse --short "$tip") on origin), but local main has commits that are not on origin/main ($(git rev-list --count "$tip"..main)); land them from a branch (scripts/land.sh), never merge into main"; exit 1
fi
say "landed $br${num:+ (PR #$num)}; main is $(git rev-parse --short "$tip"); scripts/main-build.sh status shows the build"
exit 0
