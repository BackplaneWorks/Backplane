#!/bin/sh
# Check the iOS app's dictation edits (mobile/ios/Backplane/VoiceEdit.swift) against test/fixtures/voice-edits.json, the hub's own edits, on the Mac:
# swiftc compiles VoiceEdit.swift with mobile/ios/Tests/VoiceEditTests.swift (no app, no simulator) and runs it.
# usage: scripts/mobile/ios-voice-edit-test.sh        (needs the ssh wrapper build/gpui/mac-ssh of the main checkout)
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd); MAIN=${MAIN:-$(dirname "$(git -C "$(dirname "$0")" rev-parse --path-format=absolute --git-common-dir)")}; M=${M:-$MAIN/build/gpui/mac-ssh}; D=${D:-bp-voice-edit-test}
$M hs-mac-mini "mkdir -p $D" || exit 1
RSYNC_RSH=$M rsync -a $ROOT/mobile/ios/Backplane/VoiceEdit.swift $ROOT/test/fixtures/voice-edits.json hs-mac-mini:$D/ || exit 1
# top-level code must be main.swift when swiftc gets several files
RSYNC_RSH=$M rsync -a $ROOT/mobile/ios/Tests/VoiceEditTests.swift hs-mac-mini:$D/main.swift || exit 1
$M hs-mac-mini "export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer; cd $D && xcrun swiftc -O -o voice-edit-test VoiceEdit.swift main.swift && ./voice-edit-test voice-edits.json; r=\$?; cd && rm -rf $D; exit \$r"
