#!/bin/sh
# Serial, isolated source checks/builds. No live account or application home.
set -eu
ulimit -s 1048576 2>/dev/null || true
export BUN_JSC_maxPerThreadStackUsage=536870912 BUN_JSC_forceRAMSize=4294967296 BEND_NO_TELEMETRY=1
mkdir -p build/ui-removals
if [ -f test/fixtures/ui-removals-before.tar.gz ]; then
  mkdir -p build/ui-removals/before
  tar -xzf test/fixtures/ui-removals-before.tar.gz -C build/ui-removals/before
  cp test/tools/ui_removals_fixture.bend build/ui-removals/before/test/tools/
  (cd build/ui-removals/before && bend test/tools/ui_removals_fixture.bend -o ../page-before.mjs) > build/ui-removals/before.log 2>&1 || { cat build/ui-removals/before.log; exit 1; }
  rm -rf build/ui-removals/before
fi
scripts/check.sh > build/ui-removals/check.log 2>&1 || { tail -100 build/ui-removals/check.log; exit 1; }
cat build/ui-removals/check.log
TEST_FILES='test/allowance_test.bend test/allowance_accounts_test.bend test/allowance_ui_test.bend test/allowance_publication_test.bend test/phone_composer_test.bend' scripts/test.sh > build/ui-removals/targeted.log 2>&1 || { tail -100 build/ui-removals/targeted.log; exit 1; }
cat build/ui-removals/targeted.log
bend test/tools/ui_removals_fixture.bend -o build/ui-removals/page-after.mjs > build/ui-removals/after.log 2>&1 || { cat build/ui-removals/after.log; exit 1; }
bend test/tools/composer_bar_fixture.bend > build/ui-removals/composer.json
BACKPLANE_VERSION_STAMP=0.13.1-dev.ui-removals scripts/build-js.sh src/server/main.bend build/ui-removals/backplane-serve
rm -rf build/ui-removals/web
bend src/web/index.html -o build/ui-removals/web > build/ui-removals/web.log 2>&1 || { cat build/ui-removals/web.log; exit 1; }
python3 scripts/js-unmarshal.py ../core/client.Ui build/ui-removals/web/chunk-*.js
python3 scripts/js-native.py build/ui-removals/web/chunk-*.js
cp src/web/sw.js build/ui-removals/web/sw.js
echo 'ok UI removals: full typecheck/proofs, allowance/composer tests and candidate builds'
