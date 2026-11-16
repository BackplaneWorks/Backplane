#!/bin/bash
# Feature cats on an iOS simulator on the Mac, headless, through real taps (XCUITest Cats/test_cats), against a temp-home hub on this machine.
#   test/e2e/ios/cats.sh HUB_BIN [OUT]     Take the work_run lock mac-heavy around it. The app: ~/$MAC_DIR/ios/build/dd (scripts/mobile/ios-check.sh with
#   D=$MAC_DIR builds it). The hub: scripts/mobile/data-hub.sh's seed plus an uberbot Bub (look by its name) and ada's cat picked and lilac; its control
#   port (PORT+1) answers /botset, the uberbot's last BotSet line. Both reach the simulator through ssh -R. Stops what it started, by pid.
set -u
HUB=$(realpath "${1:?hub binary}"); ROOT=$(cd "$(dirname "$0")/../../.." && pwd); OUT=${2:-$ROOT/build/e2e-ios-cats}; OUT=$(mkdir -p "$OUT" && cd "$OUT" && pwd)
. "$ROOT/scripts/lib-bptmp.sh"; . "$ROOT/test/e2e/lib/own-group.sh"
PORT=${PORT:-3861}; MAC_DIR=${MAC_DIR:-bp-uber-cat-phones-ios}; SIM=${SIM:-bp-cats}; M=${MAC_SSH:-$ROOT/build/gpui/mac-ssh}
[ -x "$M" ] || M=$(dirname "$(git -C "$ROOT" rev-parse --path-format=absolute --git-common-dir)")/build/gpui/mac-ssh
H=$BP_TMP/cats-ios-hub; rm -rf "$OUT"/*; pids=()
stop() { for p in "${pids[@]}"; do kill "$p" 2>/dev/null; done; H=$H PORT=$PORT sh "$ROOT/scripts/mobile/data-hub.sh" stop >/dev/null; own_stop; rm -rf "$H"; }
trap stop EXIT; trap "exit 143" INT TERM HUP
mkdir -p "$H"; N=$(date +%s)
cat > "$H/extra.jsonl" <<EOF
{"\$":"ThreadCreated","id":"tb0","project":"","title":"Bub","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":$N}
{"\$":"BotSet","id":"b0","name":"Bub","thread":"tb0","look":0,"auto":true,"color":"","persona":"","at":$N}
{"\$":"SettingSet","key":"uber.bot","value":"b0","at":$N}
{"\$":"BotSet","id":"b1","name":"ada","thread":"tb1","look":3,"auto":false,"color":"#a98bf0","persona":"","at":$N}
EOF
H=$H PORT=$PORT EXTRA=$H/extra.jsonl HUBBIN=$HUB sh "$ROOT/scripts/mobile/data-hub.sh" start > "$OUT/hub.out" 2>&1 || { cat "$OUT/hub.out"; exit 1; }
own python3 -c '
import http.server, json, sys
ev = sys.argv[1]
class H(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_GET(self):
        last = {}
        for l in open(ev):
            try: e = json.loads(l)
            except Exception: continue
            if e.get("$") == "BotSet" and e.get("id") == "b0": last = e
        b = json.dumps({k: last.get(k) for k in ("name", "look", "auto", "color")}).encode()
        self.send_response(200); self.send_header("content-type", "application/json"); self.end_headers(); self.wfile.write(b)
http.server.HTTPServer(("127.0.0.1", int(sys.argv[2])), H).serve_forever()' "$H/home/events.jsonl" $((PORT+1)); pids+=($!)
$M -N -o ExitOnForwardFailure=yes -R $PORT:127.0.0.1:$PORT -R $((PORT+1)):127.0.0.1:$((PORT+1)) hs-mac-mini & pids+=($!)
sleep 3; kill -0 "${pids[-1]}" 2>/dev/null || { echo "the tunnel did not start"; exit 1; }
TOK=$(cat "$H/home/token")
RSYNC_RSH=$M rsync -a --delete "$ROOT/test/e2e/ios/" hs-mac-mini:$MAC_DIR/e2e/
$M hs-mac-mini "
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; cd ~/$MAC_DIR
U=\$(xcrun simctl list devices available | grep -E '^ +$SIM \(' | grep -o '[0-9A-F-]\{36\}' | head -1); [ -n \"\$U\" ] || U=\$(xcrun simctl create $SIM 'iPhone 17 Pro')
echo simulator \$U
xcrun simctl boot \$U 2>/dev/null; xcrun simctl bootstatus \$U -b >/dev/null 2>&1
xcrun simctl uninstall \$U dev.backplane.gpui 2>/dev/null; xcrun simctl install \$U ios/build/dd/Build/Products/Debug-iphonesimulator/Backplane.app || exit 3
printf %s '$TOK' > c-token; rm -rf c-out; mkdir -p c-out
nice -n 10 xcodebuild build-for-testing -quiet -project e2e/E2E.xcodeproj -scheme BackplaneE2E -sdk iphonesimulator -destination \"platform=iOS Simulator,id=\$U\" -derivedDataPath e2e-dd CODE_SIGNING_ALLOWED=NO > c-out/build.log 2>&1 || { grep -E 'error' c-out/build.log | head; exit 4; }
TEST_RUNNER_HUB_HOST=127.0.0.1 TEST_RUNNER_HUB_PORT=$PORT TEST_RUNNER_HUB_TOKEN_FILE=\$HOME/$MAC_DIR/c-token TEST_RUNNER_HUB_CTL=http://127.0.0.1:$((PORT+1)) TEST_RUNNER_OUT=\$HOME/$MAC_DIR/c-out \
  xcodebuild test-without-building -project e2e/E2E.xcodeproj -scheme BackplaneE2E -sdk iphonesimulator -destination \"platform=iOS Simulator,id=\$U\" -derivedDataPath e2e-dd CODE_SIGNING_ALLOWED=NO -only-testing:BackplaneE2E/Cats/test_cats > c-out/xcodebuild.log 2>&1
rc=\$?; rm -f c-token; xcrun simctl shutdown \$U 2>/dev/null; exit \$rc
" 2>&1 | tail -15 | tee "$OUT/run.log"; rc=${PIPESTATUS[0]}
RSYNC_RSH=$M rsync -a hs-mac-mini:$MAC_DIR/c-out/ "$OUT/"
python3 - "$OUT/cats.json" <<'PY' 2>&1 | tee -a "$OUT/run.log"
import json,sys
try: r=json.load(open(sys.argv[1]))
except Exception as e: print("no cats.json: the test did not run"); sys.exit(0)
print("status", r.get("status"))
for s in r["metrics"]["steps"]: print(" ", "ok  " if s.get("ok") else "FAIL", s["do"])
for n in r.get("notes", []): print("  note", n[:200])
PY
[ -f "$OUT/cats.json" ] || { [ "$rc" = 0 ] && rc=5; }
echo "xcodebuild exit $rc"; exit "$rc"
