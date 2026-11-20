#!/bin/bash
# Reply on an iOS simulator on the Mac, headless (simctl only), through real taps (XCUITest ReplyRows, UITests/ReplyTests.swift) against the copy rows'
# private hub (copy-hub.mjs, thread t5) on a temp home here: test/e2e/ios/reply-rows.sh HUB_BIN [OUT]. Take the work_run lock mac-heavy around it.
ROOT=$(cd "$(dirname "$0")/../../.." && pwd)
CLASS=ReplyRows MAC_DIR=${MAC_DIR:-bp-reply-ios} SIM=${SIM:-bp-reply-rows} exec "$ROOT/test/e2e/ios/copy-rows.sh" "${1:?hub binary}" "${2:-$ROOT/build/e2e-ios-reply}" test_reply_rows
