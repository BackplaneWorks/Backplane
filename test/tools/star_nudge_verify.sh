#!/bin/sh
# Serial validation and portable renderer artifacts; pass --focused after a
# recorded full check. Use the pinned compiler on PATH, without replacing it.
set -eu
export CHECK_JOBS=1 CHECK_SHARDS=1 TEST_JOBS=1
if [ "${1:-}" != --focused ]; then scripts/check.sh; fi
TEST_FILES='test/star_nudge_test.bend test/star_nudge_ui_test.bend' scripts/test.sh
mkdir -p build/star-nudge build/dev
bend test/fixtures/star-nudge.bend -o build/star-nudge/fixture.mjs >/dev/null
bend src/web/index.html -o build/dev/web >/dev/null
python3 scripts/js-unmarshal.py ../core/client.Ui build/dev/web/chunk-*.js
python3 scripts/js-native.py build/dev/web/chunk-*.js
