#!/bin/bash
# Phone push on the iOS simulator against a throwaway hub: test/e2e/push/run.sh MODE HUB_BIN [OUT]
#   MODE sink: the hub's exact payloads go to the simulator by `simctl push`, for a token the app really registered or, if the simulator gets none, a stand-in one
#   MODE apns: real APNs sandbox to the simulator's real token
# Own Mac folder (MAC_DIR), own simulator (bp-push), heavy steps under the Mac lock. No phone is ever involved: the hub's only device is the simulator. Stops what it started, by pid.
set -u
MODE=${1:?sink or apns}; HUB=${2:?hub binary}; ROOT=$(cd "$(dirname "$0")/../../.." && pwd); OUT=${3:-$ROOT/build/push-e2e/$MODE}
mkdir -p "$HOME/.cache/respectful-push"; T=$(mktemp -d "$HOME/.cache/respectful-push/e2e.XXXXXX"); PORT=${PORT:-3860}; MAC_DIR=${MAC_DIR:-backplane-ios-push}; SIM=${SIM:-bp-push}; APP_ID=${APP_ID:-dev.backplane.works}
M=${MAC_SSH:-$HOME/.cache/respectful-push/mac-ssh}; mkdir -p "$OUT"
pids=(); stop() { curl -s -m3 "http://127.0.0.1:$((PORT+1))/stop" >/dev/null 2>&1; sleep 1; for p in "${pids[@]}"; do kill "$p" 2>/dev/null; done; }
trap stop EXIT
cd "$ROOT/test/e2e/push"
MODE=$MODE PERF_REPO=$ROOT bun ctl.mjs "$T" "$HUB" "$PORT" > "$OUT/ctl.log" 2>&1 & pids+=($!)
for _ in $(seq 60); do grep -q '^ready' "$OUT/ctl.log" && break; sleep 1; done
grep -q '^ready' "$OUT/ctl.log" || { echo "the hub did not start"; tail -5 "$OUT/ctl.log"; exit 1; }
$M -N -o ExitOnForwardFailure=yes -R $PORT:127.0.0.1:$PORT -R $((PORT+1)):127.0.0.1:$((PORT+1)) hs-mac-mini & pids+=($!)
sleep 3; kill -0 "${pids[-1]}" || { echo "the tunnel did not start"; exit 1; }
TOK=$(cat "$T/home/token")
export RSYNC_RSH=$M
rsync -a --delete "$ROOT/test/e2e/ios/" hs-mac-mini:$MAC_DIR/e2e/
U=$($M hs-mac-mini "export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; xcrun simctl list devices available | grep -E '^ +$SIM \(' | grep -o '[0-9A-F-]\{36\}' | head -1 | grep . || xcrun simctl create $SIM 'iPhone 17 Pro'")
echo "simulator $U" | tee "$OUT/run.log"
if [ "$MODE" = sink ]; then # hand each payload the hub wrote to the simulator, unchanged
  ( seen=0; while :; do for f in "$T"/sink/*.json; do [ -e "$f" ] || continue; n=$(basename "$f"); [ -e "$T/done-$n" ] && continue; : > "$T/done-$n"; cp "$f" "$OUT/payload-$n"
      python3 -c "import json,sys;print(json.dumps(json.load(open(sys.argv[1]))['payload']))" "$f" | $M hs-mac-mini "export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; xcrun simctl push $U $APP_ID -" >> "$OUT/simctl-push.log" 2>&1
    done; sleep 0.5; done ) & pids+=($!)
fi
$M hs-mac-mini "
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; cd ~/$MAC_DIR
while [ -d /tmp/bp-mac-heavy.lock ]; do sleep 5; done
xcrun simctl boot $U 2>/dev/null; xcrun simctl bootstatus $U -b >/dev/null 2>&1
xcrun simctl uninstall $U $APP_ID 2>/dev/null; xcrun simctl install $U ios/build/dd-push/Build/Products/Debug-iphonesimulator/Backplane.app || exit 3
printf %s '$TOK' > e2e-token; rm -rf e2e-out; mkdir -p e2e-out
lockf -k /tmp/bp-mac-heavy.lockf nice -n 10 xcodebuild build-for-testing -quiet -project e2e/E2E.xcodeproj -scheme BackplaneE2E -sdk iphonesimulator -destination 'platform=iOS Simulator,id=$U' -derivedDataPath e2e-dd CODE_SIGNING_ALLOWED=NO > e2e-out/build.log 2>&1 || { tail -20 e2e-out/build.log; exit 4; }
xcrun simctl io $U recordVideo --codec h264 --force e2e-out/push.mp4 >/dev/null 2>&1 & REC=\$!
TEST_RUNNER_APP_ID=$APP_ID TEST_RUNNER_HUB_HOST=127.0.0.1 TEST_RUNNER_HUB_PORT=$PORT TEST_RUNNER_HUB_TOKEN_FILE=\$HOME/$MAC_DIR/e2e-token TEST_RUNNER_HUB_CTL=http://127.0.0.1:$((PORT+1)) TEST_RUNNER_OUT=\$HOME/$MAC_DIR/e2e-out \
  xcodebuild test-without-building -resultBundlePath e2e-out/push.xcresult -project e2e/E2E.xcodeproj -scheme BackplaneE2E -sdk iphonesimulator -destination 'platform=iOS Simulator,id=$U' -derivedDataPath e2e-dd CODE_SIGNING_ALLOWED=NO -only-testing:BackplaneE2E/Push/test_push > e2e-out/xcodebuild.log 2>&1
rc=\$?; kill -INT \$REC 2>/dev/null; wait \$REC 2>/dev/null
xcrun simctl spawn $U log show --last 15m --predicate 'process == \"Backplane\" AND eventMessage CONTAINS \"push\"' --style compact > e2e-out/app-push.log 2>/dev/null
rm -f e2e-token; rm -rf e2e-out/push.xcresult; xcrun simctl shutdown $U 2>/dev/null; exit \$rc
" 2>&1 | tail -20 | tee -a "$OUT/run.log"; rc=${PIPESTATUS[0]}
rsync -a hs-mac-mini:$MAC_DIR/e2e-out/ "$OUT/"
# what the hub saw, with no token in it
python3 - "$T/home/events.jsonl" "$OUT/hub-push-events.jsonl" <<'PY'
import json,sys
want={"DeviceRegistered","DevicePrefs","DeviceDropped","PushOrdered","PushSent","PushCleared"}
with open(sys.argv[2],"w") as o:
    for l in open(sys.argv[1]):
        try: e=json.loads(l)
        except Exception: continue
        if e.get("$") in want:
            if "token" in e: e["token"]="<hidden>"
            o.write(json.dumps(e)+"\n")
PY
grep -E 'push:|APNs' "$T/hub.log" | sed -E 's/[0-9a-f]{64}/<token>/g' | tail -20 > "$OUT/hub-push.log"
python3 -c "
import json,sys
r=json.load(open('$OUT/push.json')); st=r['metrics']['steps']; ok=[s for s in st if s.get('ok')]; bad=[s for s in st if not s.get('ok')]
print('status',r.get('status'),'steps ok',len(ok),'failed',len(bad)); [print(' FAIL',s['do']) for s in bad]; [print(' note',n[:170]) for n in r.get('notes',[])]" 2>&1 | tee -a "$OUT/run.log"
rm -rf "$T"; echo "xcodebuild exit $rc"; exit "$rc"
