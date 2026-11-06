#!/bin/sh
# One command to try the Rust hub + GPUI app: a seeded synthetic home (nothing real), stand-in agents (no model), loopback port 3790,
# your own screen. Today's Backplane is not touched. Stop: scripts/gpui/switch.sh rollback
#   scripts/gpui/try-rust.sh            (opens the window on $DISPLAY; SWITCH_DISPLAY=:N for another)
exec env SWITCH_DISPLAY="${SWITCH_DISPLAY:-${DISPLAY:?set DISPLAY or SWITCH_DISPLAY}}" "$(dirname "$0")/switch.sh" try "$@"
