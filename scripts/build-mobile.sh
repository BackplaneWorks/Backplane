#!/bin/sh
# Build the phone apps' shared Bend client and the Android APK.
#   mobile/build/assets/bridge.js   src/mobile (Bend) + bridge.js, one script
#   dist/backplane-android.apk      when an Android SDK is found
# The iOS app is built on a Mac: mobile/ios/build.sh (see mobile/README.md).
set -eu
cd "$(dirname "$0")/.."
. scripts/bendok.sh
bend_check src/mobile/app.bend || exit 1
rm -rf build/mobile-web
bend src/mobile/index.html -o build/mobile-web >/dev/null
mkdir -p mobile/build/assets
cat build/mobile-web/chunk-*.js > mobile/build/assets/bridge.js
# native string comparisons (Bend's walk a char at a time: half a screen's cost)
python3 scripts/bridge-native.py mobile/build/assets/bridge.js
# the client state crosses into Bend as it is, not rebuilt at every call
python3 scripts/js-unmarshal.py hubs.Hubs mobile/build/assets/bridge.js
# the kept state's name: the apps keep it across builds with the same types
printf '\nglobalThis.BackplaneStateKey = "%s";\n' "$(python3 scripts/state-key.py)" >> mobile/build/assets/bridge.js
cp assets/fonts/NotoSansCuneiform-Regular.ttf mobile/build/assets/
cp assets/fonts/NOTO-CUNEIFORM-OFL.txt mobile/build/assets/
echo "mobile/build/assets/bridge.js ($(wc -c < mobile/build/assets/bridge.js) bytes)"
[ "${1:-}" = "--js" ] && exit 0
sdk=${ANDROID_HOME:-$HOME/.local/opt/android-sdk}
if [ -d "$sdk" ]; then
  export ANDROID_HOME="$sdk"
  [ -n "${JAVA_HOME:-}" ] || [ ! -d "$HOME/.local/opt/jdk17" ] || export JAVA_HOME="$HOME/.local/opt/jdk17"
  (cd mobile/android && ./gradlew -q assembleRelease)
  mkdir -p dist
  cp mobile/android/app/build/outputs/apk/release/app-release.apk dist/backplane-android.apk
  ls -la dist/backplane-android.apk
else
  echo "no Android SDK at $sdk: skipping the APK"
fi
