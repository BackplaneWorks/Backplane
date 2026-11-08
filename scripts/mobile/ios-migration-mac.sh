#!/bin/sh
# On the Mac (ios-migration-check.sh copies it there): the app's unit tests, then the Simulator against the import rig, one screenshot at each moment.
#   ios-migration-mac.sh build DIR                   DIR holds ios/ (the app), root/ (what ios-unit-tests.sh reads), out/ (the shots)
#   ios-migration-mac.sh shots DIR HUB UDID          HUB http://ip:port of the rig; the token is DIR/token
set -u
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
D=$2; OUT=$D/out; APP=$D/ios/build/dd/Build/Products/Debug-iphonesimulator/Backplane.app; B=dev.backplane.gpui
mkdir -p "$OUT"
if [ "$1" = build ]; then
sh "$D/root/scripts/mobile/ios-unit-tests.sh" > "$OUT/unit-tests.txt" 2>&1; echo "unit tests exit $?" >> "$OUT/unit-tests.txt"; cat "$OUT/unit-tests.txt"
cd "$D/ios" && xcodebuild -quiet -project Backplane.xcodeproj -scheme Backplane -configuration Debug -sdk iphonesimulator -destination "generic/platform=iOS Simulator" \
  -derivedDataPath build/dd CODE_SIGNING_ALLOWED=NO OTHER_SWIFT_FLAGS=-DDATA_APP SWIFT_ACTIVE_COMPILATION_CONDITIONS="GPUI_NATIVE DEBUG" build 2>&1 | grep -E "error:" | head -30
[ -d "$APP" ] || { echo "no app"; exit 1; }
exit 0
fi
HUB=$3; U=$4; LINK="$HUB/#token=$(cat "$D/token")"
xcrun simctl boot "$U" 2>/dev/null; xcrun simctl bootstatus "$U" -b >/dev/null 2>&1
xcrun simctl terminate "$U" $B >/dev/null 2>&1; xcrun simctl uninstall "$U" $B >/dev/null 2>&1; xcrun simctl install "$U" "$APP" || exit 1
mig() { curl -s -m5 "$HUB/healthz" | /usr/bin/python3 -c 'import json,sys; m=json.load(sys.stdin).get("migrating") or {}; print(m.get("state","none"), m.get("phase",""), m.get("pct",0))'; }
# -app.alerts <false/> (a plist Bool; a bare NO is a string): no notification question over the shots (the app asks only while its alerts are on)
launch() { xcrun simctl terminate "$U" $B >/dev/null 2>&1; sleep 1; env SIMCTL_CHILD_BACKPLANE_LINK="$LINK" "$@" xcrun simctl launch "$U" $B -app.alerts '<false/>' >/dev/null; }
shot() { xcrun simctl io "$U" screenshot "$OUT/$1.png" >/dev/null 2>&1; echo "shot $1: $(mig)"; }
# (a) the banner while the lines are written, the bar part way
i=0; while :; do set -- $(mig); [ "$1" = running ] && [ "$2" = writing ] && [ "${3%.*}" -ge 15 ] && break; i=$((i+1)); [ $i -gt 600 ] && { echo "never reached writing: $*"; exit 1; }; sleep 1; done
launch; sleep 8; shot a-banner-running
# (b) the open card mid-import
launch SIMCTL_CHILD_BACKPLANE_MIGRATION_OPEN=1; sleep 8; shot b-card-running
# (c) the done card, open as it came
i=0; while [ "$(mig | cut -d' ' -f1)" = running ]; do i=$((i+1)); [ $i -gt 900 ] && { echo "never finished"; exit 1; }; sleep 2; done
sleep 20; shot c-card-done
# a fresh start: hello `migrating` brings the done card back as a banner, until Continue
launch; sleep 8; shot d-banner-done
# Continue (onboard.act ack) closes it; another start shows none: the hub was answered
launch SIMCTL_CHILD_BACKPLANE_MIGRATION_OPEN=1 SIMCTL_CHILD_BACKPLANE_MIGRATION_PRESS=Continue; sleep 14; shot e-after-continue
launch; sleep 8; shot f-relaunch-after-continue
xcrun simctl spawn "$U" log show --last 15m --predicate 'process == "Backplane"' --style compact 2>/dev/null | grep -E "BPMIG|WebSocket|nw_|Socket" | cut -c1-220 > "$OUT/bpmig.log"
xcrun simctl terminate "$U" $B >/dev/null 2>&1; xcrun simctl shutdown "$U"
