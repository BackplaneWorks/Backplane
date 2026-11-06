#!/bin/sh
# iOS journey driver, on the Mac, under the mac-heavy lock (taken by whoever starts it, via work_run lock "mac-heavy").
#   run.sh JOURNEY|all
# env: HUB_HOST HUB_PORT HUB_TOKEN_FILE (the fixture hub, serve-fixture.mjs on h@backplane, never the desktop), SIM_UDID, OUT,
#      HUB_CTL (http://host:port+1, for reconnect), LONG_SECS (long-session, default 300), APP (the built Backplane.app)
# Writes $OUT/<journey>.{json,mp4,-*.png} and $OUT/summary.json in the shape of test/gpui/e2e/run.mjs.
set -u
export DEVELOPER_DIR=${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}
HERE=$(cd "$(dirname "$0")" && pwd)
: "${SIM_UDID:?}" "${OUT:?}" "${HUB_HOST:?}" "${HUB_PORT:?}" "${HUB_TOKEN_FILE:?}"
APP=${APP:-$HOME/bp-gpui-ios/ios/build/dd/Build/Products/Debug-iphonesimulator/Backplane.app}
DD=${E2E_DD:-$HOME/bp-gpui-ios/e2e-dd}
J=${1:-all}
mkdir -p "$OUT"
now_ms() { perl -MTime::HiRes=time -e 'printf "%d", time*1000'; }
xcrun simctl boot "$SIM_UDID" 2>/dev/null; xcrun simctl bootstatus "$SIM_UDID" -b >/dev/null 2>&1
xcrun simctl terminate "$SIM_UDID" dev.backplane.gpui >/dev/null 2>&1
xcrun simctl install "$SIM_UDID" "$APP" || { echo "no app at $APP"; exit 1; }
# once: build the UI test bundle
xcodebuild build-for-testing -quiet -project "$HERE/E2E.xcodeproj" -scheme BackplaneE2E -sdk iphonesimulator \
  -destination "platform=iOS Simulator,id=$SIM_UDID" -derivedDataPath "$DD" CODE_SIGNING_ALLOWED=NO > "$OUT/build.log" 2>&1 \
  || { tail -30 "$OUT/build.log"; echo "build-for-testing failed"; exit 1; }
