gpui 0.2.2 (Apache-2.0) vendored without `examples/` (and its `[[example]]` targets), with these changes:

- `build.rs`: gpui's Windows step (`mod windows`) is behind the build host's cfg, so a Windows client built on Linux had no shader bytes to include and no manifest. `mod windows_cross` runs instead when the host is not Windows: for a target without debug assertions it copies `windows-shaders/shaders_bytes.rs` into `OUT_DIR`, refusing it unless its first line is the FNV-1a 64 of every `src/platform/windows/*.hlsl`; with `windows-manifest` it embeds the manifest through embed-resource (llvm-rc for -msvc, windres for -gnu*).
- `Cargo.toml`: `embed-resource` is a build dependency on every host, not only Windows.
- `src/platform/mac/platform.rs`: `GPUIApplication` answers Chromium's `isHandlingSendEvent` / `setHandlingSendEvent:` (CrAppControlProtocol) and keeps that flag set inside `-sendEvent:`, as CEF's own applications do, so the embedded Chromium of the Browser tab (feature `cef`) runs inside gpui's NSApplication.
- Linux clipboard writes every form an item has (`src/platform/linux/clipboard_offer.rs`, used by `x11/clipboard.rs` `set_item` and `wayland/clipboard.rs`): upstream offered
  every item as text only, so a copied picture was an empty string (the client's Copy image gave nothing to paste). A picture goes as `image/png` (transcoded from JPEG,
  WebP, GIF, BMP, TIFF or SVG) and in its own format, its strings as text. No `text/uri-list` of a picture's file: snap Firefox took it over the image and
  pasted a picture it could not decode. X11 serves the text aliases it advertises (upstream listed `text/plain;charset=utf-8` and answered nothing for
  it), fixes `set_image`'s atom, and sends a reply too large for one request in INCR segments (ICCCM 2.7.2). Wayland names the newer of the last key press and the last
  click as the selection's serial (`SerialTracker::latest_input`): a copy from a menu click named an older key press, which mutter refuses when another app copied since.
  `linux_clipboard_offers` / `linux_clipboard_bytes_for` are public so the client tests them (this crate's own tests do not run).
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

- `src/platform/linux/x11/client.rs`: `BACKPLANE_GPUI_LOOPTRACE=1` (off unless set) logs the main loop's slow steps on stderr as `GPUI_LOOP {...}`: a foreground task, a frame or an X batch over 50 ms, and each start or stop of a window's refresh loop (mapped, visibility). The client adds a 100 ms foreground timer that logs when it wakes more than 50 ms late (`src/main.rs`). For finding what holds the main thread.

- `src/elements/text.rs`: a process-wide text filter (`set_text_filter`, `TextFilter`) that every text element's layout passes its string
  through (the client's streamer mode hides the person's identifiers). The filter answers byte ranges and their replacements; the layout
  draws the replaced text, fits the style runs to it, and keeps a map so `index_for_position`, `position_for_index` and
  `line_layout_for_index` still speak the original text's indices (links, selections, highlights). Off, it costs one atomic load per layout.

- Paths that cost less a frame (a board plot is hundreds of thousands of path vertices, drawn again every frame the window draws):
  `Window::paint_path_at` paints a path the caller keeps, moved by an offset (`Path::scale_moved`: one copy, where `paint_path` of a kept
  path was a clone and then a scaled copy), so a plot tessellated once follows the camera through a drag. `Scene::insert_path` keeps a path
  once (`PaintOperation::Path(PathId)`; `insert_primitive` kept it in its list and again in `paint_operations`), and `replay` finds it by id
  in the scene it replays from; `paint_path` uses it too. `src/platform/blade/blade_renderer.rs` + `shaders.wgsl`: a path vertex for the
  shader is 24 bytes (position, st, its path's index) and each path's colour and bounds are sent once (`PathRasterizationPath`,
  `b_path_data`), where every vertex carried them (104 bytes); the vertex list is one buffer kept between frames. Measured on a routed
  8-layer board (608 000 vertices): the renderer's vertex list 42 ms -> 7 ms a frame, its upload 8 ms -> 2 ms. The Metal and DirectX
  renderers are as upstream.

- `src/window.rs`: `DispatchEventResult` is `pub` (was `pub(crate)`), so the client's test-mode `pointer` script step can call the public
  `Window::dispatch_event` with a mouse move, down and up at a control's rect (scripted e2e only; no behaviour change).
