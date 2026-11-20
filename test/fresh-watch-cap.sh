#!/bin/bash
# scripts/fresh-watch.sh's wait for a new build, against a stand-in hub and fresh.sh: a build waits only for the turns the person is waiting on
# (/dev/turns `waited`), never longer than GPUI_RELOAD_CAP, shows its countdown in the bar (STATUS done/total), and relaunches at once when nobody waits.
# Nov 20: with agents always mid-turn the old rule (0 running turns) never held, and no build reached the person for a day.
#   test/fresh-watch-cap.sh        exit 0 when it holds
set -eu
HERE=$(cd "$(dirname "$0")/.." && pwd)
. "$HERE/scripts/lib-bptmp.sh"
T=$(mktemp -d "${BP_TMP:-${TMPDIR:-/tmp}}/bp-fwcap-XXXX")
D=$T/fresh; DEV=$T/dev; S=$T/scripts; mkdir -p "$D/home" "$DEV/builds/b1-old" "$S"
WATCH=; cleanup() { [ -n "$WATCH" ] && kill "$WATCH" 2>/dev/null; p=$(cat "$D/hub.pid" 2>/dev/null) && kill "$p" 2>/dev/null; rm -rf "$T"; }; trap cleanup EXIT
fail() { echo "FAIL: $*"; echo "--- watcher"; cat "$T/watch.log"; echo "--- fresh.sh calls"; cat "$T/calls" 2>/dev/null; exit 1; }
printf 'tok' > "$D/home/token"; : > "$D/.fresh"
cp "$HERE/scripts/fresh-watch.sh" "$HERE/scripts/hub-watchdog.sh" "$S/"
PORT=$(python3 -I -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])')
# the stand-in hub: /hello, and /dev/turns from $T/turns
cat > "$T/hub.py" <<'PY'
import http.server, sys
port, turns = int(sys.argv[1]), sys.argv[2]
class H(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_GET(self):
        body = open(turns).read().encode() if self.path == "/dev/turns" else b"{}"
        self.send_response(200); self.end_headers(); self.wfile.write(body)
http.server.HTTPServer(("127.0.0.1", port), H).serve_forever()
PY
cat > "$S/fresh.sh" <<SH
#!/bin/bash
echo "\$(date +%s) \$1" >> "$T/calls"
case "\$1" in
  stop) kill "\$(cat "$D/hub.pid")" 2>/dev/null; rm -f "$D/hub.pid";;
  start) python3 -I "$T/hub.py" $PORT "$T/turns" > /dev/null 2>&1 & echo \$! > "$D/hub.pid"; for _ in \$(seq 50); do curl -sf -m1 -o /dev/null http://127.0.0.1:$PORT/hello && break; sleep 0.1; done;;
esac
SH
chmod +x "$S/fresh.sh"
echo '{"running":3,"background":0,"waited":1}' > "$T/turns"
FRESH_DIR=$D GPUI_DEV_DIR=$DEV "$S/fresh.sh" start
echo b1-old > "$DEV/.fresh-shown"
run() { env -u DISPLAY -u WAYLAND_DISPLAY FRESH_DIR=$D FRESH_PORT=$PORT GPUI_DEV_DIR=$DEV GPUI_RELOAD_POLL=1 GPUI_RELOAD_CAP=${1} GPUI_RELOAD_BOOT=10 PATH="$T/bin:$PATH" bash "$S/fresh-watch.sh" < /dev/null >> "$T/watch.log" 2>&1 & WATCH=$!; }
mkdir -p "$T/bin"; printf '#!/bin/sh\nexit 0\n' > "$T/bin/notify-send"; chmod +x "$T/bin/notify-send"
# 1. a turn the person waits on: the build waits, says its countdown, then goes at the cap
run 6; t0=$(date +%s)
mkdir -p "$DEV/builds/20261120-160000-aaaa111111"; printf 'x' > "$DEV/builds/20261120-160000-aaaa111111/backplane-hub"; echo 20261120-160000-aaaa111111 > "$DEV/CURRENT"
sleep 3
grep -q '"phase":"waiting"' "$DEV/STATUS" 2>/dev/null || fail "no waiting bar"
grep -q 'relaunching in 1 min, or when the 1 turn(s) you are waiting on end' "$DEV/STATUS" || fail "the bar does not say the countdown: $(cat "$DEV/STATUS")"
grep -q '"total":6' "$DEV/STATUS" && grep -q '"deadline":' "$DEV/STATUS" || fail "no countdown fill or deadline: $(cat "$DEV/STATUS")"
! grep -q stop "$T/calls" || fail "relaunched while the person waits, before the cap"
for _ in $(seq 20); do grep -q stop "$T/calls" && break; sleep 0.5; done
grep -q stop "$T/calls" || fail "did not relaunch at the cap"
t1=$(awk '$2=="stop"{print $1; exit}' "$T/calls"); [ $((t1 - t0)) -ge 5 ] || fail "relaunched after $((t1 - t0)) s, before the 6 s cap"
grep -q "relaunching after 0 min with 1 turn(s) still running: they carry on after the restart" "$T/watch.log" || fail "the relaunch at the cap is not said"
for _ in $(seq 20); do grep -q 20261120-160000-aaaa111111 "$DEV/.fresh-shown" && break; sleep 0.5; done
grep -q 20261120-160000-aaaa111111 "$DEV/.fresh-shown" || fail "the build was not taken as shown"
grep -q "restart.next" <(ls "$D/home") && grep -q '"build":"20261120-160000-aaaa111111"' "$D/home/restart.next" || fail "the hub was not told which build it comes back on"
# 2. turns running but nobody waits on them: the next build goes at once
: > "$T/calls"; echo '{"running":5,"background":0,"waited":0}' > "$T/turns"
mkdir -p "$DEV/builds/20261120-161000-bbbb222222"; printf 'y' > "$DEV/builds/20261120-161000-bbbb222222/backplane-hub"; echo 20261120-161000-bbbb222222 > "$DEV/CURRENT"
for _ in $(seq 10); do grep -q stop "$T/calls" && break; sleep 0.5; done
grep -q stop "$T/calls" || fail "waited for turns nobody waits on"
grep -q "relaunching with 5 running turn(s) nobody is waiting on" "$T/watch.log" || fail "not said"
echo "fresh-watch-cap: ok"
