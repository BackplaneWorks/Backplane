#!/bin/bash
# The copy button and Run in terminal on an iOS simulator on the Mac, headless (simctl only), through real taps (XCUITest CopyRows) against a private hub on a
# temp home here: test/e2e/ios/copy-rows.sh HUB_BIN [OUT] [TEST...]   TEST: test_copy_rows (default) and/or test_copy_shot
#   the hub: copy-hub.mjs (a project thread, the uberbot Bub's chat, the bot Pip's chat and a room, each with the same reply); its control port: f-ctl.mjs
#   (PORT+1, /find for the markers Run in terminal writes); both reach the simulator through ssh -R. The app is built on the Mac from this checkout
#   (mobile/ios, DATA_APP Debug). Take the work_run lock mac-heavy around it; the Mac side holds its lockf too. Stops what it started, by pid.
set -u
HUB=$(realpath "${1:?hub binary}"); ROOT=$(cd "$(dirname "$0")/../../.." && pwd); OUT=${2:-$ROOT/build/e2e-ios-copy}; OUT=$(mkdir -p "$OUT" && cd "$OUT" && pwd)
shift; [ $# -gt 0 ] && shift; TESTS=${*:-test_copy_rows}
PORT=${PORT:-$(python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));p=s.getsockname()[1];s.close();print(p)')}
MAC_DIR=${MAC_DIR:-bp-copyable-ios}; SIM=${SIM:-bp-copy-rows}; APP_ID=${APP_ID:-dev.backplane.gpui}
M=${MAC_SSH:-$(dirname "$(git -C "$ROOT" rev-parse --path-format=absolute --git-common-dir)")/build/gpui/mac-ssh}
. "$ROOT/test/e2e/lib/own-group.sh"; rm -rf "$OUT"/*; pids=()
stop() { curl -s -m3 "http://127.0.0.1:$((PORT+1))/stop" >/dev/null 2>&1; sleep 1; for p in "${pids[@]}"; do kill "$p" 2>/dev/null; done; own_stop; }
trap stop EXIT; trap "exit 143" INT TERM HUP
own bun "$ROOT/test/e2e/ios/copy-hub.mjs" "$HUB" "$OUT/hub" "$PORT" > "$OUT/hub.out" 2>&1; pids+=($!)
for _ in $(seq 60); do [ -f "$OUT/hub/state.json" ] && break; sleep 1; done
[ -f "$OUT/hub/state.json" ] || { echo "the hub did not start"; tail -5 "$OUT/hub.out"; exit 1; }
own bun "$ROOT/test/e2e/ios/f-ctl.mjs" "$HUB" "$OUT" "$PORT" > "$OUT/ctl.log" 2>&1; pids+=($!)
for _ in $(seq 20); do grep -q '^ready' "$OUT/ctl.log" && break; sleep 0.5; done
$M -N -o ExitOnForwardFailure=yes -R $PORT:127.0.0.1:$PORT -R $((PORT+1)):127.0.0.1:$((PORT+1)) hs-mac-mini & pids+=($!)
sleep 3; kill -0 "${pids[-1]}" 2>/dev/null || { echo "the tunnel did not start"; exit 1; }
TOK=$(python3 -c "import json;print(json.load(open('$OUT/hub/state.json'))['token'])")
$M hs-mac-mini "mkdir -p $MAC_DIR/mobile" || exit 1
RSYNC_RSH=$M rsync -a --delete --exclude build "$ROOT/mobile/ios/" hs-mac-mini:$MAC_DIR/mobile/ios/ || exit 1
RSYNC_RSH=$M rsync -a --delete "$ROOT/test/e2e/ios/" hs-mac-mini:$MAC_DIR/e2e/ || exit 1
ONLY=$(for t in $TESTS; do printf ' -only-testing:BackplaneE2E/CopyRows/%s' "$t"; done)
$M hs-mac-mini "
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; cd ~/$MAC_DIR
lockf -k /tmp/bp-mac-heavy.lockf sh -c '
rm -rf c-out; mkdir -p c-out
(cd mobile/ios && nice -n 10 xcodebuild -quiet -project Backplane.xcodeproj -scheme Backplane -configuration Debug -sdk iphonesimulator -destination \"generic/platform=iOS Simulator\" \
  -derivedDataPath build/dd CODE_SIGNING_ALLOWED=NO OTHER_SWIFT_FLAGS=-DDATA_APP SWIFT_ACTIVE_COMPILATION_CONDITIONS=\"GPUI_NATIVE DEBUG\" build) > c-out/app-build.log 2>&1 || { grep error: c-out/app-build.log | head; exit 2; }
U=\$(xcrun simctl list devices available | grep -E \"^ +$SIM \\(\" | grep -o \"[0-9A-F-]\\{36\\}\" | head -1); [ -n \"\$U\" ] || U=\$(xcrun simctl create $SIM \"iPhone 17 Pro\")
echo simulator \$U
xcrun simctl boot \$U 2>/dev/null; xcrun simctl bootstatus \$U -b >/dev/null 2>&1
xcrun simctl uninstall \$U $APP_ID 2>/dev/null; xcrun simctl install \$U mobile/ios/build/dd/Build/Products/Debug-iphonesimulator/Backplane.app || exit 3
printf %s $TOK > c-token
nice -n 10 xcodebuild build-for-testing -quiet -project e2e/E2E.xcodeproj -scheme BackplaneE2E -sdk iphonesimulator -destination \"platform=iOS Simulator,id=\$U\" -derivedDataPath e2e-dd CODE_SIGNING_ALLOWED=NO > c-out/build.log 2>&1 || { grep -E error c-out/build.log | head; exit 4; }
xcrun simctl io \$U recordVideo --codec h264 --force c-out/copy-rows.mp4 >/dev/null 2>&1 & REC=\$!
TEST_RUNNER_APP_ID=$APP_ID TEST_RUNNER_HUB_HOST=127.0.0.1 TEST_RUNNER_HUB_PORT=$PORT TEST_RUNNER_HUB_TOKEN_FILE=\$HOME/$MAC_DIR/c-token TEST_RUNNER_HUB_CTL=http://127.0.0.1:$((PORT+1)) TEST_RUNNER_OUT=\$HOME/$MAC_DIR/c-out \
  xcodebuild test-without-building -project e2e/E2E.xcodeproj -scheme BackplaneE2E -sdk iphonesimulator -destination \"platform=iOS Simulator,id=\$U\" -derivedDataPath e2e-dd CODE_SIGNING_ALLOWED=NO$ONLY > c-out/xcodebuild.log 2>&1
rc=\$?; kill -INT \$REC 2>/dev/null; wait \$REC 2>/dev/null
rm -f c-token; xcrun simctl shutdown \$U 2>/dev/null; exit \$rc
'" 2>&1 | tail -15 | tee "$OUT/run.log"; rc=${PIPESTATUS[0]}
RSYNC_RSH=$M rsync -a hs-mac-mini:$MAC_DIR/c-out/ "$OUT/"
for j in "$OUT"/copy-rows.json "$OUT"/copy-shot.json; do [ -f "$j" ] || continue; python3 - "$j" <<'PY' 2>&1 | tee -a "$OUT/run.log"
import json,sys
r=json.load(open(sys.argv[1])); print(r["journey"], "status", r.get("status"))
for s in r["metrics"]["steps"]: print(" ", "ok  " if s.get("ok") else "FAIL", s["do"])
for n in r.get("notes", [])[:-1]: print("  note", n[:240])
PY
done
ls "$OUT"/copy-*.json >/dev/null 2>&1 || { [ "$rc" = 0 ] && rc=5; }
echo "xcodebuild exit $rc"; exit "$rc"
