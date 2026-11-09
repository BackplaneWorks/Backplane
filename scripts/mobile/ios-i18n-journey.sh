#!/bin/sh
# The iOS data app in Japanese (test/e2e/ios data-i18n) on a simulator of its own, against the demo hub of scripts/mobile/data-hub.sh on this machine's
# tailnet address: first with the simulator in Japanese and the setting on Automatic (unpaired: the pairing screen, then a real pairing), then with the
# simulator in English and Settings › Language › 日本語 chosen by hand. Screenshots land in build/mobile/ios-i18n; the simulator is deleted at the end.
# usage: scripts/mobile/ios-i18n-journey.sh       Start with work_run {lock: "mac-heavy"}; builds the app on the Mac first (scripts/mobile/ios-check.sh).
#   HUBBIN=<a backplane-hub> (default the published build in ~/.backplane-gpui-dev/CURRENT)   PORT (3871)   HOST (this machine's tailnet IPv4)
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd); MAIN=${MAIN:-$(dirname "$(git -C "$(dirname "$0")" rev-parse --path-format=absolute --git-common-dir)")}; M=${M:-$MAIN/build/gpui/mac-ssh}
D=${D:-bp-i18n-ios-journey}; OUT=$ROOT/build/mobile/ios-i18n; rm -rf "$OUT"; mkdir -p "$OUT"
. "$ROOT/scripts/lib-bptmp.sh"
export PORT=${PORT:-3871} HOST=${HOST:-$(tailscale ip -4 | head -1)} H=${H:-$BP_TMP/bp-i18n-ios-hub}
export HUBBIN=${HUBBIN:-$HOME/.backplane-gpui-dev/builds/$(cat "$HOME/.backplane-gpui-dev/CURRENT")/backplane-hub}
[ -x "$HUBBIN" ] || { echo "no hub at $HUBBIN"; exit 1; }
M=$M D=$D "$ROOT/scripts/mobile/ios-check.sh" | tail -8
$M hs-mac-mini "test -d ~/$D/ios/build/dd/Build/Products/Debug-iphonesimulator/Backplane.app" || { echo "the app did not build"; exit 1; }
sh "$ROOT/scripts/mobile/data-hub.sh" start || exit 1
trap 'sh "$ROOT/scripts/mobile/data-hub.sh" stop; rm -rf "$H"' EXIT
printf %s "$(cat $H/home/token)" > $OUT/token; RSYNC_RSH=$M rsync -a $OUT/token hs-mac-mini:$D/token; rm -f $OUT/token
RSYNC_RSH=$M rsync -a --delete $ROOT/test/e2e/ios/ hs-mac-mini:$D/e2e/
$M hs-mac-mini "export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
U=\$(xcrun simctl create bp-i18n-ios 'iPhone 17' com.apple.CoreSimulator.SimRuntime.iOS-26-5) || exit 1; echo sim \$U
lang() { xcrun simctl shutdown \$U >/dev/null 2>&1; xcrun simctl boot \$U; xcrun simctl bootstatus \$U -b >/dev/null 2>&1
  xcrun simctl spawn \$U defaults write -g AppleLanguages -array \$1 && xcrun simctl spawn \$U defaults write -g AppleLocale \$2
  xcrun simctl shutdown \$U; xcrun simctl boot \$U; xcrun simctl bootstatus \$U -b >/dev/null 2>&1; }
run() { cd ~/$D/e2e && I18N_MODE=\$1 SIM_UDID=\$U OUT=\$HOME/$D/out HUB_HOST=$HOST HUB_PORT=$PORT HUB_TOKEN_FILE=\$HOME/$D/token \
  APP=\$HOME/$D/ios/build/dd/Build/Products/Debug-iphonesimulator/Backplane.app E2E_DD=\$HOME/$D/e2e-dd sh ./run.sh data-i18n; cat ~/$D/out/data-i18n.assert.txt
  mkdir -p ~/$D/out/\$1 && mv ~/$D/out/data-i18n* ~/$D/out/\$1/; }
rm -rf ~/$D/out
lang ja-JP ja_JP; run auto
lang en-US en_US; run settings
xcrun simctl shutdown \$U; xcrun simctl delete \$U"
RSYNC_RSH=$M rsync -a --exclude '*.xcresult' --exclude '*.mp4' hs-mac-mini:$D/out/ $OUT/
for m in auto settings; do python3 -c "
import json;j=json.load(open('$OUT/$m/data-i18n.json'));print('$m', j['status'], j.get('notes', [])[:-1]);[print(' ', s['do'], s['ok']) for s in j['metrics']['steps']]"; done
ls $OUT/*/*.png
