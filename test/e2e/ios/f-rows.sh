#!/bin/bash
# The phone halves of F06, F07, F17 and F21 on an iOS simulator on the Mac, headless (simctl only, no Simulator.app window), through real taps
# (XCUITest FRows/test_f_rows) against a private hub on a temp home here: test/e2e/ios/f-rows.sh HUB_BIN [OUT]
#   the hub: test/e2e/pixel-hub.mjs (the same seed and stand-in agent the Pixel rows use); its control port: f-ctl.mjs (PORT+1); both reach the simulator
#   through ssh -R. The app on the simulator: dev.backplane.gpui from ~/MAC_DIR/ios/build/dd-push (built before this runs). Take the work_run lock
#   mac-heavy around it. Stops what it started, by pid, and only the simulator it made (SIM).
set -u
HUB=$(realpath "${1:?hub binary}"); ROOT=$(cd "$(dirname "$0")/../../.." && pwd); OUT=${2:-$ROOT/build/e2e-ios-f}; OUT=$(mkdir -p "$OUT" && cd "$OUT" && pwd)
PORT=${PORT:-$(python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));p=s.getsockname()[1];s.close();print(p)')}
MAC_DIR=${MAC_DIR:-backplane-ios-push}; SIM=${SIM:-bp-f-rows}; APP_ID=${APP_ID:-dev.backplane.gpui}; M=${MAC_SSH:-ssh}
. "$ROOT/test/e2e/lib/own-group.sh"; rm -rf "$OUT"/*; pids=()
stop() { curl -s -m3 "http://127.0.0.1:$((PORT+1))/stop" >/dev/null 2>&1; sleep 1; for p in "${pids[@]}"; do kill "$p" 2>/dev/null; done; own_stop; }
trap stop EXIT; trap "exit 143" INT TERM HUP
own bun "$ROOT/test/e2e/pixel-hub.mjs" "$HUB" "$OUT/hub" "$PORT" > "$OUT/hub.out" 2>&1; pids+=($!)
for _ in $(seq 60); do [ -f "$OUT/hub/state.json" ] && break; sleep 1; done
[ -f "$OUT/hub/state.json" ] || { echo "the hub did not start"; tail -5 "$OUT/hub.out"; exit 1; }
own bun "$ROOT/test/e2e/ios/f-ctl.mjs" "$HUB" "$OUT" "$PORT" > "$OUT/ctl.log" 2>&1; pids+=($!)
for _ in $(seq 20); do grep -q '^ready' "$OUT/ctl.log" && break; sleep 0.5; done
$M -N -o ExitOnForwardFailure=yes -R $PORT:127.0.0.1:$PORT -R $((PORT+1)):127.0.0.1:$((PORT+1)) hs-mac-mini & pids+=($!)
sleep 3; kill -0 "${pids[-1]}" 2>/dev/null || { echo "the tunnel did not start"; exit 1; }
TOK=$(python3 -c "import json;print(json.load(open('$OUT/hub/state.json'))['token'])")
RSYNC_RSH=$M rsync -a --delete "$ROOT/test/e2e/ios/" hs-mac-mini:$MAC_DIR/e2e/
$M hs-mac-mini "
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; cd ~/$MAC_DIR
U=\$(xcrun simctl list devices available | grep -E '^ +$SIM \(' | grep -o '[0-9A-F-]\{36\}' | head -1); [ -n \"\$U\" ] || U=\$(xcrun simctl create $SIM 'iPhone 17 Pro')
echo simulator \$U
xcrun simctl boot \$U 2>/dev/null; xcrun simctl bootstatus \$U -b >/dev/null 2>&1
xcrun simctl uninstall \$U $APP_ID 2>/dev/null; xcrun simctl install \$U ios/build/dd-push/Build/Products/Debug-iphonesimulator/Backplane.app || exit 3
printf %s '$TOK' > f-token; rm -rf f-out; mkdir -p f-out
nice -n 10 xcodebuild build-for-testing -quiet -project e2e/E2E.xcodeproj -scheme BackplaneE2E -sdk iphonesimulator -destination \"platform=iOS Simulator,id=\$U\" -derivedDataPath e2e-dd CODE_SIGNING_ALLOWED=NO > f-out/build.log 2>&1 || { grep -E 'error' f-out/build.log | head; exit 4; }
xcrun simctl io \$U recordVideo --codec h264 --force f-out/f-rows.mp4 >/dev/null 2>&1 & REC=\$!
TEST_RUNNER_APP_ID=$APP_ID TEST_RUNNER_HUB_HOST=127.0.0.1 TEST_RUNNER_HUB_PORT=$PORT TEST_RUNNER_HUB_TOKEN_FILE=\$HOME/$MAC_DIR/f-token TEST_RUNNER_HUB_CTL=http://127.0.0.1:$((PORT+1)) TEST_RUNNER_OUT=\$HOME/$MAC_DIR/f-out \
  xcodebuild test-without-building -project e2e/E2E.xcodeproj -scheme BackplaneE2E -sdk iphonesimulator -destination \"platform=iOS Simulator,id=\$U\" -derivedDataPath e2e-dd CODE_SIGNING_ALLOWED=NO -only-testing:BackplaneE2E/FRows/test_f_rows > f-out/xcodebuild.log 2>&1
rc=\$?; kill -INT \$REC 2>/dev/null; wait \$REC 2>/dev/null
rm -f f-token; xcrun simctl shutdown \$U 2>/dev/null; exit \$rc
" 2>&1 | tail -15 | tee "$OUT/run.log"; rc=${PIPESTATUS[0]}
RSYNC_RSH=$M rsync -a hs-mac-mini:$MAC_DIR/f-out/ "$OUT/"
python3 - "$OUT/f-rows.json" <<'PY' 2>&1 | tee -a "$OUT/run.log"
import json,sys
try: r=json.load(open(sys.argv[1]))
except Exception as e: print("no f-rows.json: the test did not run"); sys.exit(0)
print("status", r.get("status"))
for s in r["metrics"]["steps"]: print(" ", "ok  " if s.get("ok") else "FAIL", s["do"])
for n in r.get("notes", []): print("  note", n[:200])
PY
[ -f "$OUT/f-rows.json" ] || { [ "$rc" = 0 ] && rc=5; }
echo "xcodebuild exit $rc"; exit "$rc"
