#!/bin/sh
# pad_e2e.sh CLIENT_BIN HUB_BIN : the drawing pad end to end, on a private Xvfb with its own i3 and its own D-Bus, a temp-home hub and the stand-in Claude.
# Nothing here can reach the person's screen, session bus or home. Prints PASS/FAIL lines, exit 0 when all pass; artifacts in $OUT (default /tmp/pad-e2e).
# The composer's Draw button is off for now (2026-11-11: to be redone; screens/pad.rs stays): until it is back this run has no way in and says so.
set -eu
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../.." && pwd); MAIN=${GPUI_MAIN:-$(dirname "$(git -C "$(dirname "$0")" rev-parse --path-format=absolute --git-common-dir)")}
OUT=${OUT:-/tmp/pad-e2e}; DISP=${DISP:-:181}; XV=${XV:-$MAIN/build/gpui/xvfb}; ASSETS=${BACKPLANE_GPUI_ASSETS:-$MAIN/build/gpui/linux/assets}
[ ! -e "/tmp/.X11-unix/X${DISP#:}" ] || { echo "display $DISP in use"; exit 2; }
rm -rf "$OUT"; mkdir -p "$OUT/home" "$OUT/proj" "$OUT/bin" "$OUT/fakehome/.cache" "$OUT/images"; mkdir -m 700 "$OUT/run"
printf '#!/bin/sh\nexec %s %s "$@"\n' "$(command -v bun)" "$ROOT/test/tools/claude_standin.ts" > "$OUT/bin/claude"; chmod +x "$OUT/bin/claude"
env -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE -u DBUS_SESSION_BUS_ADDRESS HOME="$OUT/fakehome" XDG_RUNTIME_DIR="$OUT/run" XDG_CACHE_HOME="$OUT/fakehome/.cache" DISPLAY="$DISP" LC_ALL=C \
  dbus-run-session -- python3 -I "$HERE/pad_e2e.py" "$(realpath "$1")" "$(realpath "$2")" "$OUT" "$DISP" "$XV" "$ASSETS" "$ROOT" > "$OUT/session.log" 2>&1 && rc=0 || rc=$?
grep -E '^(PASS|FAIL|gpu:|[0-9]+ passed)|^Traceback|^  File|Error' "$OUT/session.log" || true
exit $rc
