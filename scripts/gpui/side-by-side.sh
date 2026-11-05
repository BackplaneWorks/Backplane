#!/bin/bash
# GPUI + the Rust hub on a private COPY of your real data, side by side with today's Backplane. docs/gpui/SIDE-BY-SIDE.md.
#   scripts/gpui/side-by-side.sh            start (asks you to type yes)
#   scripts/gpui/side-by-side.sh rollback   stop and delete the copy: the one line back
#   scripts/gpui/side-by-side.sh status | stop
# Today's Backplane (3787, ~/.backplane-bend) is only read once, to copy its log; it is never stopped or written.
# Run it yourself in a terminal: it refuses without one, so no agent can start it.
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
export SWITCH_DIR=${SWITCH_DIR:-$HOME/.backplane-gpui-side}
cmd=${1:-start}
case "$cmd" in
  start)
    [ -t 0 ] && [ -t 1 ] || { echo "side-by-side: run this yourself in a terminal (it needs your yes)" >&2; exit 1; }
    cat <<TXT
This copies your live Backplane log (read only) and starts the Rust hub + GPUI on the copy, port ${SWITCH_PORT:-3790}.
  - Real agents run only on a turn YOU start, with your real logins, in your real project folders (they can edit real files).
  - Nothing runs by itself: no resumed turns, routines, reminders, bots, mail, push, other machines, deploys, updates.
  - Today's Backplane keeps running untouched. Back out any time:  $0 rollback
  - Lost when you go back: everything you do in the GPUI app (it lives only in the copy). Nothing flows back.
  - Missing here: terminals, browser, git commit/push, design history, secrets box. Don't work the same folder in both apps.
TXT
    printf 'Type yes to start: '; read -r a; [ "$a" = yes ] || { echo "not started"; exit 1; }
    export SWITCH_ALLOW_LIVE=person SWITCH_DISPLAY=${SWITCH_DISPLAY:-${DISPLAY:?set DISPLAY}}
    shift || true; exec "$HERE/switch.sh" side "$@";;
  stop|status|rollback) exec "$HERE/switch.sh" "$cmd";;
  *) sed -n 2,6p "$0"; exit 2;;
esac
