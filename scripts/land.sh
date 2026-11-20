#!/bin/sh
# Lands the branch checked out here on main through a pull request: main takes no direct push (ruleset 27493875: pull_request with one approving review).
# usage: scripts/land.sh [--resume] [--dry-merge]        run from your lane's worktree, on your branch, with everything committed
#   1. git fetch origin main; git rebase origin/main (a conflict stops here: the rebase is aborted and nothing moved; rebase by hand, then land again);
#   2. checks origin/main..HEAD (scripts/check-author.sh); free space: the build and the tests' scratch each need LAND_MIN_FREE_GB (10);
#   3. the gate: scripts/dev-build.sh --gate --no-cut (LAND_TEST overrides the command), on this rebased tree, BEFORE anything is pushed; skipped only when this exact
#      tree passed it already (recorded in <git-dir>/land-tested);
#   3b. the Windows gate, on the same tree, also BEFORE anything is pushed: scripts/land-windows.sh (LAND_WIN_TEST overrides the command) under the bp-win-gate lock
#      (scripts/bp-lock.sh; landings queue, one gate on the VMs at a time): the hub's tests as ci.yml's windows-latest job runs them (bp-sh, then nextest with
#      ci-hub-windows-filter.sh and --retries 1), on bp-win, else bp-winbuild; the tree goes by git push over ssh to the VM, never to GitHub. Its time is said.
#      Run only when what the hub's Windows tests build from or read ($win_paths below) changed against origin/main, and skipped when this tree passed it already
#      (<git-dir>/land-tested-windows). Failed: "land: FAILED", nothing pushed. No VM up (or each within 45 min of its daily auto-shutdown): one "land: HELD" line
#      naming each VM tried and why, exit 4, nothing pushed. LAND_WINDOWS=skip skips it, on a person's or lead's word only: said in the output and in the PR body.
#   4. the branch goes to origin (git push --force-with-lease: it is only ever this lane's gated tip), and its pull request is opened or the open one reused
#      (gh pr create --fill --base main); gh always runs as i2cjak (GH_TOKEN from `gh auth token --user i2cjak`, never printed or stored), whatever account gh has active;
#   5. the PR's state decides:
#        mergeable        gh pr merge --rebase --match-head-commit <the gated sha> (linear history, authors kept), wait for MERGED, then fetch and fast-forward local main
#                         and print the main hash;                                                                                                   exit 0
#        waiting for an approving review     "land: PR #N open, gate green on <sha>, waiting for an approving review"                                exit 3
#                         (stop and report the PR number; when it is approved, run scripts/land.sh --resume: the gate is not run again while this tree is the tested one)
#        main moved since the gate (origin/main is not the base the gate ran on, checked right before the merge)  back to 1 (#1374)
#        behind or conflicting (main moved)  back to 1 (rebase, gate again, force-with-lease push), up to LAND_TRIES (3)
#   --resume   the branch must already have an open PR: the same run, so an approved PR is merged without a new gate when its head is still the tested tree and main has not moved
#   --dry-merge   (or LAND_DRY_MERGE=1) everything up to the merge, which is only said ("would merge"); nothing is merged or fast-forwarded
# Env: LAND_GH (the gh program), LAND_REPO (owner/name; default from origin), LAND_TRIES, LAND_TEST, LAND_WINDOWS, LAND_WIN_TEST (and land-windows.sh's LAND_WIN_*), LAND_MIN_FREE_GB, LAND_POLL_S (seconds between PR state reads, 2).
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
mark=$(git rev-parse --git-dir)/land-tested; wmark=$(git rev-parse --git-dir)/land-tested-windows
win=${LAND_WINDOWS:-}; win_note=
case "$win" in ''|skip) ;; *) die "LAND_WINDOWS=$win: only LAND_WINDOWS=skip (on a person's or lead's word) or unset";; esac
# what the hub's Windows tests build from or read (native/host, and the files its tests open by path); a change elsewhere does not run the Windows gate
win_paths='native/host native/gpui/Cargo.toml native/gpui/build.rs native/gpui/src/install.rs native/guard/Cargo.toml native/guard/build.rs skills assets deploy docs/reporting files/scripts test/agents test/bots test/commands test/files test/fixtures test/google test/hub test/mobile scripts/ci-hub-windows-filter.sh scripts/kistack.py scripts/codex-apikey.sh .github/workflows/ci.yml'
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
while :; do
  n=$((n + 1))
  git fetch -q origin main || die "git fetch origin main failed (offline?); nothing pushed"
  if ! git rebase -q origin/main; then git rebase --abort 2>/dev/null || true; die "git rebase origin/main conflicts; resolve it by hand (git rebase origin/main), then land again"; fi
  base=$(git rev-parse origin/main) # the main this tree is gated on: it must still be origin/main at the merge (#1374)
  [ -n "$(git rev-list -1 origin/main..HEAD)" ] || { say "nothing to land: $br is already on origin/main"; break; }
  "$here/check-author.sh" origin/main..HEAD || die "a commit has the wrong author or committer (see above)"
  # the lane's own commits are always i2cjak <build@amemb.com>, author and committer (check-author.sh also lets the other contributor's through; a land does not)
  odd=$(git log --format='%h %an <%ae> / %cn <%ce>' origin/main..HEAD | grep -v -x '[0-9a-f]* i2cjak <build@amemb.com> / i2cjak <build@amemb.com>' || true)
  [ -z "$odd" ] || die "commits not made as i2cjak <build@amemb.com>:
