#!/bin/sh
# Build the GPUI experiment's debug APK on THIS desktop (never the Mac), in its own
# capped scope: 8G, no swap, nice 19, cores 0-5, 2 gradle workers.
# usage: scripts/gpui/android-build.sh [cold|warm]
# cold = empty GRADLE_USER_HOME (build/gpui/android/gradle-home) + no project build dir.
# warm = reuse them. Uses ~/Android-sdk-writable and JDK 17 already here.
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd); W=$ROOT/build/gpui/android; MODE=${1:-warm}
avail=$(df --output=avail -BG / | tail -1 | tr -dc 0-9); [ "$avail" -ge 40 ] || { echo "only ${avail}G free on /, need 40"; exit 1; }
mkdir -p "$W/src/mobile/build/assets" "$ROOT/build/gpui/results/android"
rsync -a --delete --exclude build --exclude .gradle "$ROOT/mobile/android/" "$W/src/mobile/android/"
rsync -a --delete "$ROOT/mobile/shell/" "$W/src/mobile/shell/"
# bridge.js: built from Bend elsewhere, never here (see build/gpui/results/android/bridge-source.txt)
cp "${BRIDGE:-$W/assets/bridge.js}" "$W/src/mobile/build/assets/bridge.js"
if [ "$MODE" = cold ]; then rm -rf "$W/gradle-home" "$W/src/mobile/android/.gradle" "$W/src/mobile/android/app/build"; fi
mkdir -p "$W/gradle-home"
S=$(date +%s.%N)
systemd-run --user --scope -q -p MemoryMax=8G -p MemorySwapMax=0 \
  env JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 ANDROID_HOME=$HOME/Android-sdk-writable ANDROID_SDK_ROOT=$HOME/Android-sdk-writable GRADLE_USER_HOME="$W/gradle-home" \
  nice -n 19 taskset -c 0-5 sh -c 'cd "$1/src/mobile/android" && ./gradlew --no-daemon --console=plain --max-workers=2 -Dorg.gradle.jvmargs=-Xmx3g assembleDebug -PbpNative=true ${GRADLE_EXTRA:-}' sh "$W" \
  > "$W/build-$MODE.log" 2>&1 || { tail -40 "$W/build-$MODE.log"; exit 1; }
E=$(date +%s.%N)
APK=$W/src/mobile/android/app/build/outputs/apk/debug/app-debug.apk
cp "$APK" "$W/bp-gpui-android-debug.apk"
printf '{"stream":"android","machine":"desktop (this machine, 32 cpus pinned to 6, 59G RAM)","mode":"%s","seconds":%s,"apk_bytes":%s,"commit":"%s"}\n' "$MODE" "$(echo "$E - $S" | bc)" "$(stat -c %s "$APK")" "$(git -C "$ROOT" rev-parse --short HEAD)" | tee -a "$ROOT/build/gpui/results/android/builds.jsonl"
