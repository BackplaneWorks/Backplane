#!/bin/sh
# Install and launch on a simulator. Usage: ios-run.sh UDID [ACTS] [SELECT]   (env: JIT=0 for no-JIT JSC)
U=$1; ACTS=${2:-ob-skip}; SEL=${3:-}
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
APP=${APP:-~/bp-gpui-ios/ios/build/dd/Build/Products/Debug-iphonesimulator/Backplane.app}
xcrun simctl bootstatus "$U" -b >/dev/null 2>&1
xcrun simctl terminate "$U" dev.backplane.gpui >/dev/null 2>&1
xcrun simctl install "$U" "$APP" || exit 1
TOK=$(cat ${TOKFILE:-~/bp-gpui-secrets/hub-token})
export SIMCTL_CHILD_BACKPLANE_LINK="${HUBURL:-http://desktop.tail9e0cf2.ts.net:3799/}#token=$TOK"
export SIMCTL_CHILD_BACKPLANE_ACTS="$ACTS"
[ -n "$SEL" ] && export SIMCTL_CHILD_BACKPLANE_SELECT="$SEL"
[ -n "$GEOM" ] && export SIMCTL_CHILD_BACKPLANE_GEOM=1
[ -n "$NOLINK" ] && unset SIMCTL_CHILD_BACKPLANE_LINK
[ -n "$VIEW" ] && export SIMCTL_CHILD_BACKPLANE_VIEW="$VIEW"
for kv in $EXTRAENV; do export "SIMCTL_CHILD_$kv"; done
[ "${JIT:-1}" = 0 ] && export SIMCTL_CHILD_JSC_useJIT=0
xcrun simctl launch "$U" dev.backplane.gpui $LAUNCHARGS
