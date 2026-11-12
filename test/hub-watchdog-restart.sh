#!/bin/bash
# scripts/hub-watchdog.sh brings a dead hub back, again and again: what a restart starts (here a stand-in fresh.sh whose hub lives on, detached, like the real one) must
# never carry the check's lock, or every later check finds it taken and does nothing (2026-11-11 00:06 to 11-12: the hub died at 02:16 and stayed down until 10:57).
# And a check lock taken for WATCHDOG_STUCK seconds is logged, with who holds it: the watchdog is never silent.
#   test/hub-watchdog-restart.sh        exit 0 when it holds
set -eu
HERE=$(cd "$(dirname "$0")/.." && pwd)
. "$HERE/scripts/lib-bptmp.sh"
T=$(mktemp -d "${BP_TMP:-${TMPDIR:-/tmp}}/bp-wdrst-XXXX")
D=$T/fresh; mkdir -p "$D/home" "$T/bin" "$T/dev"
cleanup() { [ -f "$D/hub.pid" ] && kill "$(cat "$D/hub.pid")" 2>/dev/null; [ -n "${HOLD:-}" ] && kill "$HOLD" 2>/dev/null; rm -rf "$T"; }
trap cleanup EXIT
fail() { echo "FAIL: $*"; echo "--- watchdog.log"; cat "$D/watchdog.log" 2>/dev/null; exit 1; }
printf '#!/bin/sh\n:\n' > "$T/bin/notify-send"; chmod +x "$T/bin/notify-send"
PORT=$(python3 -I -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])')
cat > "$T/hub.py" <<'EOF'
import http.server, sys
class H(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_GET(self):
        self.send_response(200 if self.path in ("/hello", "/healthz") else 404); self.end_headers(); self.wfile.write(b'{"status":"ok"}')
http.server.HTTPServer(("127.0.0.1", int(sys.argv[1])), H).serve_forever()
EOF
# the stand-in fresh.sh: start leaves a detached hub behind (setsid, as the real one), stop kills it
cat > "$T/fresh.sh" <<EOF
#!/bin/bash
case "\$1" in
  stop) kill "\$(cat $D/hub.pid 2>/dev/null)" 2>/dev/null; rm -f $D/hub.pid; echo stopped;;
  start) ( exec setsid python3 -I $T/hub.py $PORT > /dev/null 2>&1 < /dev/null ) & echo \$! > $D/hub.pid; echo \$((\$(cat $D/starts 2>/dev/null || echo 0) + 1)) > $D/starts;;
esac
EOF
chmod +x "$T/fresh.sh"
run() { PATH="$T/bin:$PATH" FRESH_DIR=$D FRESH_PORT=$PORT GPUI_DEV_DIR=$T/dev WATCHDOG_WATCHER=0 WATCHDOG_FRESH=$T/fresh.sh WATCHDOG_CUT=/bin/false \
  WATCHDOG_TRIES=2 WATCHDOG_WAIT=10 WATCHDOG_BACKOFF=0 WATCHDOG_STUCK=${STUCK:-300} bash "$HERE/scripts/hub-watchdog.sh" check; }
up() { curl -sf -m1 -o /dev/null "http://127.0.0.1:$PORT/hello"; }
LOCK=$D/.watchdog/run.lock
"$T/fresh.sh" start; for _ in $(seq 50); do up && break; sleep 0.1; done; up || fail "the stand-in hub did not start"
run; grep -q "healthy: /hello on $PORT answers" "$D/watchdog.log" || fail "no healthy line on a first good check"
for k in 1 2 3; do   # die, come back; three times over: a leaked lock stops the second
  kill -9 "$(cat "$D/hub.pid")"; for _ in $(seq 50); do up || break; sleep 0.1; done; up && fail "the stand-in hub did not die"
  run; run
  up || fail "kill $k: the hub did not come back"
  [ "$(cat "$D/starts")" = $((k + 1)) ] || fail "kill $k: $(cat "$D/starts") starts, not $((k + 1))"
  [ -z "$(fuser "$LOCK" 2>/dev/null)" ] || fail "kill $k: the restarted hub holds the check lock: $(fuser "$LOCK" 2>&1)"
done
[ "$(grep -c 'the hub answers again' "$D/watchdog.log")" -ge 3 ] || fail "three comebacks not logged"
# a lock taken by something else: the first check notes it, the one after WATCHDOG_STUCK seconds says so and names the holder; the hub is not touched meanwhile
( exec 8> "$LOCK"; flock 8; exec sleep 60 ) & HOLD=$!; sleep 0.3   # one process, the sleep, holds it
STUCK=1 run; sleep 1.2; STUCK=1 run
grep -q "the check lock $LOCK has been taken for [0-9]* s: no check runs. Held by:.*(sleep)" "$D/watchdog.log" || fail "a stuck lock was not logged with its holder"
kill "$HOLD" 2>/dev/null; wait "$HOLD" 2>/dev/null || true; HOLD=
run; [ ! -e "$D/.watchdog/busy-since" ] || fail "a free lock did not clear busy-since"
echo "ok: the watchdog restarts a dead hub every time, nothing it starts holds its lock, and a stuck lock is logged"
