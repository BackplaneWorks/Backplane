# The Browser tab: embedded Chromium (CEF)

The desktop Browser tab is a real Chromium inside the GPUI window, the same way on Linux (X11, and Wayland desktops through XWayland), macOS and Windows.
**It is built in by default**: `scripts/dev-build.sh` (and so `scripts/main-build.sh`), `scripts/mac-build.sh`, `scripts/windows-check.sh --xwin` and `release.yml`
build `backplane-gpui` with `--features cef` and put the engine's runtime beside it (`scripts/cef-runtime-files.sh`). Nothing about the page goes through the hub: no frames, no JPEG, no network.

## What the embedded page is
CEF **in this process** (the `cef` crate, `=154.5.0`, CEF 154.0.34), **windowed**: Chromium draws into its own native child window inside the client's window, and
nothing is copied. The page (`viewers/cef_page.rs`) places that window over the Browser pane after every frame gpui draws; `viewers/cef_native.rs` is the per-platform part.
The profile (cookies, storage) is `~/.cache/backplane/cef-gpui-profile` (Windows: `%LOCALAPPDATA%\backplane\cef-gpui-profile`).

| | Linux (X11) | Windows | macOS |
|---|---|---|---|
| Chromium's window | an X11 window in a container window of ours inside the client's (gpui's window has a 32-bit visual, and a child with its parent's visual and no colormap is refused) | a child HWND of the client's window | an NSView subview of the client's view |
| placed by | `ConfigureWindow` on our own X connection, in physical pixels | `SetWindowPos` (asynchronous: the child belongs to CEF's UI thread) | `setFrame:` in points (gpui's view is not flipped) |
| hidden by | unmapping the container and `_NET_WM_STATE_HIDDEN` on Chromium's window (an unmap alone leaves its compositor running) | `ShowWindow(SW_HIDE)` | `setHidden:` |
| closed by | CEF itself (`do_close` false: it sends `WM_DELETE_WINDOW` to its own window); the container goes in `on_before_close` | `DestroyWindow` of the browser's window (`do_close` true: false would send `WM_CLOSE` to the client's window) | `removeFromSuperview` (`do_close` true, for the same reason: `performClose:`) |
| engine | `libcef.so` beside the client (RUNPATH `$ORIGIN`) | `libcef.dll`, `chrome_elf.dll` beside the client | `Chromium Embedded Framework.framework` beside the client (or in `../Frameworks`), loaded at run time (`cef_mac.rs`) |
| CEF's thread | its own UI thread (multi-threaded message loop) | the same | gpui's main thread: an external message pump (`cef_page.rs` `drive_pump`), run when CEF asks and at least every 1/30 s |
| helper processes | the client itself, run again with `--type=...` (`cef::subprocess()` first in `main`) | the same | `Backplane Helper.app` and its `(GPU)`, `(Renderer)`, `(Plugin)`, `(Alerts)` twins beside the client (`scripts/cef-mac-helpers.sh`) |

