#!/bin/bash
# CEF's runtime, laid out where the client looks for it (native/gpui/src/viewers/cef.rs runtime_dir / NEED_*), from a build of native/gpui with feature cef.
#   scripts/cef-runtime-pack.sh windows|macos TARGET_PROFILE_DIR DEST
# TARGET_PROFILE_DIR is the cargo profile dir (native/gpui/target/release); the CEF distribution is taken from cef-dll-sys's OUT_DIR under it (a cached
# build keeps that one, not always the copy beside the executable).
#   windows: DEST gets libcef.dll, chrome_elf.dll, the other DLLs, the paks, icudtl.dat, the snapshot and locales/: put DEST beside backplane-gpui.exe.
#   macos:   DEST/cef/ gets Chromium Embedded Framework.framework and the five helper bundles "Backplane Helper*.app" (each with backplane-cef-helper from
#            TARGET_PROFILE_DIR as its executable) and a stub Backplane.app (the main bundle CEF is told about), all ad-hoc signed: put DEST/cef beside
#            backplane-gpui. With CODESIGN_ID set, they are signed with that identity and the hardened runtime instead (scripts' entitlements below).
# Exit non-zero, naming what is missing, when the build has no CEF distribution or a file is not there.
set -euo pipefail
os=$1; T=$2; D=$3
die() { echo "cef-runtime-pack: $*" >&2; exit 1; }
[ -d "$T" ] || die "no $T"
mkdir -p "$D"
case $os in
windows)
  R=$(dirname "$(find "$T/build" -path '*cef-dll-sys*' -name libcef.dll 2>/dev/null | head -1)")
  [ -f "$R/libcef.dll" ] || R=$T
  [ -f "$R/libcef.dll" ] || die "no libcef.dll under $T/build (built without --features cef?)"
  echo "CEF runtime from $R"
  for f in libcef.dll chrome_elf.dll icudtl.dat resources.pak chrome_100_percent.pak chrome_200_percent.pak v8_context_snapshot.bin; do
    [ -f "$R/$f" ] || die "$f missing from $R"; cp "$R/$f" "$D/"; done
  # ANGLE, SwiftShader, DirectX's compilers: Chromium's GPU process falls back to these without a GPU (a CI runner, a VM)
  for f in d3dcompiler_47.dll dxcompiler.dll dxil.dll libEGL.dll libGLESv2.dll vk_swiftshader.dll vk_swiftshader_icd.json vulkan-1.dll; do [ ! -f "$R/$f" ] || cp "$R/$f" "$D/"; done
  rm -rf "$D/locales"; cp -r "$R/locales" "$D/locales"
  [ -f "$D/locales/en-US.pak" ] || die "locales/en-US.pak missing"
  ;;
macos)
  F=$(find "$T/build" -path '*cef-dll-sys*' -maxdepth 5 -type d -name 'Chromium Embedded Framework.framework' 2>/dev/null | head -1)
  [ -n "$F" ] && [ -f "$F/Chromium Embedded Framework" ] || die "no Chromium Embedded Framework.framework under $T/build (built without --features cef?)"
  H=$T/backplane-cef-helper; [ -x "$H" ] || die "no $H (cargo build --features cef builds it)"
  echo "CEF framework from $F"
  C=$D/cef; rm -rf "$C"; mkdir -p "$C"
  ditto "$F" "$C/Chromium Embedded Framework.framework"
  plist() { # $1 bundle dir, $2 executable, $3 id, $4 1 for a helper
    mkdir -p "$1/Contents"
    cat > "$1/Contents/Info.plist" <<PL
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>$2</string><key>CFBundleDisplayName</key><string>$2</string>
<key>CFBundleExecutable</key><string>$2</string><key>CFBundleIdentifier</key><string>$3</string>
<key>CFBundlePackageType</key><string>APPL</string><key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
<key>CFBundleVersion</key><string>1</string><key>CFBundleShortVersionString</key><string>1.0</string>
<key>LSMinimumSystemVersion</key><string>11.0</string><key>LSEnvironment</key><dict><key>MallocNanoZone</key><string>0</string></dict>
$( [ "$4" = 1 ] && echo '<key>LSUIElement</key><string>1</string>' )
<key>NSSupportsAutomaticGraphicsSwitching</key><true/>
</dict></plist>
PL
  }
  plist "$C/Backplane.app" "Backplane" dev.backplane.gpui 0
  for v in "" " (GPU)" " (Renderer)" " (Plugin)" " (Alerts)"; do
    n="Backplane Helper$v"; b="$C/$n.app"
    id=dev.backplane.gpui.helper$(echo "$v" | tr -d ' ()' | tr 'A-Z' 'a-z' | sed 's/^./.&/')
    plist "$b" "$n" "$id" 1; mkdir -p "$b/Contents/MacOS"; cp "$H" "$b/Contents/MacOS/$n"
  done
  # signing: ad-hoc unless CODESIGN_ID names an identity (then the hardened runtime, with the JIT entitlements Chromium's helpers need)
  ent=$(mktemp); cat > "$ent" <<EN
<?xml version="1.0" encoding="UTF-8"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>com.apple.security.cs.allow-jit</key><true/><key>com.apple.security.cs.allow-unsigned-executable-memory</key><true/><key>com.apple.security.cs.disable-library-validation</key><true/></dict></plist>
EN
  if [ -n "${CODESIGN_ID:-}" ]; then sg=(codesign --force --timestamp --options runtime --sign "$CODESIGN_ID"); else sg=(codesign --force --sign -); fi
  for l in "$C/Chromium Embedded Framework.framework/Libraries/"*.dylib; do [ -f "$l" ] && "${sg[@]}" "$l"; done
  "${sg[@]}" "$C/Chromium Embedded Framework.framework"
  for v in "" " (GPU)" " (Renderer)" " (Plugin)" " (Alerts)"; do "${sg[@]}" --entitlements "$ent" "$C/Backplane Helper$v.app"; done
  rm -f "$ent"
  codesign --verify --strict "$C/Chromium Embedded Framework.framework" && codesign --verify "$C/Backplane Helper (Renderer).app" || die "a signature does not verify"
  ;;
*) die "usage: cef-runtime-pack.sh windows|macos TARGET_PROFILE_DIR DEST" ;;
esac
du -sh "$D" | sed 's/^/CEF runtime size: /'
