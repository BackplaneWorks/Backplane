#!/bin/bash
# The auto-builder: follow gpui-next, and for each new tip run the tests, build the hub and window, and publish them (dev-cut.sh); the app's watcher (fresh-watch.sh) takes it from there.
#   scripts/gpui/next-build.sh            stay in the foreground (or run it detached: setsid nohup ... &); Ctrl-C stops it
#   scripts/gpui/next-build.sh --once     build the tip now if it is new, then leave
# It builds in a checkout of its own (default ~/Documents/i2cjak/gpui-next-build, detached at the tip), so no worker's untracked files or half edits are in a build, with one shared
# target directory per crate (default ~/.cache/gpui-next). Always the newest tip: a merge while a build runs drops that build and starts on the new tip. A tip that failed is not tried
# again; the bar says why until a newer tip lands. Env: NEXT_BRANCH (gpui-next), NEXT_WT, NEXT_TARGET, GPUI_DEV_DIR, NEXT_POLL (s, default 3), and dev-build.sh's.
set -u
if [ -n "${BACKPLANE_AGENT_OWNER:-}" ]; then exec env -u BACKPLANE_AGENT_OWNER "$0" "$@"; fi   # the hub reaps marked processes (see fresh-watch.sh)
HERE=$(cd "$(dirname "$0")" && pwd); REPO=$(git -C "$HERE" worktree list --porcelain | awk 'NR==1{print substr($0,10)}')   # the main checkout: the refs are shared by every worktree
BR=${NEXT_BRANCH:-gpui-next}; WT=${NEXT_WT:-$HOME/Documents/i2cjak/gpui-next-build}; TG=${NEXT_TARGET:-$HOME/.cache/gpui-next}
DEV=${GPUI_DEV_DIR:-$HOME/.backplane-gpui-dev}; POLL=${NEXT_POLL:-3}; ST=$DEV/BUILD
say() { printf '%s %s\n' "$(date +%H:%M:%S)" "$*"; }
status() { printf '{"phase":"%s","text":"%s","done":0,"total":0}\n' "$1" "$2" > "$ST.n" && mv -f "$ST.n" "$ST"; }
tip() { git -C "$REPO" rev-parse --verify -q "refs/heads/$BR" 2>/dev/null; }
built() { cat "$DEV/.next-built" 2>/dev/null; }
mkdir -p "$DEV" "$TG"
[ -n "$(tip)" ] || { echo "next-build: no $BR branch (git branch $BR main)" >&2; exit 1; }
[ -d "$WT/.git" ] || [ -f "$WT/.git" ] || git -C "$REPO" worktree add --detach "$WT" "$(tip)" > /dev/null 2>&1 || { echo "next-build: cannot make the checkout $WT" >&2; exit 1; }
PID=
stop_build() { [ -n "$PID" ] && kill -TERM -- "-$PID" 2>/dev/null; [ -n "$PID" ] && wait "$PID" 2>/dev/null; PID=; }
trap 'echo; say "stopped"; stop_build; rm -f "$ST" "$ST.n"; exit 0' INT TERM
say "following $BR in $REPO (built: $(built | cut -c1-10)); checkout $WT"
while :; do
  t=$(tip); b=$(built); bad=$(cat "$DEV/.next-bad" 2>/dev/null)
  if [ "$t" = "$b" ] || [ "$t" = "$bad" ]; then
    # a failed tip stays on the bar until a newer one comes
    if [ "$t" = "$bad" ] && [ -f "$DEV/.last-failure" ]; then status failed "$BR $(printf %s "$t" | cut -c1-10) failed in $(cut -c1-120 "$DEV/.last-failure")"; fi
    [ "${1:-}" = --once ] && exit 0; sleep "$POLL"; continue
  fi
  say "building $BR $(printf %s "$t" | cut -c1-10) (tests first)"
  git -C "$WT" checkout -q --detach "$t" && git -C "$WT" clean -qfd -e Cargo.lock || { say "cannot check out $t"; sleep "$POLL"; continue; }
  ( exec setsid env BUILD_ROOT="$WT" HOST_TARGET="$TG/host" GPUI_TARGET="$TG/gpui" "$HERE/dev-build.sh" --gate ) &
  PID=$!
  while kill -0 "$PID" 2>/dev/null; do
    if [ "$(tip)" != "$t" ]; then say "$BR moved: dropping the build of $(printf %s "$t" | cut -c1-10)"; stop_build; rm -f "$ST"; break; fi
    sleep 2
  done
  [ -z "$PID" ] && continue
  wait "$PID"; rc=$?; PID=
  if [ "$rc" = 0 ]; then printf '%s\n' "$t" > "$DEV/.next-built"; say "published $(printf %s "$t" | cut -c1-10)"; rm -f "$DEV/.next-bad"
  else printf '%s\n' "$t" > "$DEV/.next-bad"; say "FAILED $(printf %s "$t" | cut -c1-10): $(cat "$DEV/.last-failure" 2>/dev/null)"; fi
  [ "${1:-}" = --once ] && exit "$rc"
done
