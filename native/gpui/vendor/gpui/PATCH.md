gpui 0.2.2 (Apache-2.0) vendored without `examples/` (and its `[[example]]` targets), with these changes:

- `build.rs`: gpui's Windows step (`mod windows`) is behind the build host's cfg, so a Windows client built on Linux had no shader bytes to include and no manifest. `mod windows_cross` runs instead when the host is not Windows: for a target without debug assertions it copies `windows-shaders/shaders_bytes.rs` into `OUT_DIR`, refusing it unless its first line is the FNV-1a 64 of every `src/platform/windows/*.hlsl`; with `windows-manifest` it embeds the manifest through embed-resource (llvm-rc for -msvc, windres for -gnu*).
- `Cargo.toml`: `embed-resource` is a build dependency on every host, not only Windows.
- `src/platform/mac/platform.rs`: `GPUIApplication` answers Chromium's `isHandlingSendEvent` / `setHandlingSendEvent:` (CrAppControlProtocol) and keeps that flag set inside `-sendEvent:`, as CEF's own applications do, so the embedded Chromium of the Browser tab (feature `cef`) runs inside gpui's NSApplication.
- `src/platform/windows/clipboard.rs`: a clipboard holding only a bitmap (CF_DIB: PrintScreen, many programs' Copy) reads as a PNG (`read_bitmap_as_png`, tried only when no PNG, GIF, JPEG or SVG is there); upstream read nothing from it.

- `src/window.rs`: `Window::on_frame_drawn` (a listener run after every frame the window draws, for as long as it answers true) and `Window::hitbox_drawn_after` (in the
  frame last drawn, a hitbox's visible bounds and those of every hitbox drawn after it over them): the windowed Browser tab places Chromium's native window over its pane
  and hides it under anything gpui draws on top (`viewers/cef_page.rs`).
- `src/platform/linux/x11/window.rs`: `X11Window` gives its window id as a raw window handle (upstream: `unimplemented!()`), the parent of Chromium's window.
- `src/platform/linux/x11/client.rs`: a `FocusOut` with detail `Inferior` (the keyboard went to a child window of ours, the Browser tab's Chromium) leaves the window active.

- `src/platform/windows/directx_renderer.rs`: the DirectComposition target is not topmost (`CreateTargetForHwnd(hwnd, false)`), so child windows (the Browser tab's
  Chromium) are composited above gpui's drawing; topmost, DWM put gpui's visual over them and the page could not be seen.
- `src/platform/windows/window.rs`: only popups get `WS_EX_NOREDIRECTIONBITMAP`: a window without a redirection surface shows nothing of a child that paints through
  GDI (Chromium without GPU compositing).

- `resources/windows/gpui.manifest.xml`: a `compatibility` section (Windows 7 to 11), as CEF's own applications have: without it Windows runs the client as a
  Vista-era program, where a child window cannot be layered and Chromium takes older code paths.

- Trackpad pinch (#1106): `PinchEvent` (`src/interactive.rs`: position, AppKit's magnification as `delta`, modifiers, phase) and `PlatformInput::Pinch`, dispatched
  as a mouse event (`src/window.rs` keeps the pointer and modifiers from it), with `on_pinch` on elements (`src/elements/div.rs`, hit-tested as the scroll wheel is).
  `src/platform/mac/window.rs` registers `magnifyWithEvent:` on `GPUIView` and `src/platform/mac/events.rs` turns `NSEventTypeMagnify` into it; upstream dropped
  pinches. A pinch is its own event, not a ctrl + wheel, so lists, the terminal and pages that scroll never take it for a scroll.

`windows-shaders/shaders_bytes.rs` is made on a Windows machine by `scripts/windows-shaders.sh HOST` (fxc.exe with gpui's own arguments). A Windows build host still compiles the shaders itself, as upstream does.