IDS="pair projects-threads send-image stream switch-fast popups composer-keys bots-rooms viewers downloads settings reconnect long-session"
[ "$J" = all ] || IDS=$*
for id in $IDS; do
  fn=$(echo "$id" | tr - _)
  rm -rf "$OUT/$id.json" "$OUT/$id.mp4" "$OUT/$id.xcresult"
  # the app starts fresh each journey (the test relaunches it; a pair journey must find it unpaired)
  xcrun simctl terminate "$SIM_UDID" dev.backplane.gpui >/dev/null 2>&1
  VS=$(now_ms)
  xcrun simctl io "$SIM_UDID" recordVideo --codec h264 --force "$OUT/$id.mp4" >/dev/null 2>&1 &
  REC=$!
  sleep 1
  if [ "$id" = long-session ]; then
    ( echo "t_s,footprint"; t=0; while :; do p=$(xcrun simctl spawn "$SIM_UDID" launchctl list | awk '/UIKitApplication:dev.backplane.gpui\[/ {print $1}'); \
        [ -n "$p" ] && [ "$p" != "-" ] && echo "$t,$(footprint "$p" 2>/dev/null | awk '/phys_footprint:/ {print $2,$3}')"; sleep 30; t=$((t+30)); done ) > "$OUT/long-session.mem.csv" &
    MEM=$!
  else MEM=; fi
  TEST_RUNNER_HUB_HOST=$HUB_HOST TEST_RUNNER_HUB_PORT=$HUB_PORT TEST_RUNNER_HUB_TOKEN_FILE=$HUB_TOKEN_FILE TEST_RUNNER_OUT=$OUT \
  TEST_RUNNER_HUB_INVITE_FILE=${HUB_INVITE_FILE:-} TEST_RUNNER_HUB_PAIR_LINK=${HUB_PAIR_LINK:-} TEST_RUNNER_HUB_PROJECT_PATH=${HUB_PROJECT_PATH:-} TEST_RUNNER_HUB_FIRST_PROMPT=${HUB_FIRST_PROMPT:-} TEST_RUNNER_HUB_FIRST_EXPECT=${HUB_FIRST_EXPECT:-} TEST_RUNNER_HUB_ASK_PROMPT=${HUB_ASK_PROMPT:-} TEST_RUNNER_HUB_ASK_EXPECT=${HUB_ASK_EXPECT:-} TEST_RUNNER_HUB_REPLY_WAIT=${HUB_REPLY_WAIT:-} TEST_RUNNER_HUB_CTL=${HUB_CTL:-} TEST_RUNNER_LONG_SECS=${LONG_SECS:-300} TEST_RUNNER_VIDEO_START_EPOCH_MS=$VS \
  xcodebuild test-without-building -resultBundlePath "$OUT/$id.xcresult" -project "$HERE/E2E.xcodeproj" -scheme BackplaneE2E -sdk iphonesimulator \
    -destination "platform=iOS Simulator,id=$SIM_UDID" -derivedDataPath "$DD" CODE_SIGNING_ALLOWED=NO \
    -only-testing:"BackplaneE2E/Journeys/test_$fn" > "$OUT/$id.xcodebuild.log" 2>&1
  rc=$?
  [ -n "$MEM" ] && kill "$MEM" 2>/dev/null
  kill -INT "$REC" 2>/dev/null; wait "$REC" 2>/dev/null
  [ -s "$OUT/$id.json" ] || printf '{"journey":"%s","client":"ios","status":"fail","blocked":[],"metrics":{"video_start_epoch_ms":%s},"notes":["the test did not write a result (xcodebuild exit %s): see %s.xcodebuild.log"],"capture":"%s/%s.mp4"}\n' "$id" "$VS" "$rc" "$id" "$OUT" "$id" > "$OUT/$id.json"
  grep -E "error: -\[|JourneyTests.swift:[0-9]+: error|Driver.swift:[0-9]+: error|failed -" "$OUT/$id.xcodebuild.log" | head -3 > "$OUT/$id.assert.txt"
  # the app's own BPPerf records for this journey (numeric only): the navigation display times, from the unified log
  secs=$(( ($(now_ms) - VS) / 1000 + 10 ))
  xcrun simctl spawn "$SIM_UDID" log show --last ${secs}s --predicate 'process == "Backplane"' --style compact 2>/dev/null | grep -o "BPPerf [0-9 ]*" > "$OUT/$id.bpperf"
  echo "$id: xcodebuild exit $rc"
done
/usr/bin/python3 - "$OUT" <<'PY'
import json,glob,os,sys,datetime
out=sys.argv[1]; res=[]
items={"pair":"Pair","projects-threads":"Open projects and threads","send-image":"Send, reply and paste an image","stream":"Watch a running turn stream","switch-fast":"Switch threads fast","popups":"Mention and command popups","composer-keys":"Return, Shift+Return and autocorrect","bots-rooms":"Bots and rooms","viewers":"Open Board, Schematic and 3D viewers","downloads":"Download files","settings":"Settings","reconnect":"Reconnect after sleep","long-session":"Long session with 20 simulated agents"}
for f in sorted(glob.glob(out+"/*.json")):
    if f.endswith("summary.json"): continue
    r=json.load(open(f)); r["item"]=items.get(r["journey"],r["journey"]); res.append(r)
gate={r["item"]:("pass" if r["status"]=="pass" else "fail" if r["status"]=="fail" else "open") for r in res}
counts={}
for r in res: counts[r["status"]]=counts.get(r["status"],0)+1
caps=[r["capture"] for r in res if r.get("capture")]
json.dump({"client":"ios","when":datetime.datetime.utcnow().isoformat()+"Z","hz":120,"results":res,"captures":caps,"gate":gate,"counts":counts},open(out+"/summary.json","w"),indent=1)
print(json.dumps(counts))
PY
