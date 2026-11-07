#!/bin/sh
# live_e2e.sh CLIENT_BIN HUB_BIN PYTHON : one buy thread end to end in the GPUI window, on a private Xvfb with its own i3 and D-Bus, a temp-home hub,
# the stand-in Claude and the real order server on loopback (test/order/local_server.py: order/fabsrv, its stand-in fab pack, a Stripe double).
# PYTHON has order/requirements.txt installed. Never the real order server or a real payment. Prints PASS/FAIL lines; screenshots in $OUT.
# LIVE_KEEP=1 keeps the hub and order server up afterwards (ports in $OUT/rig.json) until $OUT/stop exists, for the phones.
set -eu
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../.." && pwd); MAIN=${GPUI_MAIN:-/home/h/Documents/i2cjak/backplane-gpui}
OUT=${OUT:-$HOME/.cache/bp-tmp/order-live}; DISP=${DISP:-:183}; XV=${XV:-$MAIN/build/gpui/xvfb}; ASSETS=${BACKPLANE_GPUI_ASSETS:-$MAIN/build/gpui/linux/assets}
[ ! -e "/tmp/.X11-unix/X${DISP#:}" ] || { echo "display $DISP in use"; exit 2; }
rm -rf "$OUT"; mkdir -p "$OUT/home" "$OUT/bin" "$OUT/fakehome/.cache"; mkdir -m 700 "$OUT/run"
printf '#!/bin/sh\nexec %s %s "$@"\n' "$(command -v bun)" "$ROOT/test/tools/claude_standin.ts" > "$OUT/bin/claude"; chmod +x "$OUT/bin/claude"
# Buy now's browser: the link is recorded, then opened as a browser would (following the order server's redirect through the Stripe double's checkout)
printf '#!/bin/sh\nprintf "%%s\\n" "$1" > %s/opened.txt\nexec curl -sS -L -o /dev/null "$1"\n' "$OUT" > "$OUT/bin/xdg-open"; chmod +x "$OUT/bin/xdg-open"
env -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE -u DBUS_SESSION_BUS_ADDRESS -u CLAUDE_CONFIG_DIR GVFS_DISABLE_FUSE=1 GIO_USE_VFS=local GIO_USE_VOLUME_MONITOR=unix HOME="$OUT/fakehome" XDG_RUNTIME_DIR="$OUT/run" XDG_CACHE_HOME="$OUT/fakehome/.cache" DISPLAY="$DISP" LC_ALL=C \
  dbus-run-session -- python3 -I "$HERE/live_e2e.py" "$(realpath "$1")" "$(realpath "$2")" "$OUT" "$DISP" "$XV" "$ASSETS" "$ROOT" "$3" > "$OUT/session.log" 2>&1 && rc=0 || rc=$?
grep -E '^(PASS|FAIL|shot|rig up|[0-9]+ passed)|^Traceback|^  File|Error' "$OUT/session.log" || true
exit $rc
