#!/bin/bash
# Settings > Notifications on the iOS simulator against a throwaway hub: test/e2e/push/notify-ios.sh HUB_BIN [OUT]
#   the hub: ctl.mjs MODE=sink on a temp home here (stand-in agents, a project, a worker thread, the uberbot Bub); its port and control port reach the Mac by ssh -R;
#   every payload the hub would send to APNs is handed to the simulator unchanged by `simctl push`. The app is built from this tree's mobile/ios on the Mac (MAC_DIR), the
#   test is Notify/test_notify (test/e2e/ios/UITests/NotifyTests.swift). Run it under the mac-heavy lock (work_run lock "mac-heavy"); heavy Mac steps also take the Mac's lockf.
#   Results: OUT (notify.json, notify-*.png, hub-notify-events.jsonl, hub-push.log); shots also in SHOTS as ios-*.png. Stops what it started, by pid.
set -u
HUB=$(realpath "${1:?hub binary}"); ROOT=$(cd "$(dirname "$0")/../../.." && pwd); . "$ROOT/scripts/lib-bptmp.sh"
OUT=${2:-$BP_TMP/notify-ios-out}; SHOTS=${SHOTS:-$BP_TMP/notify-shots}
T=$(mktemp -d "$BP_TMP/bp-notify-ios.XXXXXX"); PORT=${PORT:-3871}; MAC_DIR=${MAC_DIR:-bp-notify-ios}; SIM=${SIM:-bp-notify}; APP_ID=${APP_ID:-dev.backplane.gpui}
MAIN=$(dirname "$(git -C "$ROOT" rev-parse --path-format=absolute --git-common-dir)"); M=${MAC_SSH:-}
for c in "$M" "$MAIN/build/gpui/mac-ssh" /home/h/Documents/i2cjak/backplane-gpui/build/gpui/mac-ssh; do [ -n "$c" ] && [ -x "$c" ] && { M=$c; break; }; done
rm -rf "$OUT"; mkdir -p "$OUT" "$SHOTS"
pids=(); stop() { curl -s -m3 "http://127.0.0.1:$((PORT+1))/stop" >/dev/null 2>&1; sleep 1; for p in "${pids[@]}"; do kill "$p" 2>/dev/null; done; }
trap stop EXIT; trap "exit 143" INT TERM HUP
export RSYNC_RSH=$M
# the app, built from this tree (simulator Debug, DATA_APP: the app the phones run), signed ad hoc so its aps-environment entitlement is kept and the simulator gives it a push token
rsync -a --delete --exclude build "$ROOT/mobile/ios/" hs-mac-mini:$MAC_DIR/ios/ || exit 1
rsync -a --delete "$ROOT/test/e2e/ios/" hs-mac-mini:$MAC_DIR/e2e/ || exit 1
$M hs-mac-mini "export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; cd ~/$MAC_DIR/ios
lockf -k /tmp/bp-mac-heavy.lockf nice -n 10 xcodebuild -quiet -project Backplane.xcodeproj -scheme Backplane -configuration Debug -sdk iphonesimulator -destination 'generic/platform=iOS Simulator' \
  -derivedDataPath build/dd-notify CODE_SIGN_IDENTITY=- CODE_SIGN_STYLE=Manual DEVELOPMENT_TEAM= PROVISIONING_PROFILE_SPECIFIER= OTHER_SWIFT_FLAGS=-DDATA_APP SWIFT_ACTIVE_COMPILATION_CONDITIONS='GPUI_NATIVE DEBUG' build > ../app-build.log 2>&1 || { grep -E 'error:' ../app-build.log | head -20; exit 2; }" || { echo "the app did not build"; exit 2; }

