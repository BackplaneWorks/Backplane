#!/bin/sh
# Small policy/browser/native compile checks. Never builds or installs a hub.
set -eu
cd "$(dirname "$0")/../.."
export BEND_NO_TELEMETRY=1 BUN_JSC_forceRAMSize=4294967296
mkdir -p build/reporting-evidence
bend src/core/issue-report.bend -o build/issue-report-policy.mjs
export BACKPLANE_PLAYWRIGHT_PATH="${BACKPLANE_PLAYWRIGHT_PATH:-$HOME/.bun/install/cache/playwright-core@1.64.1@@@1}"
export BACKPLANE_CHROME="${BACKPLANE_CHROME:-/usr/bin/google-chrome}"
export BACKPLANE_REPORT_EVIDENCE="$PWD/build/reporting-evidence"
node --test desktop/test/issue-report-popup.test.cjs desktop/test/issue-report.test.cjs desktop/test/improvements-popup.test.cjs desktop/test/connection-log.test.cjs
export JAVA_HOME="${JAVA_HOME:-/usr/lib/jvm/java-17-openjdk-amd64}"
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
cd mobile/android
./gradlew --offline --no-daemon --max-workers=2 -Dorg.gradle.jvmargs=-Xmx1536m :app:compileDebugKotlin
