gpui 0.2.2 (Apache-2.0) vendored without `examples/` (and its `[[example]]` targets), with these changes:

- `build.rs`: gpui's Windows step (`mod windows`) is behind the build host's cfg, so a Windows client built on Linux had no shader bytes to include and no manifest. `mod windows_cross` runs instead when the host is not Windows: for a target without debug assertions it copies `windows-shaders/shaders_bytes.rs` into `OUT_DIR`, refusing it unless its first line is the FNV-1a 64 of every `src/platform/windows/*.hlsl`; with `windows-manifest` it embeds the manifest through embed-resource (llvm-rc for -msvc, windres for -gnu*).
- `Cargo.toml`: `embed-resource` is a build dependency on every host, not only Windows.
- `src/platform/windows/clipboard.rs`: a clipboard holding only a bitmap (CF_DIB: PrintScreen, many programs' Copy) reads as a PNG (`read_bitmap_as_png`, tried only when no PNG, GIF, JPEG or SVG is there); upstream read nothing from it.

`windows-shaders/shaders_bytes.rs` is made on a Windows machine by `scripts/windows-shaders.sh HOST` (fxc.exe with gpui's own arguments). A Windows build host still compiles the shaders itself, as upstream does.
