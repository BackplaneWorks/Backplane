# Embedded Chromium on Windows x64 and macOS arm64: status

How each platform runs CEF, the layout and the build are in
`docs/browser-gpui-cef.md`. This file covers what is proven, what is not, the
risks and the remaining effort.

Before the port, the Browser tab on macOS and Windows was the hub's JPEG
frame stream from `backplane-browser`. That stream is still the fallback
everywhere. When the runtime is missing or incomplete, or CEF refuses to
start, the client starts anyway, the tab's banner says
`embedded browser unavailable: <why>`, and the frames are shown.

## How it is made safe to start without a runtime

- No CEF function runs in a process that is not a CEF helper.
  `cef::subprocess()` decides from argv alone (`is_helper_argv`). The browser
  process declares the API version (`api_hash`) only in `Engine::start`, after
  the runtime check.
- Windows: `libcef.dll` is delay-loaded (`native/gpui/build.rs`, MSVC only).
  The runtime check requires `libcef.dll`, `chrome_elf.dll`, `icudtl.dat` and
  `resources.pak`. A runtime directory that is not beside the exe
  (`BACKPLANE_CEF`, the cache) is put on the DLL search path with
  `SetDllDirectoryW`. The helpers are the client run again, and they get it
  through `BACKPLANE_CEF_DLL_DIR`.
- macOS: the framework is loaded by hand (`cef_mac.rs`), so nothing is linked.
- Linux: `libcef.so` is linked (RUNPATH `$ORIGIN`); releases always ship it.

## Proven on CI

`test/browser/browser_e2e.py` runs on one machine, in workflow
`.github/workflows/browser-e2e.yml` on windows-latest, macos-latest and
ubuntu-latest. It starts a hub on a temp home and the client on the runner's
desktop, and serves a page on loopback. The client runs a script of
`{"browser":{"op":…}}` steps. Each step makes the call the toolbar, mouse,
wheel or keys would make, on whichever page is shown
(`BrowserPage::script`, `CefPage::browser_op`). It checks:

- a local http page, a `file://` page and an https page load
- typing, scrolling, and a click that navigates
- back and forward
- a second tab opens and closes
- the Browser view closes and reopens and still navigates
- the client exits 0 by itself
- no process of the run is left after quit

It runs in three modes:

- `frames`: CEF off. In this mode the hub refuses a `file://` address from
  its toolbar by design (`term/driver.rs` `cef_url`), and the test checks
  for that refusal.
- `cef`: the runtime laid out by `scripts/cef-runtime-files.sh` and
  `scripts/cef-mac-helpers.sh`. A fallback to frames fails the job.
- fallback, run on Windows and macOS: the cef client with its runtime
  removed must start, say why, and show frames.

Results: see the table at the end.

The e2e found these bugs, all fixed:

- **macOS deadlock:** closing a tab hung the window. CEF runs
  `on_before_close` on the main thread inside `close_browser`, and
  `Tab::with` held the tab's browser lock across the call. It now takes the
  browser out of the lock first.
- **Windows typing:** typed text never reached the page. CEF makes WM_CHAR
  from `windows_key_code`, which carried 0 for scripted text and the virtual
  key for real keys (`A` for `a`). It now carries the character
  (`char_code`).
- **Windows fallback:** a client without `libcef.dll` could not start at
  all, because the DLL was linked at load time. Fixed by the delay-load.

## Not tried on real hardware

- **Windows:** no real Windows machine is reachable from here. Only the
  windows-latest runner was used: no GPU (WARP and SwiftShader), at 100%
  scaling.
- **macOS:** only the macos-latest runner was used, an arm64 VM with no GPU,
  where CEF ran with `BACKPLANE_CEF_NO_GPU`. The Mac mini was not used for
  this: the repo keeps it for builds (`e2e-macos.yml`), and a GUI run would
  draw in the person's logged-in session.
- **Unverified on both:** GPU compositing, HiDPI, IME, drag and drop,
  DevTools windows, and signing beyond ad-hoc.

## Risks

- **OSR copy:** rendering is the software OSR path on all platforms:
  `on_paint` BGRA is copied into a gpui image, at most 30 fps. The
  accelerated path (`accelerated_osr`: D3D11 shared handles, IOSurface) is
  not used.
- **macOS pump:** the external pump wakes gpui's main thread at least 30
  times a second while the engine is up, as cefclient does. Nested run loops
  (menus or modal dialogs inside a page) are untested.
- **Versions:** the `cef` crate is pinned (`=154.5.0`, CEF 154.0.34). A bump
  changes the API hash. The runtime is packed from the same build's OUT_DIR,
  so it cannot drift from the client.
- **Sandbox:** off on every platform. Windows would need `cef_sandbox.lib`
  and the bootstrap model; macOS would need sandboxed helpers.
- **Signing:**
  - macOS: everything is ad-hoc signed. Notarization would need a Developer
    ID, the hardened runtime with `allow-jit` /
    `allow-unsigned-executable-memory` on the helpers, and an `.app` or
    `.dmg`, since a tarball cannot be stapled.
  - Windows: no Authenticode signature.
- **Size:** the runtime adds about 560–600 MB uncompressed to the macOS and
  Windows installs, measured by `du` on the CI runners' dist folders.

## Effort left

| Piece | Days |
|---|---|
| Accelerated OSR, per OS | 3–5 |
| Sandbox, per OS | 2–4 |
| Developer ID signing and notarization (needs an identity, a .app or .dmg) | 2–3 |
| Real-hardware pass (GPU, HiDPI, IME) on a Windows PC and a Mac | 1–2 |

## Results

[browser-e2e run 42570253995](https://github.com/BackplaneWorks/Backplane/actions/runs/42570253995), all six jobs green:

| OS | frames | cef | fallback (cef client, runtime removed) | dist with runtime (debug build) |
|---|---|---|---|---|
| windows-latest (x64, no GPU) | 17/17 | 17/17 | 18/18 | 596 MB |
| macos-latest (arm64 VM, no GPU) | 17/17 | 17/17 | 18/18 | 573 MB |
| ubuntu-latest (x64, Xvfb, lavapipe) | 17/17 | 17/17 | n/a (libcef.so is linked) | 700 MB |
