# The Browser tab: embedded Chromium (CEF)

The desktop Browser tab is a real Chromium inside the GPUI window, the same way on Linux (X11, and Wayland desktops through XWayland), macOS and Windows.
**It is built in by default**: `scripts/dev-build.sh` (and so `scripts/main-build.sh`), `scripts/mac-build.sh`, `scripts/windows-check.sh --xwin` and `release.yml`
build `backplane-gpui` with `--features cef` and put the engine's runtime beside it (`scripts/cef-runtime-files.sh`). Nothing about the page goes through the hub: no frames, no JPEG, no network.

## What the embedded page is
CEF **off-screen rendering, in this process** (the `cef` crate, `=154.5.0`, CEF 154.0.34): Chromium paints each frame into a BGRA buffer (`on_paint`), the client
copies it into a gpui image and draws it in the pane. The view's size is the pane's size (it fills the pane, resizes with it, honours the display scale), the
frame rate is CEF's 30 fps. The profile (cookies, storage) is `~/.cache/backplane/cef-gpui-profile` (Windows: `%LOCALAPPDATA%\backplane\cef-gpui-profile`).

| | Linux | Windows | macOS |
|---|---|---|---|
| engine | `libcef.so` beside the client (RUNPATH `$ORIGIN`) | `libcef.dll`, `chrome_elf.dll` beside the client | `Chromium Embedded Framework.framework` beside the client (or in `../Frameworks`), loaded at run time (`cef_mac.rs`) |
| CEF's thread | its own UI thread (multi-threaded message loop) | the same | gpui's main thread: an external message pump (`cef_page.rs` `drive_pump`), run when CEF asks and at least every 1/30 s, as CEF's cefclient does (it wakes the main thread; nothing is drawn unless CEF paints) |
| helper processes | the client itself, run again with `--type=...` (`cef::subprocess()` first in `main`) | the same | `Backplane Helper.app` and its `(GPU)`, `(Renderer)`, `(Plugin)`, `(Alerts)` twins beside the client, each a copy of the small `backplane-cef-helper` (`scripts/cef-mac-helpers.sh`; copies, as an update refuses links) |
| window system | X11, pinned (`--ozone-platform=x11`): on a Wayland desktop the client runs through XWayland and so does Chromium | none needed | gpui's `NSApplication` answers Chromium's `CrAppControlProtocol` (`vendor/gpui/PATCH.md`); keys carry Mac key codes (`cef_mac::key_code`) |

The sandbox is off on every platform (`no_sandbox`), as it was on Linux.

A *windowed* child (an X11 window parented into the gpui window) was considered and not used: gpui draws the whole window through one Vulkan swapchain, so a child X window
would sit on top of every gpui overlay (menus, popups, the tour, the settings dialog), would need its position and clipping kept in step with the layout, and could not be
covered by gpui at all. Off-screen rendering keeps one compositor, so overlays, themes and clipping just work.

Measured on a private Xvfb with software rendering (llvmpipe, no GPU) by `test/cef/cef_e2e.sh` (an animated page): about **30 fps** painted into the gpui image,
**75-90 ms** from a click to the changed pixel being painted, **10-30 ms** from a layout change to the first frame of the new size. Not measured: video (this CEF build ships no
proprietary codecs) and a real GPU.

Input is native: mouse (move, buttons, double click, wheel), keys, text and IME composition through gpui's input handler, copy/cut/paste/undo, focus, cursor shapes,
file dialogs (gpui's own picker), popups as tabs, **F12 / Ctrl+Shift+I** (DevTools in a window of its own). Downloads go to `~/Downloads`.

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
