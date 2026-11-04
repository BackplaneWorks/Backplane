#!/bin/sh
# Build the iOS app on a Mac over SSH (bend runs here; Xcode runs there).
#   scripts/build-ios.sh [sim|device|archive]     (default sim)
#   BACKPLANE_MAC=hs-mac-mini              the ssh host
#   BACKPLANE_MAC_DIR=backplane-ios        the copy on the Mac (one per
#                                          checkout when several build at once)
#   sim      a Simulator build, installed and launched on a booted iPhone
#   device   a development build installed on the iPhone connected to the Mac
#   archive  a signed App Store archive (needs mobile/ios/signing.env there)
# On the Mac itself, in a whole checkout: mobile/ios/build.sh (docs/ios-build.md).
set -eu
cd "$(dirname "$0")/.."
MAC=${BACKPLANE_MAC:-hs-mac-mini}
MODE=${1:-sim}
DIR=${BACKPLANE_MAC_DIR:-backplane-ios}
[ -n "${BACKPLANE_SKIP_JS:-}" ] || scripts/build-mobile.sh --js
cp mobile/build/assets/bridge.js mobile/ios/Backplane/bridge.js
cp mobile/shell/backplane-app.js mobile/ios/Backplane/backplane-app.js
rsync -a --delete --exclude build/ --exclude authorize-mac.sh --exclude signing.env --exclude '*.p8' \
  mobile/ios/ "$MAC:$DIR/"
# the copy on the Mac is mobile/ios alone: build.sh builds there with the two scripts copied above
ssh "$MAC" "cd $DIR && sh build.sh $MODE"
