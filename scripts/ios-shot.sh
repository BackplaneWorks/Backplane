#!/bin/sh
# ios-shot.sh UDID NAME WAIT [run.sh args...]: launch, wait, screenshot ~/bp-gpui-ios/shots/NAME.png
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
U=$1; N=$2; W=$3; shift 3
sh ~/bp-gpui-ios/ios-run.sh "$U" "$@" >/dev/null
sleep "$W"
xcrun simctl io "$U" screenshot ~/bp-gpui-ios/shots/$N.png 2>&1 | tail -1
