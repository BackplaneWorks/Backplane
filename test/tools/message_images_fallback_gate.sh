#!/bin/sh
# Static browser composition only: no app emit, service or installation.
set -eu
mkdir -p build/message-images-browser
cd build/message-images-browser
printf '%s\n' '{"private":true,"dependencies":{"playwright":"1.63.0"}}' >package.json
bun install --ignore-scripts
bun x playwright install chromium
cd ../..
export PLAYWRIGHT_MODULE="$PWD/build/message-images-browser/node_modules/playwright-core"
BACKPLANE_CHROME=$(node -e 'console.log(require(process.env.PLAYWRIGHT_MODULE).chromium.executablePath())')
export BACKPLANE_CHROME
node --test desktop/test/shot-missing.test.cjs
