#!/bin/bash
# The auto-builder: follow `main`, and for each new tip run the tests, build the hub and window, and publish them (dev-cut.sh); the app's watcher (fresh-watch.sh) takes it from there.
#   scripts/main-build.sh            stay in the foreground; Ctrl-C stops it
#   scripts/main-build.sh --once     build the tip now if it is new, then leave
#   scripts/main-build.sh ensure     start it detached unless one is already running (what the main hook calls; see install-hooks.sh)
#   scripts/main-build.sh status     running or not, the tip it built last, the tip that failed
# It builds in a checkout of its own (default ~/Documents/i2cjak/main-build, detached at the tip), so no worker's untracked files or half edits are in a build, with one shared
# target directory per crate (default ~/.cache/main-build). Always the newest tip, never a dropped build: a merge while a build runs lets it finish (and publish if it passed), then the newest tip is built. A tip that failed is not
# tried again; the bar says why until a newer tip lands. Env: MAIN_BRANCH (main), MAIN_WT, MAIN_TARGET, GPUI_DEV_DIR, MAIN_POLL (s, default 3), MAIN_MIN_FREE_GB (20), and dev-build.sh's.
set -u
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX GIT_COMMON_DIR GIT_OBJECT_DIRECTORY GIT_QUARANTINE_PATH
if [ -n "${BACKPLANE_AGENT_OWNER:-}" ]; then exec env -u BACKPLANE_AGENT_OWNER "$0" "$@"; fi   # the hub reaps marked processes (see fresh-watch.sh)
HERE=$(cd "$(dirname "$0")" && pwd); REPO=$(git -C "$HERE" worktree list --porcelain | awk 'NR==1{print substr($0,10)}')   # the main checkout: the refs are shared by every worktree
BR=${MAIN_BRANCH:-main}; WT=${MAIN_WT:-$HOME/Documents/i2cjak/main-build}; TG=${MAIN_TARGET:-$HOME/.cache/main-build}
DEV=${GPUI_DEV_DIR:-$HOME/.backplane-gpui-dev}; POLL=${MAIN_POLL:-3}; ST=$DEV/BUILD; FREE=${MAIN_MIN_FREE_GB:-20}; PIDF=$DEV/.main-build.pid
say() { printf '%s %s\n' "$(date +%H:%M:%S)" "$*"; }
status() { printf '{"phase":"%s","text":"%s","done":0,"total":0}\n' "$1" "$2" > "$ST.n" && mv -f "$ST.n" "$ST"; }
tip() { git -C "$REPO" rev-parse --verify -q "refs/heads/$BR" 2>/dev/null; }
built() { cat "$DEV/.main-built" 2>/dev/null; }
running() { [ -f "$PIDF" ] && p=$(cat "$PIDF" 2>/dev/null) && [ -n "$p" ] && kill -0 "$p" 2>/dev/null && tr '\0' ' ' < "/proc/$p/cmdline" 2>/dev/null | grep -q main-build; }
mkdir -p "$DEV" "$TG"
case "${1:-}" in
  ensure) running && exit 0
          LOGF=$DEV/main-build.log
          setsid nohup "$HERE/main-build.sh" >> "$LOGF" 2>&1 < /dev/null & exit 0;;
  status) if running; then echo "running (pid $(cat "$PIDF"))"; else echo "not running"; fi
          echo "main:  $(tip | cut -c1-10)"; echo "built: $(built | cut -c1-10)"; echo "bad:   $(cat "$DEV/.main-bad" 2>/dev/null | cut -c1-10)"; exit 0;;
esac
running && { echo "main-build: already running (pid $(cat "$PIDF"))" >&2; exit 1; }
echo $$ > "$PIDF"
[ -n "$(tip)" ] || { echo "main-build: no $BR branch" >&2; exit 1; }
[ -d "$WT/.git" ] || [ -f "$WT/.git" ] || git -C "$REPO" worktree add --detach "$WT" "$(tip)" > /dev/null 2>&1 || { echo "main-build: cannot make the checkout $WT" >&2; exit 1; }
PID=
stop_build() { [ -n "$PID" ] && kill -TERM -- "-$PID" 2>/dev/null; [ -n "$PID" ] && wait "$PID" 2>/dev/null; PID=; }
trap 'echo; say "stopped"; stop_build; rm -f "$ST" "$ST.n" "$PIDF"; exit 0' INT TERM
trap 'rm -f "$PIDF"' EXIT
say "following $BR in $REPO (built: $(built | cut -c1-10)); checkout $WT"
while :; do
  t=$(tip); b=$(built); bad=$(cat "$DEV/.main-bad" 2>/dev/null)
  if [ "$t" = "$b" ] || [ "$t" = "$bad" ]; then
    # a failed tip stays on the bar until a newer one comes
    if [ "$t" = "$bad" ] && [ -f "$DEV/.last-failure" ]; then status failed "$BR $(printf %s "$t" | cut -c1-10) failed in $(cut -c1-120 "$DEV/.last-failure")"; fi
    [ "${1:-}" = --once ] && exit 0; sleep "$POLL"; continue
  fi
  avail=$(df -BG --output=avail "$TG" | tail -1 | tr -dc 0-9)
  if [ "${avail:-0}" -lt "$FREE" ]; then "$HERE/worktree-gc.sh" --apply --quiet > /dev/null 2>&1; avail=$(df -BG --output=avail "$TG" | tail -1 | tr -dc 0-9); fi   # lanes' merged worktrees and idle target dirs first
  if [ "${avail:-0}" -lt "$FREE" ]; then status failed "Not building $BR $(printf %s "$t" | cut -c1-10): ${avail}G free, need ${FREE}G"; [ "${1:-}" = --once ] && exit 1; sleep 30; continue; fi
  say "building $BR $(printf %s "$t" | cut -c1-10) (tests first)"
  git -C "$WT" checkout -q --detach "$t" && git -C "$WT" clean -qfd -e Cargo.lock || { say "cannot check out $t"; sleep "$POLL"; continue; }
  ( exec setsid env BQ_MAIN=1 MAIN_PREV="$b" BUILD_ROOT="$WT" HOST_TARGET="$TG/host" GPUI_TARGET="$TG/gpui" VOICE_TARGET="$TG/voice" "$HERE/dev-build.sh" --gate ) &
  PID=$!
  # a merge while a build runs does not drop it: the build finishes and publishes what passed, then the loop builds the newest tip (one build per burst of merges, never lost work)
  moved=
  while kill -0 "$PID" 2>/dev/null; do
    if [ -z "$moved" ] && [ "$(tip)" != "$t" ]; then moved=1; say "$BR moved to $(tip | cut -c1-10): finishing the build of $(printf %s "$t" | cut -c1-10) first"; fi
    sleep 2
  done
  wait "$PID"; rc=$?; PID=
  if [ "$rc" = 0 ]; then printf '%s\n' "$t" > "$DEV/.main-built"; say "published $(printf %s "$t" | cut -c1-10)"; rm -f "$DEV/.main-bad"
  else printf '%s\n' "$t" > "$DEV/.main-bad"; say "FAILED $(printf %s "$t" | cut -c1-10): $(cat "$DEV/.last-failure" 2>/dev/null)"; fi
  [ "${1:-}" = --once ] && exit "$rc"
done