On Windows a child window shows over gpui's drawing only with three changes in vendored gpui (`vendor/gpui/PATCH.md`): the client's manifest declares Windows 10
compatibility (without it Windows runs the client as a Vista-era program and Chromium's window never reached the screen), gpui's DirectComposition target is not
topmost (it is drawn under child windows), and the window keeps a redirection surface (`WS_EX_NOREDIRECTIONBITMAP` only on popups), for a Chromium that paints
through GDI (no GPU compositing).
On a Wayland desktop the client is an X11 client through XWayland (gpui's x11 backend) and Chromium is pinned to X11 (`--ozone-platform=x11`), so it is windowed there too.
The sandbox is off on every platform (`no_sandbox`).

**Placement, overlays, views and tabs** (`CefPage::after_frame`, run after every frame gpui draws through `Window::on_frame_drawn`, a vendored-gpui hook): the pane inserts a
marker hitbox; from the frame just drawn, `Window::hitbox_drawn_after` gives the marker's visible bounds and those of every hitbox drawn after it that overlaps them. The active
tab's window is placed on the marker's bounds; it is hidden when the marker was not drawn (another view in the pane, another thread, the viewer closed) or when anything drawn
later covers the pane (a popover, a menu, a dialog: a native window would otherwise sit on top of it), apart from slivers under 12 px (a splitter's grip). The other tabs'
windows are hidden. A cached view keeps its hitboxes, so this holds when gpui replays a view without rendering it.

**Keyboard and focus.** The keyboard follows gpui's focus: when the page's focus handle gets it, Chromium's window gets the keyboard (`give_keyboard`); when it loses it while the
client's window is active (a click in the composer, Ctrl+L to the address field), the client's window gets it back. A click into the page (`on_set_focus` from the system) gives the
page gpui's focus and counts as the person taking the page over; Chromium focusing itself (a page shown, a script's `focus()`) gives the keyboard straight back unless the page
already has gpui's focus. Keys pressed in the page go to the client first when they could be its shortcuts (a chord with Ctrl, Alt or Cmd, or an F key, except the editing chords:
copy, cut, paste, select all, undo, redo, moving and deleting by word or line; `cef::client_key`): `on_pre_key_event` takes them, the page dispatches them through gpui's
bindings with the page's key context (so `CefPage`'s own Ctrl+L and F12 apply), and sends a key no binding took back to Chromium. On X11 the client's window stays the active one
while the keyboard is in its child (vendored gpui ignores `FocusOut` with detail `Inferior`).

**Display scale.** Linux and Windows place in physical pixels (the pane's bounds times gpui's scale factor); Chromium picks its own scale from the system (Xft.dpi, per-monitor DPI).
macOS places in points.

**Off screen, the fallback.** When the client's window cannot be a parent (a raw window handle that is not X11, Win32 or AppKit) or `BACKPLANE_CEF_OSR=1` is set, CEF renders
off screen: Chromium paints each frame into a BGRA buffer (`on_paint`), the client copies it into a gpui image at 30 fps and sends the person's input to it. The log says which:
`{"cef_mode":"windowed"|"offscreen","why":...}`.

Measured on a private Xvfb with software rendering (llvmpipe, no GPU) by `test/cef/cef_e2e.sh` (an animated page), windowed: about **61 fps** (the page's own frame rate;
off screen: 30 fps painted into the gpui image), **9 ms** from a click to the changed pixel in Chromium's window (off screen: 75-90 ms), **2 ms** from a layout change to
Chromium's widget at the new size. `test/cef/windowed_e2e.sh` (Linux, with a window manager and real X input) and `test/cef/windowed_proof.py` (every platform) check the
placement, overlays, views, tabs, the window manager's resizing and the keyboard.

Input is native: windowed, Chromium's window takes the mouse, keys, IME and cursor shapes itself (off screen they come through gpui's input handler); file dialogs are
Chromium's own on Windows and macOS and gpui's picker on Linux, popups as tabs, **F12 / Ctrl+Shift+I** (DevTools in a window of its own). Downloads go to `~/Downloads`.

