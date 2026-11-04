#!/bin/sh
# Sync mobile/android to the Mac and build the debug APK there. Run under mac-heavy.
# usage: scripts/gpui/android-build.sh [cold]
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
SSH="$ROOT/build/gpui/mac-ssh"; HOST=hs-mac-mini; D=bp-gpui-android
export RSYNC_RSH="$SSH"
$SSH $HOST "mkdir -p ~/$D/src/mobile ~/$D/logs ~/$D/dist"
rsync -a --delete --exclude build --exclude .gradle "$ROOT/mobile/android/" $HOST:$D/src/mobile/android/
rsync -a --delete "$ROOT/mobile/shell/" $HOST:$D/src/mobile/shell/
$SSH $HOST "sh -s" -- "${1:-warm}" <<'REMOTE'
set -eu
T=$HOME/bp-gpui-android/toolchain
export JAVA_HOME=$T/jdk/Contents/Home ANDROID_HOME=$T/sdk ANDROID_SDK_ROOT=$T/sdk GRADLE_USER_HOME=$T/gradle-home
export PATH=$JAVA_HOME/bin:$PATH
cd ~/bp-gpui-android/src/mobile
mkdir -p build/assets
# bridge.js is built from Bend elsewhere (release packet); never built here.
cp ~/backplane-ios-0142-656174-afa967927-202611041741/packet/bridge.js build/assets/bridge.js
if [ "$1" = cold ]; then rm -rf android/app/build android/.gradle "$T/gradle-home/caches" "$T/gradle-home/daemon"; fi
df -h /System/Volumes/Data | tail -1
cd android
S=$(date +%s)
/usr/bin/time -l ./gradlew --no-daemon --console=plain assembleDebug -PbpNative=true > ~/bp-gpui-android/logs/build-$1.log 2>&1 || { tail -60 ~/bp-gpui-android/logs/build-$1.log; exit 1; }
echo "build $1 seconds: $(( $(date +%s) - S ))"
tail -22 ~/bp-gpui-android/logs/build-$1.log | grep -E "maximum resident|real|BUILD"
cp app/build/outputs/apk/debug/app-debug.apk ~/bp-gpui-android/dist/bp-gpui-android-debug.apk
ls -l ~/bp-gpui-android/dist/; du -sh $T
REMOTE
