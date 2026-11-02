#!/bin/sh
# Source qualification only. No install, credentials, GitHub issue or release.
set -eu
cd "$(dirname "$0")/../.."
export BEND_NO_TELEMETRY=1 BUN_JSC_forceRAMSize=4294967296
export BUN_JSC_maxPerThreadStackUsage=536870912 CHECK_JOBS=1 CHECK_SHARDS=1 TEST_JOBS=1
ulimit -s 1048576 2>/dev/null || true
out=build/reporting-evidence
mkdir -p "$out"
. scripts/bendok.sh
run() {
  name=$1;shift
  if "$@" > "$out/$name.log" 2>&1; then
    printf '%s PASS\n' "$name" | tee -a "$out/gates.txt"
  else
    code=$?;printf '%s FAIL %s\n' "$name" "$code" | tee -a "$out/gates.txt"
    tail -60 "$out/$name.log"
    return "$code"
  fi
}
phase=${1:-source}
case "$phase" in source|build) ;; *) echo 'usage: reporting-gates.sh source|build' >&2;exit 2;; esac
if [ "$phase" = source ]; then
: > "$out/gates.txt"
# Catch an integration error before the repository manifest's diagnostic fallback.
run server-typecheck bend_check src/server/server.bend
run android-compile env JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 ANDROID_HOME="$HOME/Android/Sdk" sh -c 'cd mobile/android && ./gradlew --offline --no-daemon --max-workers=2 -Dorg.gradle.jvmargs=-Xmx1536m :app:compileDebugKotlin'
run focused env TEST_FILES='test/issue_report_test.bend test/report_writer_test.bend test/improvements_test.bend test/diagnostics_backend_test.bend' scripts/test.sh
run check-proof scripts/check.sh
printf '%s\n' 'Source checks and proof complete; production build is a separate admitted phase.'
exit 0
fi
run policy-emit bend src/core/issue-report.bend -o build/issue-report-policy.mjs
if [ -z "${BACKPLANE_PLAYWRIGHT_PATH:-}" ] && [ ! -d desktop/node_modules/playwright-core ]; then
  BACKPLANE_PLAYWRIGHT_PATH=$(python3 -c 'from pathlib import Path; print(next((Path.home()/".bun/install/cache").glob("playwright-core@1.64.1*/package.json")).parent)')
  export BACKPLANE_PLAYWRIGHT_PATH
fi
if [ -z "${BACKPLANE_CHROME:-}" ] && command -v google-chrome >/dev/null 2>&1; then
  export BACKPLANE_CHROME=$(command -v google-chrome)
fi
export BACKPLANE_REPORT_EVIDENCE="$PWD/$out"
run desktop-web node --test desktop/test/improvements.test.cjs desktop/test/improvements-popup.test.cjs desktop/test/improvements-capture-csp.test.cjs desktop/test/issue-report.test.cjs desktop/test/issue-report-popup.test.cjs desktop/test/connection-log.test.cjs desktop/test/host-thin-startup.test.cjs desktop/test/thin-upload.test.cjs desktop/test/browser-preload.test.cjs desktop/test/attachment-identity.test.cjs
# Full backend fault/rotation suite was run separately; keep a receipt here.
run backend-storage node --test test/tools/diagnostics_backend_test.cjs
run web-build bend src/web/index.html -o build/reporting-web
run web-native python3 scripts/js-unmarshal.py ../core/client.Ui build/reporting-web/chunk-*.js
run web-native-strings python3 scripts/js-native.py build/reporting-web/chunk-*.js
run mobile-js scripts/build-mobile.sh --js
if [ -z "${JAVA_HOME:-}" ]; then
  if [ -d "$HOME/.local/opt/jdk17" ]; then export JAVA_HOME="$HOME/.local/opt/jdk17";else export JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64;fi
fi
run native-models env JAVA_HOME="$JAVA_HOME" python3 mobile/tests/improvements/check.py
run native-adapters node mobile/tests/improvements/bridge.mjs
run hub-build scripts/build-js.sh src/server/main.bend build/reporting-hub
run wire-build bend test/wire/index.html -o build/reporting-wire
run hub-e2e bun test/tools/reporting_e2e.ts build/reporting-hub build/reporting-wire
run diff-check git diff --check
printf '%s\n' 'Source qualification finished. No install, external issue or credential change.'
