#!/bin/bash
# A haiku on what a build adds: the commits from FROM (the tip built before) to TO, summed up by Haiku.
#   scripts/build-haiku.sh REPO FROM TO     prints three lines, or nothing (no FROM, no commits, no claude, too slow)
# Called by dev-build.sh while the build runs; it never fails the build. Env: HAIKU_MODEL (default claude-haiku-5-5), HAIKU_SECS (default 90), HAIKU_CLAUDE (the claude program).
set -u
REPO=${1:-}; FROM=${2:-}; TO=${3:-}
[ -n "$REPO" ] && [ -n "$FROM" ] && [ -n "$TO" ] && [ "$FROM" != "$TO" ] || exit 0
git -C "$REPO" cat-file -e "$FROM^{commit}" 2>/dev/null || exit 0
log=$(git -C "$REPO" log --no-merges --format='- %s' "$FROM..$TO" 2>/dev/null | head -40 | cut -c1-200)
[ -n "$log" ] || exit 0
C=${HAIKU_CLAUDE:-claude}; command -v "$C" >/dev/null 2>&1 || exit 0
out=$(printf 'These commits make up a new build of Backplane:\n%s\n\nWrite one haiku (5-7-5, three lines) about what this build adds. Plain words, no title, no quotes, nothing else.\n' "$log" |
  timeout "${HAIKU_SECS:-90}" env -u BACKPLANE_AGENT_OWNER "$C" -p --model "${HAIKU_MODEL:-claude-haiku-5-5}" --tools "" --disable-slash-commands --no-session-persistence --strict-mcp-config 2>/dev/null) || exit 0
# three non-empty lines, short; anything else is not a haiku
out=$(printf '%s\n' "$out" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//' | grep -v '^$' | tr -d '"\\')
[ "$(printf '%s\n' "$out" | wc -l)" = 3 ] && [ "$(printf %s "$out" | wc -c)" -le 160 ] && printf '%s\n' "$out"
exit 0