cd "$ROOT/test/e2e/push"
MODE=sink PERF_REPO=$ROOT bun ctl.mjs "$T" "$HUB" "$PORT" > "$OUT/ctl.log" 2>&1 & pids+=($!)
for _ in $(seq 60); do grep -q '^ready' "$OUT/ctl.log" && break; sleep 1; done
grep -q '^ready' "$OUT/ctl.log" || { echo "the hub did not start"; tail -5 "$OUT/ctl.log"; exit 1; }
$M -N -o ExitOnForwardFailure=yes -R $PORT:127.0.0.1:$PORT -R $((PORT+1)):127.0.0.1:$((PORT+1)) hs-mac-mini & pids+=($!)
sleep 3; kill -0 "${pids[-1]}" || { echo "the tunnel did not start"; exit 1; }
TOK=$(cat "$T/home/token")
U=$($M hs-mac-mini "export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; xcrun simctl list devices available | grep -E '^ +$SIM \(' | grep -o '[0-9A-F-]\{36\}' | head -1 | grep . || xcrun simctl create $SIM 'iPhone 17 Pro'")
echo "simulator $U" | tee "$OUT/run.log"
# hand each payload the hub wrote to the simulator, unchanged
( while :; do for f in "$T"/sink/*.json; do [ -e "$f" ] || continue; n=$(basename "$f"); [ -e "$T/done-$n" ] && continue; : > "$T/done-$n"; cp "$f" "$OUT/payload-$n"
    python3 -c "import json,sys;print(json.dumps(json.load(open(sys.argv[1]))['payload']))" "$f" | $M hs-mac-mini "export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; xcrun simctl push $U $APP_ID -" >> "$OUT/simctl-push.log" 2>&1
  done; sleep 0.5; done ) & pids+=($!)
$M hs-mac-mini "
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; cd ~/$MAC_DIR
xcrun simctl boot $U 2>/dev/null; xcrun simctl bootstatus $U -b >/dev/null 2>&1
xcrun simctl uninstall $U $APP_ID 2>/dev/null
xcrun simctl install $U ios/build/dd-notify/Build/Products/Debug-iphonesimulator/Backplane.app || exit 3
printf %s '$TOK' > e2e-token; rm -rf e2e-out; mkdir -p e2e-out
lockf -k /tmp/bp-mac-heavy.lockf nice -n 10 xcodebuild build-for-testing -quiet -project e2e/E2E.xcodeproj -scheme BackplaneE2E -sdk iphonesimulator -destination 'platform=iOS Simulator,id=$U' -derivedDataPath e2e-dd CODE_SIGNING_ALLOWED=NO > e2e-out/build.log 2>&1 || { grep error e2e-out/build.log | head -20; exit 4; }
xcrun simctl io $U recordVideo --codec h264 --force e2e-out/notify.mp4 >/dev/null 2>&1 & REC=\$!
TEST_RUNNER_APP_ID=$APP_ID TEST_RUNNER_HUB_HOST=127.0.0.1 TEST_RUNNER_HUB_PORT=$PORT TEST_RUNNER_HUB_TOKEN_FILE=\$HOME/$MAC_DIR/e2e-token TEST_RUNNER_HUB_CTL=http://127.0.0.1:$((PORT+1)) TEST_RUNNER_OUT=\$HOME/$MAC_DIR/e2e-out \
  xcodebuild test-without-building -resultBundlePath e2e-out/notify.xcresult -project e2e/E2E.xcodeproj -scheme BackplaneE2E -sdk iphonesimulator -destination 'platform=iOS Simulator,id=$U' -derivedDataPath e2e-dd CODE_SIGNING_ALLOWED=NO -only-testing:BackplaneE2E/Notify/test_notify > e2e-out/xcodebuild.log 2>&1
rc=\$?; kill -INT \$REC 2>/dev/null; wait \$REC 2>/dev/null
xcrun simctl spawn $U log show --last 15m --predicate 'process == \"Backplane\" AND eventMessage CONTAINS \"push\"' --style compact > e2e-out/app-push.log 2>/dev/null
rm -f e2e-token; rm -rf e2e-out/notify.xcresult; xcrun simctl shutdown $U 2>/dev/null; exit \$rc
" 2>&1 | tail -20 | tee -a "$OUT/run.log"; rc=${PIPESTATUS[0]}
rsync -a hs-mac-mini:$MAC_DIR/e2e-out/ "$OUT/"
for f in "$OUT"/notify-*.png; do [ -e "$f" ] && cp "$f" "$SHOTS/ios-$(basename "$f" .png | sed 's/^notify-//').png"; done
# what the hub kept, with no token in it
python3 - "$T/home/events.jsonl" "$OUT/hub-notify-events.jsonl" <<'PY'
import json,sys
with open(sys.argv[2],"w") as o:
    for l in open(sys.argv[1]):
        try: e=json.loads(l)
        except Exception: continue
        k=e.get("$")
        if k in ("DeviceRegistered","DevicePrefs","PushOrdered","PushSent","PushCleared") or (k=="SettingSet" and str(e.get("key","")).startswith("notify.")):
            e.pop("token",None); o.write(json.dumps(e)+"\n")
PY
grep -E 'push:|APNs' "$T/hub.log" | sed -E 's/[0-9a-f]{64}/<token>/g' | tail -40 > "$OUT/hub-push.log"
python3 -c "
import json
r=json.load(open('$OUT/notify.json')); st=r['metrics']['steps']
print('status',r.get('status'),'steps ok',sum(1 for s in st if s.get('ok')),'failed',sum(1 for s in st if not s.get('ok')))
[print(' FAIL',s['do']) for s in st if not s.get('ok')]; [print(' note',n[:200]) for n in r.get('notes',[])]" 2>&1 | tee -a "$OUT/run.log"
[ -f "$OUT/notify.json" ] || { echo "no notify.json: the test did not run"; [ "$rc" = 0 ] && rc=5; }
rm -rf "$T"; echo "xcodebuild exit $rc"; exit "$rc"
