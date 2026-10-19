#!/bin/sh
set -eu
# On macOS, compile the production helper definitions directly. The optional
# kept-state lifetime probe returns commands but never executes them.
root=$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)
cd "$root"
out=${IOS_NATIVE_TEST_OUT:-build/ios-native-helpers}
mkdir -p "$out"
xcrun swiftc -swift-version 6 -parse-as-library mobile/ios/Backplane/HubConnections.swift test/tools/ios_connection_observation_test.swift -o "$out/connection-test"
"$out/connection-test"
cat mobile/ios/Backplane/WirePriority.swift mobile/ios/Backplane/ScreenCache.swift test/tools/ios_native_helpers_test.swift > "$out/helpers.swift"
xcrun swiftc "$out/helpers.swift" -o "$out/helpers"
"$out/helpers"
if [ "$#" = 2 ]; then
  python3 - "$out/worker.swift" <<'PY'
from pathlib import Path
import sys
worker = Path('mobile/ios/Backplane/Engine.swift').read_text().split('// The Bend client (bridge.js)')[0]
Path(sys.argv[1]).write_text(worker + Path('test/tools/ios_worker_lifetime.swift').read_text())
PY
  xcrun swiftc "$out/worker.swift" -framework JavaScriptCore -o "$out/worker"
  JSC_useJIT=0 "$out/worker" "$1" "$2" > "$out/lifetime.log"
  cat "$out/lifetime.log"
fi