## Build and runtime
- `scripts/dev-build.sh` builds the window with `GPUI_FEATURES=cef` (default; `GPUI_FEATURES=` for none). `scripts/mac-build.sh` (Mac mini, `MAC_DEST` for a lane's own
  copy) puts the framework and helper apps in its `dist/`; `scripts/windows-check.sh --xwin DIR` puts the Windows runtime in DIR (clang-cl builds CEF's wrapper there). CEF's distribution is cached once in `CEF_PATH`
  (`~/.cache/backplane/cef-rs`; the `cef` crate downloads it on the first build and compiles its small C++ wrapper, a few minutes once).
- The runtime other than `libcef.so` (paks, `icudtl.dat`, locales, snapshot) lives in `~/.cache/backplane/cef-runtime/<version>`, made once from the build with `libcef.so`
  stripped (270 MB instead of 1.4 GB). Every published build dir holds `libcef.so` as a link to that one file; the window is linked with RUNPATH `$ORIGIN`.
  `BACKPLANE_CEF` names another runtime directory.
- If the cef build fails the window is built without it and the Browser tab says so. If the feature is there but CEF cannot start (no runtime, `BACKPLANE_CEF_OFF`), the tab
  falls back to the hub's frames and shows a banner `embedded browser unavailable: <reason>`.
- Also: `BACKPLANE_CEF_NO_GPU=1` (software only, for Xvfb), `BACKPLANE_CEF_CDP_PORT`, `BACKPLANE_CEF_PROFILE`.

## The agent and the hub
- CEF's DevTools port is on loopback; the client sends `bots.browser {op: "attach"}` and the hub's browser helper (`tools/browser`, found beside the hub, in
  `~/.local/share/backplane`, or on PATH; dev builds ship it) attaches over CDP, so `preview_*` act on the page the person sees. The first mouse or key input takes the page over
  (`takeover`: the agent's calls are refused); "Hand back" returns it. Teach mode records native clicks.
- **Frames only for watchers.** The helper does not screencast or encode anything for a page nobody watches. A phone or web client that shows the Browser tab polls
  `/bots/<id>/screen.jpg`; each request is its lease (the hub asks the helper to `cast` for 2.5 s), the screencast runs while it lasts and stops after. The embedded desktop
  page never polls. `test/cef/cef_e2e.sh` asserts: no `shots/<id>.jpg` and no screencast with only the desktop attached; a lease turns it on and `screen.jpg` is a JPEG; after
  the lease the screencast is off and nothing is written.
- In the hub-frame fallback the page takes the pane's size (the window tells the hub, `bots.browser {op: "viewport"}`), so it fills the pane.

## The chrome
One themed bar for both pages (`viewers/chrome.rs`): an always-visible tab strip (title, close, +), a 40 px toolbar (back, forward, reload/stop icons with hover and disabled states), a
bordered address field that shows the page's address when not being edited (the first key typed replaces it), and "Hand back" when the person has the page. Colours are the
app's tokens, light and dark.

## Tests
`test/cef/agent_proof.py CLIENT HUB HELPER ASSETS OUT` runs on any desktop (Linux X11 or Wayland, macOS, Windows; e2e-macos.yml job `browser`, test/windows/rig/sess.sh on
Windows): a hub with the stand-in agent, the Browser tab, and the agent's `preview_navigate`, `preview_click`, `preview_evaluate` and `preview_snapshot` (with its screenshot)
against a local page, checked from the client's own report of its embedded page; it saves the window and the agent's screenshot.
`test/browser/browser_e2e.py` (workflow `browser-e2e.yml`, windows-latest, macos-latest and ubuntu-latest) drives both pages through the same `{"browser":{"op":…}}` steps:
http, `file://` and https pages, typing, scrolling, a link, back and forward, tabs, the view closed and reopened, a clean exit and no process left; with CEF, without it,
and with a cef client whose runtime was taken away (it must start and fall back). Status, risks and what is unverified: `docs/browser-cef-port.md`.
`test/cef/cef_e2e.sh CLIENT HUB` (embedded page: input, agent, lease, file dialog, DevTools, teach, performance), `bar_e2e.sh` (the fallback's bar and tabs; `THEME=light|dark`,
`NOHELPER=1` for the missing-helper message), `open_e2e.sh HUB`, `shots.sh` (screenshots, including a real https site). All need a private Xvfb (the scripts start one) and the
shared `flock /tmp/bp-heavy.lock` rules.

## Known gaps
- **Downloads:** `can_download` is called, but CEF never reaches `on_before_download`, so nothing is saved (`test/cef/cef_e2e.sh` reports it red).
- **Popups keep no opener:** `window.open` pages open as tabs and load, but `window.opener` is not set in them (red in the same script).
- DevTools (F12) opens a window of Chromium's own, outside gpui. GPU rendering is untested on macOS and Windows beyond the proof runs; no video codecs.
- Native Wayland: the client itself is X11-only (gpui's `x11` backend), so a Wayland desktop runs it, and Chromium, through XWayland.
