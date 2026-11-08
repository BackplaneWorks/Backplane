#!/bin/bash
# scripts/hub-watchdog.sh's account guard, against a stand-in hub: a start that failed on the accounts after the watchdog's first look is logged, put on the desktop once,
# and the hub is asked to start it again (POST /dev/account-retry, with the token); history from before the first look, and other spawn failures, are not.
#   test/hub-watchdog-account.sh        exit 0 when it holds
set -eu
HERE=$(cd "$(dirname "$0")/.." && pwd)
. "$HERE/scripts/lib-bptmp.sh"
T=$(mktemp -d "${BP_TMP:-${TMPDIR:-/tmp}}/bp-wdacct-XXXX"); trap 'kill $HUB 2>/dev/null; rm -rf "$T"' EXIT
D=$T/fresh; mkdir -p "$D/home" "$T/bin"; printf 'tok-123' > "$D/home/token"
fail() { echo "FAIL: $*"; echo "--- watchdog.log"; cat "$D/watchdog.log" 2>/dev/null; exit 1; }
line() { printf '{"$":"ActivityLogged","thread":"%s","act":"spawn-1","tone":"error","kind":"spawn","summary":"could not start claude: %s","at":1}\n' "$1" "$2" >> "$D/home/events.jsonl"; }
line t-old "Accounts are not available; select the default account"   # history: before the guard's first look
printf '#!/bin/sh\necho "$@" >> %s/notified\n' "$T" > "$T/bin/notify-send"; chmod +x "$T/bin/notify-send"
PORT=$(python3 -I -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])')
python3 -I - "$PORT" "$T/asked" <<'EOF' &
import http.server, sys
port, asked = int(sys.argv[1]), sys.argv[2]
class H(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_GET(self):
        self.send_response(200 if self.path in ("/hello", "/healthz") else 404); self.end_headers(); self.wfile.write(b'{"status":"ok"}')
    def do_POST(self):
        open(asked, "a").write(self.path + " " + self.headers.get("Authorization", "") + "\n")
        self.send_response(200); self.end_headers(); self.wfile.write(b'{"retried":["t-new"]}')
http.server.HTTPServer(("127.0.0.1", port), H).serve_forever()
EOF
HUB=$!
for _ in $(seq 50); do curl -sf -m1 -o /dev/null "http://127.0.0.1:$PORT/hello" && break; sleep 0.1; done
run() { PATH="$T/bin:$PATH" FRESH_DIR=$D FRESH_PORT=$PORT WATCHDOG_WATCHER=0 GPUI_DEV_DIR=$T/dev bash "$HERE/scripts/hub-watchdog.sh" check; }
run
[ ! -e "$T/asked" ] || fail "history before the first look was taken for news"
line t-other "this thread's worktree is not ready"                     # not an account error
run; [ ! -e "$T/asked" ] || fail "another spawn failure was taken for an account one"
line t-new "Accounts are not available; select the default account"; line t-new2 "Reconnect and select this account before starting a turn"
run
grep -q "account guard: 2 start(s) failed on the accounts: t-new t-new2" "$D/watchdog.log" || fail "not logged"
grep -q '/dev/account-retry Bearer tok-123' "$T/asked" || fail "the hub was not asked to retry, with its token: $(cat "$T/asked" 2>/dev/null)"
grep -q 'the hub answered: {"retried":\["t-new"\]}' "$D/watchdog.log" || fail "the answer is not logged"
sleep 0.3; [ "$(wc -l < "$T/notified")" = 1 ] || fail "one desktop note a batch"
run; [ "$(wc -l < "$T/asked")" = 1 ] || fail "the same failures were taken twice"
echo "ok: the watchdog's account guard logs, alerts once and asks the hub to retry"
