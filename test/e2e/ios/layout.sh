#!/bin/sh
# iOS size matrix (docs/e2e/SIZES.md): the app's screens on each simulator in sizes.json (phones.ios), portrait and landscape, at several Dynamic Type sizes;
# the XCUITest LayoutAudit (UITests/LayoutTests.swift) records each screen's accessibility tree and a screenshot, test/e2e/layout/ios_check.py judges them.
#   layout.sh [--sims "iPhone 16,iPad (10th generation)"] [--orient portrait,landscape] [--text L,XXXL,AX3]      (on the Mac, under the mac-heavy lock; never on a phone)
# env: HUB_HOST HUB_PORT HUB_TOKEN_FILE (a fixture hub, serve-fixture.mjs on h@backplane), OUT (default ./layout-out), APP (the built Backplane.app), E2E_DD.
# Writes $OUT/<sim>/<orient>-<text>/layout-<screen>.{json,png}, $OUT/findings.json (ios_check's), $OUT/report.md; exit 1 when a finding is new (ios-known.json).
# NOT RUN on this branch: written on a Linux desktop; the Mac mini's jobs wait for its owner (see docs/e2e/SIZES.md).
set -u
export DEVELOPER_DIR=${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}
HERE=$(cd "$(dirname "$0")" && pwd); LAYOUT=$HERE/../layout
: "${HUB_HOST:?}" "${HUB_PORT:?}" "${HUB_TOKEN_FILE:?}"
OUT=${OUT:-$PWD/layout-out}; mkdir -p "$OUT"
[ -n "${APP_ID:-}" ] && export TEST_RUNNER_APP_ID=$APP_ID   # the built app's bundle id when it is not dev.backplane.gpui
APP=${APP:-$HOME/bp-gpui-ios/ios/build/dd/Build/Products/Debug-iphonesimulator/Backplane.app}
DD=${E2E_DD:-$HOME/bp-gpui-ios/e2e-dd}
SIMS=$(python3 -c "import json;print(','.join(d['name'] for d in json.load(open('$LAYOUT/sizes.json'))['phones']['ios']))")
ORIENT=portrait,landscape
TEXT=L,XXXL,AX3     # Dynamic Type: default, the largest standard size, the third accessibility size
while [ $# -gt 0 ]; do case $1 in --sims) SIMS=$2; shift 2;; --orient) ORIENT=$2; shift 2;; --text) TEXT=$2; shift 2;; *) echo "unknown $1"; exit 2;; esac; done
cat_of() { case $1 in XS) echo UICTContentSizeCategoryXS;; L) echo UICTContentSizeCategoryL;; XXXL) echo UICTContentSizeCategoryXXXL;; AX3) echo UICTContentSizeCategoryAccessibilityXXL;; AX5) echo UICTContentSizeCategoryAccessibilityXXXL;; *) echo "$1";; esac; }
sim_of() { case $1 in XS) echo extra-small;; L) echo large;; XXXL) echo extra-extra-extra-large;; AX3) echo accessibility-extra-extra-large;; AX5) echo accessibility-extra-extra-extra-large;; *) echo "$1";; esac; }
xcodebuild build-for-testing -quiet -project "$HERE/E2E.xcodeproj" -scheme BackplaneE2E -sdk iphonesimulator -destination "generic/platform=iOS Simulator" \
  -derivedDataPath "$DD" CODE_SIGNING_ALLOWED=NO > "$OUT/build.log" 2>&1 || { tail -30 "$OUT/build.log"; echo "build-for-testing failed"; exit 1; }
OLDIFS=$IFS; IFS=,
for sim in $SIMS; do
  IFS=$OLDIFS
  udid=$(xcrun simctl list devices available | grep -F "    $sim (" | head -1 | sed -E 's/.*\(([0-9A-F-]{36})\).*/\1/')
  [ -n "$udid" ] || { echo "no simulator named $sim"; continue; }
  xcrun simctl boot "$udid" 2>/dev/null; xcrun simctl bootstatus "$udid" -b >/dev/null 2>&1
  xcrun simctl install "$udid" "$APP" || { echo "install on $sim failed"; continue; }
  tag=$(echo "$sim" | tr -c 'A-Za-z0-9\n' '-')
  for o in $(echo $ORIENT | tr , ' '); do for t in $(echo $TEXT | tr , ' '); do
    d=$OUT/$tag/$o-$t; mkdir -p "$d"
    xcrun simctl ui "$udid" content_size "$(sim_of $t)" >/dev/null 2>&1
    TEST_RUNNER_HUB_HOST=$HUB_HOST TEST_RUNNER_HUB_PORT=$HUB_PORT TEST_RUNNER_HUB_TOKEN_FILE=$HUB_TOKEN_FILE TEST_RUNNER_OUT=$d \
    TEST_RUNNER_LAYOUT_ORIENT=$o TEST_RUNNER_LAYOUT_TEXT=$(cat_of $t) \
    xcodebuild test-without-building -resultBundlePath "$d/layout.xcresult" -project "$HERE/E2E.xcodeproj" -scheme BackplaneE2E -sdk iphonesimulator \
      -destination "platform=iOS Simulator,id=$udid" -derivedDataPath "$DD" CODE_SIGNING_ALLOWED=NO -only-testing:"BackplaneE2E/LayoutAudit/testLayout" > "$d/xcodebuild.log" 2>&1
    echo "$sim $o $t: xcodebuild exit $?"
  done; done
  xcrun simctl shutdown "$udid" 2>/dev/null
  IFS=,
done
IFS=$OLDIFS
python3 "$LAYOUT/ios_check.py" "$OUT" > "$OUT/findings.json"; rc=$?
python3 - "$OUT" <<'PY'
import json, sys
o = sys.argv[1]; r = json.load(open(o + '/findings.json'))
L = ['## Size matrix on iOS', '', '%d new, %d known' % (len(r['new']), len(r['known'])), '']
L += ['- `%s` %s: %s (%s) [%s]' % (f['rule'], f['scenario'], f['text'][:70], f['detail'], f['file']) for f in r['new']]
open(o + '/report.md', 'w').write('\n'.join(L) + '\n')
PY
exit $rc
