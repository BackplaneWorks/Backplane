#!/bin/sh
# Run ON the Mac, under the mac-heavy lock. Installs JDK 17 + Android SDK
# + arm64 system image + AVD inside ~/bp-gpui-android/toolchain. No brew/sudo.
set -eu
T=$HOME/bp-gpui-android/toolchain
mkdir -p "$T/dl" "$T/jdk" "$T/sdk/cmdline-tools" "$T/gradle-home" "$T/avd"
cd "$T/dl"
[ -x "$T/jdk/Contents/Home/bin/java" ] || {
  curl -fL -o jdk.tgz https://api.adoptium.net/v3/binary/latest/17/ga/mac/aarch64/jdk/hotspot/normal/eclipse
  tar -xzf jdk.tgz -C "$T/jdk" --strip-components=1 && rm jdk.tgz; }
export JAVA_HOME=$T/jdk/Contents/Home
[ -d "$T/sdk/cmdline-tools/latest" ] || {
  curl -fL -o cl.zip https://dl.google.com/android/repository/commandlinetools-mac-15641748_latest.zip
  mkdir x && unzip -q cl.zip -d x && mv x/cmdline-tools "$T/sdk/cmdline-tools/latest" && rm -rf x cl.zip; }
export ANDROID_HOME=$T/sdk ANDROID_SDK_ROOT=$T/sdk ANDROID_AVD_HOME=$T/avd
SM=$T/sdk/cmdline-tools/latest/bin/sdkmanager
yes | "$SM" --licenses >/dev/null 2>&1 || true
"$SM" "platform-tools" "platforms;android-35" "build-tools;35.0.0" "emulator" "system-images;android-35;google_apis;arm64-v8a"
echo no | "$T/sdk/cmdline-tools/latest/bin/avdmanager" create avd -n bp-gpui-android -k "system-images;android-35;google_apis;arm64-v8a" -d pixel_6 --force
du -sh "$T"; df -h /System/Volumes/Data | tail -1
