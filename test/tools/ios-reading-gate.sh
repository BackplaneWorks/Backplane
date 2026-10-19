#!/bin/sh
set -eu
. scripts/bendok.sh
mkdir -p build/ios-reading-final
bend_check src/mobile/app.bend > build/ios-reading-final/preflight.log 2>&1 || { cat build/ios-reading-final/preflight.log; exit 1; }
for t in test/ios_usage_test.bend test/ios_scroll_page_test.bend test/hubs_test.bend test/mhalves_test.bend; do
  bend "$t" > "build/ios-reading-final/$(basename "$t").log" 2>&1
  if grep -q '^FAIL\|^Error:' "build/ios-reading-final/$(basename "$t").log"; then cat "build/ios-reading-final/$(basename "$t").log"; exit 1; fi
  grep -q '^ok ' "build/ios-reading-final/$(basename "$t").log"
  echo "$t PASS"
done
scripts/check.sh > build/ios-reading-final/check-proof.log 2>&1 || { tail -60 build/ios-reading-final/check-proof.log; exit 1; }
echo 'check/proof PASS'
scripts/build-mobile.sh --js > build/ios-reading-final/bridge.log 2>&1
sha256sum src/mobile/hubs.bend src/mobile/screen.bend LAWS.bend PROOF.bend mobile/build/assets/bridge.js > build/ios-reading-final/inputs.sha256
echo 'PASS iOS shared usage/retained page/proof/production bridge'
