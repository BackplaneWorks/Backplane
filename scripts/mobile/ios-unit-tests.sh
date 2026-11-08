#!/bin/sh
# The iOS app's unit tests on a Mac itself (CI: the ios job): each swiftc-compiles one app source with its test (mobile/ios/Tests, a main.swift)
# and runs it, no app and no simulator. The ios-*-test.sh scripts here do the same through the Mac mini over ssh.
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd); S=$ROOT/mobile/ios/Backplane; T=$ROOT/mobile/ios/Tests
W=$(mktemp -d); trap 'rm -rf "$W"' EXIT
run() { # name, args for the test, then the app sources
  name=$1; test=$2; shift 2; args=$1; shift
  mkdir "$W/$name"; cp "$T/$test" "$W/$name/main.swift"
  for f in "$@"; do cp "$S/$f" "$W/$name/"; done
  (cd "$W/$name" && xcrun swiftc -O -o t *.swift && ./t $args)
  echo "ok $name"
}
run hub-list HubListTests.swift "$ROOT/test/mobile/hubs_cases.json" HubList.swift
run thread-groups ThreadGroupsTests.swift "" ThreadGroups.swift
run voice-edit VoiceEditTests.swift "$ROOT/test/fixtures/voice-edits.json" VoiceEdit.swift
run plot-parts PlotPartsTests.swift "" Plot.swift PlotParts.swift
run wire-json WireJSONTests.swift "" WireJSON.swift DataModels.swift ThreadGroups.swift TermModels.swift
run term-stream TermStreamTests.swift "" TermStream.swift
run vt VtTests.swift "" Vt.swift
run term-card TermCardTests.swift "" Vt.swift TermCard.swift
run migration MigrationTests.swift "" Migration.swift
