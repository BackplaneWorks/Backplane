#!/bin/sh
# iOS simulator app build on the Mac, cold (fresh derived data) and warm (touch one Swift file, same derived data). Run on the Mac (sh -s) under mac-heavy.
set -u
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer PATH=/usr/bin:/bin:/usr/sbin:/sbin
M=$HOME/bp-gpui-measure; O=$M/measure; mkdir -p $O; S=$O/summary.txt; : > $S
cd $M/mobile/ios || { echo "no project" >> $S; exit 1; }
echo "machine: $(hostname), $(sysctl -n hw.ncpu) cpus, $(sysctl -n hw.memsize) bytes RAM, $(xcodebuild -version | tr '\n' ' ')" >> $S
step() { n=$1; shift; /usr/bin/time -l -o $O/$n.time "$@" > $O/$n.log 2>&1; rc=$?
  real=$(awk '/real/{print $1}' $O/$n.time); rss=$(awk '/maximum resident set size/{print $1}' $O/$n.time)
  echo "$n exit $rc wall_s $real maxrss_bytes $rss" >> $S; }
DD=$(mktemp -d)
BUILD="xcodebuild -quiet -project Backplane.xcodeproj -scheme Backplane -configuration Debug -sdk iphonesimulator -destination generic/platform=iOS\ Simulator -derivedDataPath $DD build"
step ios-cold sh -c "xcodebuild -quiet -project Backplane.xcodeproj -scheme Backplane -configuration Debug -sdk iphonesimulator -destination 'generic/platform=iOS Simulator' -derivedDataPath $DD build"
F=$(find Backplane -name '*.swift' | head -1); touch "$F"
step ios-warm sh -c "xcodebuild -quiet -project Backplane.xcodeproj -scheme Backplane -configuration Debug -sdk iphonesimulator -destination 'generic/platform=iOS Simulator' -derivedDataPath $DD build"
rm -rf $DD; cat $S
