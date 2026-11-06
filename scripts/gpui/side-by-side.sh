#!/bin/bash
# GPUI + the Rust hub on a private COPY of your real data, side by side with today's Backplane. docs/gpui/SIDE-BY-SIDE.md.
#   scripts/gpui/side-by-side.sh            start (asks you to type yes)
#   scripts/gpui/side-by-side.sh rollback   stop and delete the copy: the one line back
#   scripts/gpui/side-by-side.sh status | stop
#   scripts/gpui/side-by-side.sh watch      stay open and relaunch on each new test build, on the same copy (dev-reload.sh; docs/gpui/DEV-RELOAD.md)
# Today's Backplane (3787, ~/.backplane-bend) is only read once, to copy its log; it is never stopped or written.
# Run it yourself in a terminal: it refuses without one, so no agent can start it.
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
export SWITCH_DIR=${SWITCH_DIR:-$HOME/.backplane-gpui-side}
cmd=${1:-start}; case "$cmd" in --*) cmd=start;; esac   # an option alone means start
WORDS="This is a copy of your data. Agents you start here run in your real project folders: their file changes are real, and going back does not undo them (this copy keeps no history of them; use git). Threads and settings made here are lost when you go back. Nothing else runs by itself."
case "$cmd" in
  start)
    [ -t 0 ] && [ -t 1 ] || { echo "side-by-side: run this yourself in a terminal (it needs your yes)" >&2; exit 1; }
    cat <<TXT
$WORDS
Also: an agent has a shell and your real logins, so nothing here stops it pushing, or reaching today's Backplane.
Don't work the same folder in both apps: nothing here can tell if today's Backplane starts a turn there after you do.
A copied thread starts a fresh agent session: the agent does not know the thread's earlier conversation (the messages stay on screen).
Port ${SWITCH_PORT:-3790}; your live Backplane is read once, to copy its log, and keeps running. Back out any time:  $0 rollback
TXT
    printf 'Type yes to start: '; read -r a; [ "$a" = yes ] || { echo "not started"; exit 1; }
    export SWITCH_ALLOW_LIVE=person SWITCH_DISPLAY=${SWITCH_DISPLAY:-${DISPLAY:?set DISPLAY}}
    case "${1:-}" in start) shift;; esac; exec "$HERE/switch.sh" side "$@";;
  watch)
    [ -t 0 ] && [ -t 1 ] || { echo "side-by-side: run this yourself in a terminal (it needs your yes)" >&2; exit 1; }
    cat <<TXT
$WORDS
This terminal stays open and follows the test builds: each new one replaces the running hub and window on this same copy, and your thread, place and drafts
come back. A hub restart waits for running turns (Enter overrides). Ctrl-C stops following; the app keeps running.
TXT
    printf 'Type yes to follow new builds: '; read -r a; [ "$a" = yes ] || { echo "not started"; exit 1; }
    export SWITCH_ALLOW_LIVE=person SWITCH_DISPLAY=${SWITCH_DISPLAY:-${DISPLAY:?set DISPLAY}}
    exec "$HERE/dev-reload.sh";;
  rollback) echo "$WORDS"; exec "$HERE/switch.sh" rollback;;
  stop|status) exec "$HERE/switch.sh" "$cmd";;
  *) sed -n 2,7p "$0"; exit 2;;
esac
