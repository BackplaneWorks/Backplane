#!/bin/sh
# Proposed FF-only exact-candidate integration. Call only while holding the
# existing merge-main lock. No merge-result tree may differ from the measured
# head. The remote lease supplies the base guard the GitHub merge API lacks.
# Repository protection/trusted checks are still needed against other writers.
set -eu
cd "$(dirname "$0")/.."
[ "$#" = 5 ] || { echo 'usage: perf-merge.sh PR EVIDENCE BINARY FIXTURE RECEIPT' >&2; exit 1; }
pr=$1
shift
[ "$(gh api user --jq .login)" = i2cjak ] || { echo 'i2cjak GitHub identity required' >&2; exit 1; }
case $(git remote get-url origin) in
  https://github.com/i2cjak/Backplane|https://github.com/i2cjak/Backplane.git|git@github.com:i2cjak/Backplane.git) ;;
  *) echo 'unexpected repository' >&2; exit 1 ;;
esac
[ -z "$(git status --porcelain)" ] || { echo 'candidate checkout must be clean' >&2; exit 1; }
head=$(git rev-parse HEAD)
base=$(gh api "repos/i2cjak/Backplane/pulls/$pr" --jq .base.sha)
[ "$(gh api "repos/i2cjak/Backplane/pulls/$pr" --jq .base.ref)" = main ] || exit 1
[ "$(gh api "repos/i2cjak/Backplane/pulls/$pr" --jq .base.repo.full_name)" = i2cjak/Backplane ] || exit 1
[ "$(gh pr view "$pr" --repo i2cjak/Backplane --json headRefOid --jq .headRefOid)" = "$head" ] || exit 1
git cat-file -e "$base^{commit}" 2>/dev/null || git fetch --no-tags origin "$base"
git merge-base --is-ancestor "$base" "$head" || { echo 'rebase onto the actual base and measure the combined candidate first' >&2; exit 1; }
# This wrapper currently verifies Linux window evidence only. It must not
# authorize a phone/browser/Windows renderer change on a Linux-only result.
unsupported=$(git diff --name-only "$base" "$head" -- mobile src/mobile src/web tools/browser tools/cef src/app/effects/win32.inc)
[ -z "$unsupported" ] || { echo 'additional changed-client performance evidence is required; this Linux-only wrapper cannot merge that scope' >&2; exit 1; }
bun scripts/perf-verify.ts "$@" linux-native long-history timeline subagents rooms bots status
# Moving main or head invalidates the prepared integration, even if clean.
[ "$(gh api repos/i2cjak/Backplane/git/ref/heads/main --jq .object.sha)" = "$base" ] || exit 1
[ "$(gh pr view "$pr" --repo i2cjak/Backplane --json headRefOid --jq .headRefOid)" = "$head" ] || exit 1
# This cannot replace another writer's update: lease mismatch rejects it. Since
# base is an ancestor of head, the only permitted update is a fast-forward to
# exactly the already-built commit/tree. No new merge commit is manufactured.
git -c push.followTags=false push --porcelain --force-with-lease="refs/heads/main:$base" origin "$head:refs/heads/main"
echo "main updated to exact measured commit $head; PR $pr bookkeeping must name this same commit"
