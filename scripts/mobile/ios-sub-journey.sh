#!/bin/sh
# The iOS subagent drill-down journey (test_subagent in test/e2e/ios/UITests): a rig hub on a temp home (stand-in agents, --no-tailscale; ios-term-rig.sh), a thread whose stand-in runs
# two background subagents for a while, the app built on the Mac and driven in the Simulator. Shots land in $OUT (default $BP_TMP/isub/shots/ios). Under the Backplane lock mac-heavy.
#   scripts/mobile/ios-sub-journey.sh        env: M (the ssh wrapper), SIM_UDID, OUT
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd); . "$ROOT/scripts/lib-bptmp.sh"
M=${M:-/home/h/Documents/i2cjak/backplane-gpui/build/gpui/mac-ssh}; U=${SIM_UDID:-E3BC1B9E-AE4D-498C-97AF-B22FE897B41F}; OUT=${OUT:-$BP_TMP/isub/shots/ios}
IP=$(tailscale ip -4 | head -1); D=bp-isub; R=$BP_TMP/iterm-rig
mkdir -p "$OUT"
"$ROOT/scripts/mobile/ios-term-rig.sh" stop; sleep 2; "$ROOT/scripts/mobile/ios-term-rig.sh" start || exit 1; sleep 7
THREAD=$(cd "$ROOT/test/e2e/ios" && ~/.bun/bin/bun -e '
const [host, port, tokf, dir] = process.argv.slice(1); const tok = (await Bun.file(tokf).text()).trim();
const ws = new WebSocket(`ws://${host}:${port}/ws?data=1&enc=json&token=${tok}`); let id = 0; const wait = new Map();
ws.onmessage = (e) => { const m = JSON.parse(String(e.data)); if (m.t === "reply") wait.get(m.id)?.(m); };
const call = (m, p) => new Promise((res) => { const i = ++id; wait.set(i, res); ws.send(JSON.stringify({ id: i, m, p })); });
await new Promise((r) => (ws.onopen = r));
const p = await call("project.add", { path: dir, title: "Rig" }); const project = p.result?.project?.id ?? (await call("projects.list", {})).result.projects[0].id;
const t = await call("thread.create", { project, title: "Survey", text: "mode:stream BP_STREAM_N=4 survey agents:2 agentsteps:3 subwork:180000" });
console.log(t.result.thread?.id ?? t.result.thread); ws.close();' "$IP" 3801 "$R/home/token" "$R/proj") || exit 1
echo "thread $THREAD"
RSYNC_RSH=$M rsync -a --delete --exclude build "$ROOT/mobile/ios/" hs-mac-mini:$D/ios/ || exit 1
RSYNC_RSH=$M rsync -a --delete "$ROOT/test/e2e/ios/" hs-mac-mini:$D/e2e/ || exit 1
RSYNC_RSH=$M rsync -a "$R/home/token" hs-mac-mini:$D/token || exit 1
$M hs-mac-mini '
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
cd ~/'$D'/ios || exit 1
lockf -k /tmp/bp-mac-heavy.lockf sh -c "
xcodebuild -quiet -project Backplane.xcodeproj -scheme Backplane -configuration Debug -sdk iphonesimulator -destination \"generic/platform=iOS Simulator\" \
  -derivedDataPath build/dd CODE_SIGNING_ALLOWED=NO OTHER_SWIFT_FLAGS=-DDATA_APP SWIFT_ACTIVE_COMPILATION_CONDITIONS=\"GPUI_NATIVE DEBUG\" build 2>&1 | grep -E \"error:\" | head -30
cd ~/'$D'/e2e && rm -rf ~/'$D'/out && SIM_UDID='$U' OUT=\$HOME/'$D'/out HUB_HOST='$IP' HUB_PORT=3801 HUB_TOKEN_FILE=\$HOME/'$D'/token TERM_THREAD='$THREAD' \
  APP=\$HOME/'$D'/ios/build/dd/Build/Products/Debug-iphonesimulator/Backplane.app E2E_DD=\$HOME/'$D'/e2e-dd sh ./run.sh subagent
xcrun simctl shutdown '$U'"
'
RSYNC_RSH=$M rsync -a --exclude "*.xcresult" --exclude "*.mp4" hs-mac-mini:$D/out/ "$OUT/"
"$ROOT/scripts/mobile/ios-term-rig.sh" stop
python3 -c 'import json,sys; j=json.load(open(sys.argv[1])); print(j["status"], [(s["do"][:60], s["ok"]) for s in j["metrics"]["steps"]])' "$OUT/subagent.json" || tail -20 "$OUT/subagent.xcodebuild.log"
