#!/bin/sh
# The iOS outbox's XCTest (mobile/ios/Tests/OutboxTests.swift against mobile/ios/Backplane/Outbox.swift): a throwaway Swift package of the two, `swift test`.
#   scripts/mobile/ios-outbox-test.sh          on a Mac itself (CI, or the Mac mini's shell)
#   scripts/mobile/ios-outbox-test.sh --mac    from this machine, on the Mac mini over ssh (the ssh wrapper build/gpui/mac-ssh of the main checkout)
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
if [ "${1:-}" = --mac ]; then
  MAIN=${MAIN:-$(dirname "$(git -C "$(dirname "$0")" rev-parse --path-format=absolute --git-common-dir)")}; M=${M:-$MAIN/build/gpui/mac-ssh}; D=${D:-bp-outbox-test}
  $M hs-mac-mini "rm -rf $D && mkdir -p $D/mobile/ios/Backplane $D/mobile/ios/Tests $D/scripts/mobile"
  RSYNC_RSH=$M rsync -a "$ROOT/mobile/ios/Backplane/Outbox.swift" hs-mac-mini:$D/mobile/ios/Backplane/
  RSYNC_RSH=$M rsync -a "$ROOT/mobile/ios/Tests/OutboxTests.swift" hs-mac-mini:$D/mobile/ios/Tests/
  RSYNC_RSH=$M rsync -a "$0" hs-mac-mini:$D/scripts/mobile/
  exec $M hs-mac-mini "export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; sh $D/scripts/mobile/ios-outbox-test.sh; r=\$?; rm -rf $D; exit \$r"
fi
W=$(mktemp -d); trap 'rm -rf "$W"' EXIT
mkdir -p "$W/Sources/OutboxKit" "$W/Tests/OutboxTests"
cp "$ROOT/mobile/ios/Backplane/Outbox.swift" "$W/Sources/OutboxKit/"
cp "$ROOT/mobile/ios/Tests/OutboxTests.swift" "$W/Tests/OutboxTests/"
cat > "$W/Package.swift" <<'EOF'
// swift-tools-version:5.9
import PackageDescription
let package = Package(name: "OutboxKit", targets: [.target(name: "OutboxKit"), .testTarget(name: "OutboxTests", dependencies: ["OutboxKit"])])
EOF
cd "$W"; r=0; swift test > "$W/log" 2>&1 || r=$?
grep -vE "^(Building|Compiling|Build complete|Write|\[[0-9]+/[0-9]+\])" "$W/log"; exit $r
