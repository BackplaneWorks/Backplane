# Embedded Chromium in the GPUI client (feature `cef`)

The native GPUI client (`native/gpui`) can show the Browser tab as a real, clickable Chromium: CEF off-screen rendering through the Rust `cef` crate
(`=154.5.0`, CEF 154.0.34 — the same runtime the Electron/X11 helper uses, `~/.cache/backplane/cef-runtime/154.0.34`).
Default builds do not have it; the Browser tab then shows the hub's JPEG frames (`viewers/browser.rs`).

    CEF_PATH=$HOME/.cache/backplane/cef-rs cargo build --profile fast --features cef     # in native/gpui
    # the first build downloads the CEF distribution into CEF_PATH and builds its C++ wrapper (cmake + ninja); later builds reuse it

- **Runtime**: `BACKPLANE_CEF` (the runtime directory, or the `backplane-cef` file in it), else `~/.cache/backplane/cef-runtime/154.0.34`.
  The binary links `libcef.so` dynamically with `RUNPATH=$ORIGIN` (the cef crate copies it beside the binary), so a `cef` build needs libcef next to it
  to start at all; the JPEG fallback applies when the runtime *directory* (paks, icudtl.dat) is missing or CEF refuses to start (a reason is logged `GPUI_LOG {"cef":…}`
  and said in the page). Do not put the runtime dir on `LD_LIBRARY_PATH`: it carries its own `libvulkan.so.1`, which breaks the GPU stack.
- **Processes**: one engine per client process (`viewers/cef.rs`: multi-threaded message loop, `no_sandbox`, windowless). CEF's helper processes are this
  same executable re-run with `--type=…`; `main()` calls `cef::subprocess()` first. Profile: `~/.cache/backplane/cef-gpui-profile` (`BACKPLANE_CEF_PROFILE`).
  `BACKPLANE_CEF_NO_GPU=1` disables GPU (needed on Xvfb).
- **Page** (`viewers/cef_page.rs`): OSR frames (BGRA, 30 fps) become a gpui image; address bar (Enter, Ctrl+L, paste), back/forward/reload/stop, tabs, `+` new tab,
  popups (`window.open`, `target=_blank`) open as new tabs; mouse move/click/double-click/wheel, keys (+ copy/cut/paste/select-all/undo), text through gpui's
  input handler (IME composition and commit), focus, resize/scale, cursor shape.
- **Agent**: CEF's DevTools port is on loopback (`BACKPLANE_CEF_CDP_PORT`, else a free port). The client sends the hub `bots.browser {op:"attach", endpoint, urls, active}`;
  the hub (`term/driver.rs`) passes it to its browser helper (`tools/browser`, op `attach`: `connectOverCDP` to the loopback endpoint) which makes that page the
  owner's active page, so `preview_*` act on what the person sees. Only loopback endpoints are accepted. The first mouse/key input takes the page over
  (`op:"takeover"`: the agent's calls are refused with "The person has the browser…"); the toolbar's "Hand back" sends `handback`.
  The attach path assumes the client and hub share a machine (loopback).
- **Hub presenter**: the Rust hub's viewer panel now accepts kind `browser` (`present/viewer.rs`), which was missing, so the tab could not be opened.
- **Test**: `test/cef/cef_e2e.sh CLIENT HUB` (private Xvfb + i3, local hub, stand-in agent, local page; real XTest mouse/keys via `xinput.py`; CDP; the agent through
  the hub's MCP `preview_evaluate`). Script steps: `{"cef":{"goto":url|"back"|"forward":true|"click":[x,y]|"type":text|"handback":true|"report":true}}`.
  `test/cef/lifecycle_e2e.sh CLIENT HUB` (CI job `browser-linux`): an https and a file:// page load, navigation and back, real X typing and wheel scrolling,
  the Browser tab closed and reopened and its page dropped (another thread) and made again with no CEF helper growing, and quitting leaves none of the helpers it
  ran (descendants of the client's pid, checked by pid). `E2E_DISPLAY=:N` runs it on an X server already up (e.g. a headless sway's Xwayland: this desktop's Xvfb has no lavapipe).
- **Closing**: a page going away (its thread left) closes its browsers (`Drop for CefPage`): CEF's client holds each `Tab`, which holds its browser, so nothing else would,
  and every renderer it ran would live until the client quit. Quitting needs no `cef::shutdown`: the helpers exit with the browser process (asserted by the lifecycle test).
- **Display**: the window draws through X11 only (gpui's `x11` feature). With `DISPLAY` set (Xwayland on a Wayland desktop) CEF is given `--ozone-platform=x11`, so a
  `WAYLAND_DISPLAY` beside it does not move Chromium's own windows (DevTools) to another server. Wayland without Xwayland: the client says so and exits (status 3)
  instead of gpui's panic (`plat::display_problem`).
- **Archive**: `scripts/check-linux-archive.sh` (release.yml, every Linux build) unpacks the archive and fails unless libcef.so, icudtl.dat, resources.pak,
  chrome_100/200_percent.pak, v8_context_snapshot.bin and locales/ are beside backplane-gpui, with RUNPATH `$ORIGIN` and libcef.so resolving to that copy.

Not done / known limits: remote JPEG frames for attached pages (`--jpeg-dir` screencast skips them, so web/phone viewers do not mirror the embedded page);
the first click is not swallowed (it both takes over and reaches the page); no downloads/file dialogs/drag-and-drop; only `window.open` popups that CEF reports to
`on_before_popup` become tabs (script popups lose their opener); X11 only; teach-mode recording is not wired to native clicks; GPU path untested (Xvfb software).

**Layout**: CEF reads `icudtl.dat`, the paks and the snapshot from the directory `libcef.so` was loaded from (the link's own directory, not its target's). A missing file is a
failed CHECK inside `cef_initialize` that kills the process, so `Engine::start` links what is missing beside the executable from the runtime directory and refuses (the page
says why) if it still is not all there. `test/cef/link_only_e2e.sh` runs the dev-build layout (only a link to libcef) and asserts the window survives opening the Browser tab.
Platforms: CEF ships on Linux only; macOS and Windows builds use the hub's frame stream (see `release.yml`).
