#!/bin/sh
# Uploaded and run by boat-job.sh (never run here): the debug APK. Installs its own JDK 17 and Android SDK inside the job.
# usage (inside the sandbox): sh scripts/gpui/android-build-boat.sh [nosidebar]
set -eu
[ "${BACKPLANE_HEAVY_BOAT_SANDBOX:-}" = 1 ] || { echo 'APK builds run only on Boat' >&2; exit 75; }
CAP=8589934592
CHECK='g=$(cut -d: -f3 /proc/self/cgroup); b=/sys/fs/cgroup$g; mx=$(cat "$b/memory.max"); sw=$(cat "$b/memory.swap.max"); case "$mx" in ""|*[!0-9]*) exit 1;; esac; [ "$mx" -gt 0 ] && [ "$mx" -le 8589934592 ] && [ "$sw" = 0 ]'
if ! sh -c "$CHECK"; then
 [ "${GPUI_CAP_TRIED:-}" != 1 ] || { echo "kernel memory cap unavailable; refusing" >&2; exit 75; }
 export GPUI_CAP_TRIED=1
 exec systemd-run --user --scope -q -p MemoryAccounting=yes -p MemoryMax=$CAP -p MemorySwapMax=0 -- sh "$0" "$@"
fi
A=build/gpui-apk-artifacts; rm -rf $A; mkdir -p $A; T=$PWD/build/gpui-toolchain; mkdir -p $T
now() { date +%s; }; S0=$(now)
sh -c 'g=$(cut -d: -f3 /proc/self/cgroup); b=/sys/fs/cgroup$g; cat "$b/memory.max" "$b/memory.swap.max"' > $A/cap.txt
# JDK 17: the image's, else a downloaded one (inside the job only)
if java -version 2>&1 | grep -q 'version "17'; then JH=$(dirname $(dirname $(readlink -f $(command -v java)))); else
  [ -x $T/jdk/bin/java ] || { mkdir -p $T/jdk; curl --proto '=https' --tlsv1.2 -fsSL -o $T/jdk.tgz https://api.adoptium.net/v3/binary/latest/17/ga/linux/x64/jdk/hotspot/normal/eclipse; tar -xzf $T/jdk.tgz -C $T/jdk --strip-components=1; rm -f $T/jdk.tgz; }
  JH=$T/jdk; fi
export JAVA_HOME=$JH PATH=$JH/bin:$PATH
# Android SDK command line tools, platform 35, build-tools 34
SDK=$T/sdk; export ANDROID_HOME=$SDK ANDROID_SDK_ROOT=$SDK
if [ ! -d $SDK/cmdline-tools/latest ]; then
  mkdir -p $SDK/cmdline-tools $T/x; curl --proto '=https' --tlsv1.2 -fsSL -o $T/cl.zip https://dl.google.com/android/repository/commandlinetools-linux-13114758_latest.zip
  (cd $T/x && python3 -c "import zipfile;zipfile.ZipFile('$T/cl.zip').extractall('.')"); chmod -R u+x $T/x/cmdline-tools/bin; mv $T/x/cmdline-tools $SDK/cmdline-tools/latest; rm -rf $T/x $T/cl.zip
fi
yes | $SDK/cmdline-tools/latest/bin/sdkmanager --licenses >/dev/null 2>&1 || true
$SDK/cmdline-tools/latest/bin/sdkmanager "platforms;android-35" "build-tools;34.0.0" > $A/sdkmanager.log 2>&1 || { tail -20 $A/sdkmanager.log; exit 1; }
S1=$(now)
# bridge.js comes from the Boat bridge build (uploaded with --include-file), never built here
mkdir -p mobile/build/assets; cp scripts/gpui/android-bridge-stage/bridge.js mobile/build/assets/bridge.js
X=""; [ "${1:-}" = nosidebar ] && X="-PbpNativeSidebar=false"
export GRADLE_USER_HOME=$PWD/build/gpui-gradle-home
cd mobile/android
/usr/bin/time -v -o ../../$A/gradle.time ./gradlew --no-daemon --console=plain --max-workers=2 -Dorg.gradle.jvmargs=-Xmx3g assembleDebug -PbpNative=true $X > ../../$A/gradle.log 2>&1 || { tail -40 ../../$A/gradle.log; exit 1; }
cd ../..; S2=$(now)
cp mobile/android/app/build/outputs/apk/debug/app-debug.apk $A/app-debug.apk
sha256sum $A/app-debug.apk bridge-check 2>/dev/null | head -1 > $A/apk.sha256; stat -c %s $A/app-debug.apk > $A/apk.bytes
sha256sum mobile/build/assets/bridge.js > $A/bridge.sha256
printf '{"machine":"Boat default VM (cap 8 GiB, swap 0)","variant":"%s","toolchain_seconds":%s,"gradle_seconds":%s,"total_seconds":%s,"apk_bytes":%s}\n' "${1:-default}" $((S1-S0)) $((S2-S1)) $((S2-S0)) "$(cat $A/apk.bytes)" > $A/timing.json
grep -E "Maximum resident" $A/gradle.time >> $A/timing.json; cat $A/timing.json $A/cap.txt; java -version 2>&1 | head -1
