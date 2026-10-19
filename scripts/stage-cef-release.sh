#!/bin/sh
# Explicit Linux x64 release staging, inside the caller's build budget.
set -eu
cd "$(dirname "$0")/.."
[ "$(uname -sm)" = 'Linux x86_64' ] || { echo 'native CEF release supports Linux x86_64 only' >&2; exit 1; }
[ -x dist/backplane-browser ] || { echo 'build the browser helper first' >&2; exit 1; }
scripts/build-cef.sh build/cef
revision=$(cat tools/cef/host.cc tools/cef/CMakeLists.txt scripts/build-cef.sh | sha256sum | cut -d' ' -f1)
sha256sum dist/backplane-browser | cut -d' ' -f1 > dist/backplane-browser.stamp
rm -rf dist/browser-runtime
python3 scripts/cef-runtime.py stage dist build/cef/Release build/cef/Release/backplane-cef dist/backplane-browser dist/backplane-browser.stamp "$revision"
ln -sfn browser-runtime/cef dist/cef
