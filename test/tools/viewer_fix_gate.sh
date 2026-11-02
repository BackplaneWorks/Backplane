#!/bin/sh
# Focused viewer regressions; runs in an isolated checkout on Boat.
set -eu
mkdir -p build/viewer-fix
node --test desktop/test/viewer-intent.test.cjs desktop/test/thin-host.test.cjs
# .js emits an executable entry point; .mjs is an embeddable module.
bend test/native/viewer_subscription_stamp_probe.bend -o build/viewer-fix/stamp-probe.js
bun build --compile --no-compile-autoload-dotenv --no-compile-autoload-bunfig build/viewer-fix/stamp-probe.js --outfile build/viewer-fix/stamp-probe
bun test/tools/viewer_subscription_stamp_test.mjs build/viewer-fix/stamp-probe