$odd
fix: git config user.name i2cjak && git config user.email build@amemb.com (this repo's config), then re-author them: git rebase --exec 'git commit --amend --no-edit --reset-author' origin/main"
  free_check
  tree=$(git rev-parse 'HEAD^{tree}')
  if [ "$(cat "$mark" 2>/dev/null)" = "$tree" ]; then say "tests already passed on this tree ($(printf %s "$tree" | cut -c1-10))"
  else
    say "testing $(git rev-parse --short HEAD) on origin/main $(git rev-parse --short origin/main)"
    sh -c "${LAND_TEST:-scripts/dev-build.sh --gate --no-cut}" || die "tests failed; nothing pushed"
    printf '%s\n' "$tree" > "$mark"
  fi
  # the Windows gate: the hub's Windows tests on this same tree on bp-win (else bp-winbuild), under the bp-win-gate lock, before anything is pushed
  if [ "$win" = skip ]; then win_note=1; say "LAND_WINDOWS=skip: the Windows gate is NOT run on this tree (only on a person's or lead's word); the PR body says so"
  elif git diff --quiet origin/main HEAD -- $win_paths; then say "Windows gate skipped: nothing the hub's Windows tests build from or read changed against origin/main"
  elif [ "$(cat "$wmark" 2>/dev/null)" = "$tree" ]; then say "Windows tests already passed on this tree ($(printf %s "$tree" | cut -c1-10))"
  else
    say "Windows gate: the hub's Windows tests on $(git rev-parse --short HEAD) (lock bp-win-gate; another landing's gate runs first)"
    w0=$(date +%s); rc=0
    "$here/bp-lock.sh" bp-win-gate -- sh -c "${LAND_WIN_TEST:-sh '$here/land-windows.sh'}" || rc=$?
    w=$(( $(date +%s) - w0 )); took="$((w / 60))m$((w % 60))s"
    case "$rc" in
      0) printf '%s\n' "$tree" > "$wmark"; say "Windows gate passed: $took in all, the lock's wait included";;
      75) say "held after $took: no Windows VM could run the gate (the line above names each); nothing pushed or merged"; exit 4;;
      *) die "the Windows tests failed ($took; the output: $(git rev-parse --git-dir)/land-windows.log); nothing pushed";;
    esac
  fi
  sha=$(git rev-parse HEAD)
  # the branch, only after a green gate: forced with a lease (a rebase rewrote it; nothing else pushes to a lane's branch)
  git fetch -q origin "$br" 2>/dev/null || true
  [ "$br" != main ] && [ "$br" != master ] || die "refusing to push $br: land.sh never pushes to main, only a lane's branch (main takes the pull request's merge)"
  git push -q --force-with-lease origin "HEAD:refs/heads/$br" || die "could not push $br to origin (see above); nothing landed"
  num=$(pr_number)
  if [ -z "$num" ]; then
    url=$(ghi pr create -R "$repo" --fill --base main --head "$br") || die "could not open the pull request for $br"
    num=$(pr_number); [ -n "$num" ] || num=${url##*/}
    say "opened PR #$num $url"
  else say "PR #$num is open for $br"; fi
  if [ -n "$win_note" ]; then
    body=$(ghi pr view "$num" -R "$repo" --json body -q .body) || die "could not read PR #$num's body to note LAND_WINDOWS=skip"
    case "$body" in *"LAND_WINDOWS=skip"*) ;; *) ghi pr edit "$num" -R "$repo" --body "$body

