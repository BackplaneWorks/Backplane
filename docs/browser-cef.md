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
  the hub's MCP `preview_evaluate`). Script steps: `{"cef":{"goto":url|"click":[x,y]|"type":text|"handback":true|"report":true}}`.

Not done / known limits: remote JPEG frames for attached pages (`--jpeg-dir` screencast skips them, so web/phone viewers do not mirror the embedded page);
the first click is not swallowed (it both takes over and reaches the page); no downloads/file dialogs/drag-and-drop; only `window.open` popups that CEF reports to
`on_before_popup` become tabs (script popups lose their opener); X11 only; teach-mode recording is not wired to native clicks; GPU path untested (Xvfb software).
