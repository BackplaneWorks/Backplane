#!/bin/sh
# cef-runtime-files.sh FROM TO OS: copy the embedded Chromium's runtime (what viewers/cef.rs needs at run time) from a build dir FROM (the cef crate copies
# CEF's distribution beside the client) into TO, an install or dev cut beside the client. OS: linux, windows or macos. Fails naming any file missing.
#   linux:   libcef.so (stripped), the paks, icudtl.dat, v8_context_snapshot.bin, locales/, SwiftShader (libvk_swiftshader.so, libvulkan.so.1, vk_swiftshader_icd.json)
#   windows: libcef.dll, chrome_elf.dll, d3dcompiler_47.dll, libEGL.dll, libGLESv2.dll, the paks, icudtl.dat, v8_context_snapshot.bin, locales/, SwiftShader (vk_swiftshader.dll, vulkan-1.dll, vk_swiftshader_icd.json)
#   macos:   Chromium Embedded Framework.framework (FROM is the distribution's Release folder); scripts/mac-cef-bundle.sh makes the helper app
set -eu
F=$1; T=$2; OS=$3; mkdir -p "$T"
need="resources.pak chrome_100_percent.pak chrome_200_percent.pak icudtl.dat v8_context_snapshot.bin locales/en-US.pak"
case $OS in
  linux) need="libcef.so $need"; opt="libvk_swiftshader.so libvulkan.so.1 vk_swiftshader_icd.json";;
  windows) need="libcef.dll chrome_elf.dll $need"; opt="d3dcompiler_47.dll libEGL.dll libGLESv2.dll vk_swiftshader.dll vulkan-1.dll vk_swiftshader_icd.json dxcompiler.dll dxil.dll";;
  macos) fw="Chromium Embedded Framework.framework"; [ -d "$F/$fw" ] || { echo "cef-runtime-files: no $fw in $F" >&2; exit 1; }
    rm -rf "$T/$fw"; cp -R "$F/$fw" "$T/"; exit 0;;
  *) echo "cef-runtime-files: OS is linux, windows or macos" >&2; exit 2;;
esac
for f in $need; do [ -f "$F/$f" ] || { echo "cef-runtime-files: $f missing from $F" >&2; exit 1; }; done
for f in $need; do case $f in locales/*) ;; *) cp "$F/$f" "$T/";; esac; done
rm -rf "$T/locales"; cp -r "$F/locales" "$T/locales"
for f in $opt; do [ ! -f "$F/$f" ] || cp "$F/$f" "$T/"; done
[ "$OS" != linux ] || strip --strip-unneeded "$T/libcef.so" 2>/dev/null || true