**Windows gate skipped (LAND_WINDOWS=skip)** on $(git rev-parse --short "$sha"): the hub's Windows tests did not run before this landed." >/dev/null || die "could not note LAND_WINDOWS=skip in PR #$num's body";; esac
  fi
  # its state: GitHub works out mergeability a moment after a push, and its head must be the sha that was gated
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
  [ "$head" = "$sha" ] || die "PR #$num's head is ${head:-unknown}, not the gated $sha (GitHub has not taken the push); run scripts/land.sh --resume"
  case "$merge_st" in
    BEHIND|DIRTY)
      [ "$n" -lt "$tries" ] || die "PR #$num is still $merge_st after $n tries (main keeps moving, or it conflicts); nothing merged"
      say "main moved ($merge_st): fetching, rebasing and testing again ($((n + 1))/$tries)"; continue;;
    CLEAN|UNSTABLE|HAS_HOOKS) ;;
    *) # BLOCKED, DRAFT: what blocks it is said, and a lane stops here instead of retrying
      case "$review" in
        REVIEW_REQUIRED) say "PR #$num open, gate green on $(git rev-parse --short "$sha"), waiting for an approving review"; exit 3;;
        NONE) say "PR #$num open, gate green on $(git rev-parse --short "$sha"), blocked by a rule with no review asked for ($merge_st): not merged (gh pr view $num names nothing more; see the ruleset)"; exit 3;;
        CHANGES_REQUESTED) say "PR #$num open, gate green on $(git rev-parse --short "$sha"), changes requested by a reviewer (answer them, push, then land again)"; exit 3;;
        *) say "PR #$num open, gate green on $(git rev-parse --short "$sha"), blocked ($merge_st, review $review): a required check or another rule has not passed"; exit 3;;
      esac;;
  esac
  # main must still be the base the gate ran on: GitHub's rebase merge would put the PR on a newer main, a tree no gate ran (#1374)
  git fetch -q origin main || die "git fetch origin main failed before the merge (offline?); PR #$num is left open, gated on $(git rev-parse --short "$base")"
  if [ "$(git rev-parse origin/main)" != "$base" ]; then
    [ "$n" -lt "$tries" ] || die "main keeps moving: it moved again after the gate on try $n of $tries ($(git rev-parse --short "$base") -> $(git rev-parse --short origin/main)); PR #$num left open, nothing merged"
    say "main moved after the gate ($(git rev-parse --short "$base") -> $(git rev-parse --short origin/main)): rebasing and testing again ($((n + 1))/$tries)"; continue
  fi
  if [ "$dry" = 1 ]; then say "dry run: PR #$num is mergeable ($merge_st) at $(git rev-parse --short "$sha"); would run: gh pr merge $num --rebase --match-head-commit $sha, wait for MERGED, then fast-forward local main"; exit 0; fi
  if ! ghi pr merge "$num" -R "$repo" --rebase --match-head-commit "$sha"; then
    git fetch -q origin main || true
    [ "$n" -lt "$tries" ] || die "GitHub refused the merge of PR #$num (see above); left open"
    say "GitHub refused the merge of PR #$num (main moved?): fetching, rebasing and testing again ($((n + 1))/$tries)"; continue
  fi
  for _ in $(seq 90); do
    pstate=$(ghi pr view "$num" -R "$repo" --json state -q .state 2>/dev/null || true); [ "$pstate" = MERGED ] && break; sleep "$poll"
  done
  [ "$pstate" = MERGED ] || die "PR #$num was merged but GitHub does not say MERGED yet; run scripts/land.sh --resume"
  break
done
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
