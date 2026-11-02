#!/bin/sh
# A source-built web bundle and law proof for code-copy.cjs acceptance.
# Run on Boat (--size large); no service or installed user home is modified.
set -eu
cd "$(dirname "$0")/../.."
[ "${1:-}" = --locked ] || exec scripts/bp-lock.sh bend -- sh "$0" --locked
mkdir -p build/code-copy
ulimit -s 1048576
export BUN_JSC_maxPerThreadStackUsage=536870912 BEND_NO_TELEMETRY=1
. scripts/bendok.sh
if [ "${CODE_COPY_SKIP_PROOF:-0}" != 1 ]; then
  rc=0
  bend PROOF.bend > build/code-copy/proof.log 2>&1 || rc=$?
  if ! bend_verdict "$rc" "$(cat build/code-copy/proof.log)"; then cat build/code-copy/proof.log;exit 1;fi
  tail -n 4 build/code-copy/proof.log
fi
rc=0
bend src/web/app.bend --check-only > build/code-copy/web-check.log 2>&1 || rc=$?
if ! bend_verdict "$rc" "$(cat build/code-copy/web-check.log)"; then cat build/code-copy/web-check.log;exit 1;fi
bend src/web/index.html -o build/code-copy/web
python3 scripts/js-unmarshal.py ../core/client.Ui build/code-copy/web/chunk-*.js
python3 scripts/js-native.py build/code-copy/web/chunk-*.js
cp src/web/sw.js build/code-copy/web/sw.js
