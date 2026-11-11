#!/bin/sh
# The iOS app's unit tests on a Mac itself (CI: the ios job): each swiftc-compiles one app source with its test (mobile/ios/Tests, a main.swift)
# and runs it, no app and no simulator. The ios-*-test.sh scripts here do the same through the Mac mini over ssh.
# First the catalog gates: every iOS word goes through i18n/*.msg and the generated TrCatalog.swift is current (docs/i18n/README.md).
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd); S=$ROOT/mobile/ios/Backplane; T=$ROOT/mobile/ios/Tests
python3 "$ROOT/scripts/i18n-check.py"
python3 "$ROOT/scripts/i18n-ios.py" --check
python3 "$ROOT/scripts/check-ios-i18n.py"
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
run badge-symbols BadgeSymbolsTests.swift "$ROOT/native/host/src/present/badge.rs" BadgeSymbols.swift
run thread-find ThreadFindTests.swift "" ThreadFind.swift
run voice-edit VoiceEditTests.swift "$ROOT/test/fixtures/voice-edits.json" VoiceEdit.swift
run plot-parts PlotPartsTests.swift "" Plot.swift PlotParts.swift
run wire-json WireJSONTests.swift "" WireJSON.swift DataModels.swift ThreadGroups.swift TermModels.swift Outbox.swift
run term-stream TermStreamTests.swift "" TermStream.swift
run vt VtTests.swift "" Vt.swift
run term-card TermCardTests.swift "" Vt.swift TermCard.swift
run migration MigrationTests.swift "" Migration.swift
S=$ROOT/mobile/ios/Shared; run tr TrTests.swift "" Tr.swift TrCatalog.swift; S=$ROOT/mobile/ios/Backplane
run viewer-backdrop ViewerBackdropTests.swift "" ViewerBackdrop.swift
run browser-input BrowserInputTests.swift "$ROOT/test/mobile/browser_input_cases.json" BrowserInput.swift
run google-loop GoogleLoopTests.swift "" GoogleLoopLogic.swift
run swarm-model SwarmModelTests.swift "" SwarmModel.swift
run section-menu SectionMenuTests.swift "" SectionMenu.swift
run mcp-page McpPageTests.swift "" McpPage.swift
sh "$ROOT/scripts/mobile/ios-outbox-test.sh" && echo "ok outbox"
