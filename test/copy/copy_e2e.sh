#!/bin/sh
# copy_e2e.sh CLIENT_BIN HUB_BIN : copying a selection in a rendered message, end to end, on a private Xvfb with its own i3 and its own D-Bus, a temp-home
# hub and the stand-in Claude. Real X input through XTest; the proof is the X CLIPBOARD and PRIMARY as another program (xclip) reads them.
# Nothing here can reach the person's screen, session bus or home. Prints PASS/FAIL lines, exit 0 when all pass; artifacts in $OUT (default $BP_TMP/copy-e2e).
# Env: DISP (:187), XV (the Xvfb root: ~/.local/xvfb-root), BACKPLANE_GPUI_ASSETS (default: beside the client).
set -eu
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../.." && pwd)
. "$ROOT/scripts/lib-bptmp.sh"
OUT=${OUT:-$BP_TMP/copy-e2e}; DISP=${DISP:-:187}; XV=${XV:-$HOME/.local/xvfb-root}; ASSETS=${BACKPLANE_GPUI_ASSETS:-$(dirname "$(realpath "$1")")/assets}
[ ! -e "/tmp/.X11-unix/X${DISP#:}" ] || { echo "display $DISP in use"; exit 2; }
for t in xclip xmessage i3 dbus-run-session bun; do command -v $t > /dev/null || { echo "needs $t"; exit 2; }; done
rm -rf "$OUT"; mkdir -p "$OUT/home" "$OUT/proj" "$OUT/bin" "$OUT/fakehome/.cache"; mkdir -m 700 "$OUT/run"
printf '#!/bin/sh\nexec %s %s "$@"\n' "$(command -v bun)" "$ROOT/test/tools/claude_standin.ts" > "$OUT/bin/claude"; chmod +x "$OUT/bin/claude"
env -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE -u DBUS_SESSION_BUS_ADDRESS HOME="$OUT/fakehome" XDG_RUNTIME_DIR="$OUT/run" XDG_CACHE_HOME="$OUT/fakehome/.cache" DISPLAY="$DISP" LC_ALL=C \
  dbus-run-session -- python3 -I "$HERE/copy_e2e.py" "$(realpath "$1")" "$(realpath "$2")" "$OUT" "$DISP" "$XV" "$ASSETS" "$ROOT" > "$OUT/session.log" 2>&1 && rc=0 || rc=$?
grep -E '^(PASS|FAIL|gpu:|[0-9]+ passed)|^Traceback|^  File|Error' "$OUT/session.log" || true
exit $rc
