# Embedded Chromium on Windows x64 and macOS arm64

Status and design of the native CEF port of the Browser tab. Linux x64 has
shipped embedded Chromium since 0.15 (`docs/browser-native.md`). Before this
port, macOS and Windows showed only the hub's JPEG frames from
`backplane-browser`. Those frames are still the fallback on every OS.

## What the client does per OS

| | Linux x64 | Windows x64 | macOS arm64 |
|---|---|---|---|
| CEF library | `libcef.so`, linked, RUNPATH `$ORIGIN` | `libcef.dll`, linked with `/DELAYLOAD:libcef.dll` (`native/gpui/build.rs`) | framework loaded by hand (`cef::load_library`), nothing linked |
| Runtime found in | `$BACKPLANE_CEF`, `<cache>/backplane/cef-runtime/<ver>`, beside the exe | the same | `$BACKPLANE_CEF`, `cef/` beside the exe, `../Frameworks` in an app bundle |
| Checked before the first CEF call | `NEED_LINUX` | `NEED_WINDOWS` (libcef.dll, chrome_elf.dll, paks, icudtl.dat, snapshot, locales/en-US.pak) | `NEED_MACOS` (framework binary, icudtl.dat, the five helper bundles) |
| Helper processes | the client, re-run with `--type=` | the client, re-run with `--type=` (`BACKPLANE_CEF_DLL_DIR` passes a runtime directory that is not beside the exe) | `Backplane Helper{, (GPU), (Renderer), (Plugin), (Alerts)}.app`, whose executable is `backplane-cef-helper` (`src/bin/backplane-cef-helper.rs`) |
| Message loop | CEF's own UI thread (`multi_threaded_message_loop`) | the same | `external_message_pump`: `do_message_loop_work` on gpui's main thread when CEF asks (`on_schedule_message_pump_work`), and at least every 33 ms while the engine is up (`viewers/cef.rs` `pump`) |
| NSApp | n/a | n/a | `isHandlingSendEvent` / `setHandlingSendEvent:` and the Cr/Cef app protocols are added at run time to gpui's application class (`viewers/cef.rs` `mac::conform_app`) |
| Sandbox | off (`no_sandbox`) | off | off |
| Rendering | OSR, BGRA copied from `on_paint` into a gpui image | the same | the same |

`subprocess()` decides from argv alone whether the process is a CEF helper
(`is_helper_argv`). No CEF function runs before that check, and none runs
before the runtime check. A missing or incomplete runtime is not a crash: the
Browser tab says `embedded browser unavailable: …` in its banner and shows the
hub's frames. On Windows a delay-loaded DLL that is missing would raise an
exception at the first call, which is why the check happens first. On Linux a
client without `libcef.so` cannot start at all, as before. Releases always
ship it there.

## Layout in the release archives

- Windows zip: `libcef.dll`, `chrome_elf.dll`, `d3dcompiler_47.dll`,
  `dxcompiler.dll`, `dxil.dll`, `libEGL.dll`, `libGLESv2.dll`,
  `vk_swiftshader.dll`, `vk_swiftshader_icd.json`, `vulkan-1.dll`, the paks,
  `icudtl.dat`, `v8_context_snapshot.bin` and `locales/`, beside
  `backplane-gpui.exe`.
- macOS tarball: `cef/Chromium Embedded Framework.framework`,
  `cef/Backplane Helper*.app` (five bundles) and `cef/Backplane.app` beside
  `backplane-gpui`. `Backplane.app` is a stub bundle that holds only an
  Info.plist: the client is not in a bundle, so CEF is given this one as
  `main_bundle_path`. Everything is ad-hoc signed.
- `scripts/cef-runtime-pack.sh windows|macos TARGET_DIR DEST` lays out either
  one from cef-dll-sys's OUT_DIR, and fails when a file is missing.
  `release.yml` and `browser-e2e.yml` both use it.

## Tests

`test/browser/browser_e2e.py` runs on one machine with a temp hub, the client,
a loopback page server and `{"browser":{"op":…}}` script steps. Those steps
make the same calls the toolbar, mouse, wheel and keys make, on whichever page
is shown. It checks:

- a local http page, a `file://` page and an https page load
- typing, scrolling, and a click that navigates
- back and forward
- a second tab opens and closes
- the Browser view closes and reopens and still navigates
- the client exits 0 by itself
- no process whose command line names the run's folder, the helper or the client is left after quit

`.github/workflows/browser-e2e.yml` runs it on windows-latest, macos-latest and
ubuntu-latest in three modes:

- `frames`: CEF off
- `cef`: the packed runtime; falling back to frames fails the job
- fallback, part of the `cef` jobs on Windows and macOS: the cef client with
  no runtime beside it must start, say why, and show frames

## Status

(The results table is in the section below, updated with each CI run.)

## Risks and open items

- **OSR per OS.** Rendering is the software OSR path (`on_paint` BGRA) on all
  three OSes, with no shared textures: a copy per frame, at most 30 fps. The
  accelerated path (`accelerated_osr`, D3D11 shared handles on Windows,
  IOSurface on macOS) is not used. It would need the cef crate's feature and a
  gpui surface import, about 3 to 5 days per OS.
- **macOS message pump.** The 33 ms floor means 30 idle wakeups a second while
  a CEF engine is up (it starts on the first Browser tab and stays up). That
  is CEF's own recommendation for external pumps, and it is not a repaint.
  Nested run loops (menus, modal dialogs inside the page) are untested.
- **NSApp protocol.** The methods are added to gpui's class at run time rather
  than by subclassing. `sendEvent:` is not wrapped, so `isHandlingSendEvent`
  is always false. If Chromium ever needs it to be true (drag and drop,
  nested event tracking), gpui's `sendEvent:` override in `vendor/gpui` would
  need a wrapper.
- **CEF and crate versions.** The `cef` crate is pinned at `=154.5.0`
  (CEF 154.0.34). Its build script downloads the binary distribution for the
  target (about 120 MB on Windows, 110 MB on macOS, bz2) and builds
  `libcef_dll_wrapper` with CMake and Ninja. Windows needs MSVC and Ninja on
  PATH; macOS needs Xcode's clang. A crate bump changes the API hash, and the
  runtime shipped beside the client must be the same build. The packing
  script takes it from the same build's OUT_DIR, so they cannot drift.
- **Sandbox.** Off everywhere (`no_sandbox`).
  - Windows needs `cef_sandbox.lib` linked statically into the exe plus the
    `bootstrap.exe` model of recent CEF. The crate's `sandbox` feature exists
    but is untested here.
  - macOS needs the sandbox helper library and `cef::sandbox` in each helper.
  - About 2 to 4 days per OS, mostly signing and verification.
- **Signing and notarization (macOS).** Releases are ad-hoc signed today, both
  the client and the CEF bundles. With a Developer ID,
  `cef-runtime-pack.sh` signs with `CODESIGN_ID` and the hardened runtime,
  and gives the helpers `allow-jit`, `allow-unsigned-executable-memory` and
  `disable-library-validation`. Notarization would then need the whole
  archive submitted with `notarytool` and stapled, which cannot be stapled to
  a bare tarball, so it would need a `.app` or `.dmg`. Not done: there is no
  identity in CI.
- **Signing (Windows).** Nothing is Authenticode-signed today. CEF's DLLs come
  from the Spotify CDN, unsigned by us. SmartScreen treats the exe as before.
- **GPU on runners.** The CI runners have no GPU. The cef jobs set
  `BACKPLANE_CEF_NO_GPU` (software compositing). Real-GPU behaviour is
  verified only where noted in the table below.
- **Size.** The CEF runtime adds roughly 250 to 300 MB uncompressed (about
  110 to 130 MB compressed) to each of the Windows and macOS archives. Exact
  figures are in the CI log line `CEF runtime size:`.
- **Updater.** The guard replaces the install folder's files from the
  archive, so the new `cef/` folder (macOS) and the DLLs (Windows) arrive with
  the next update like any other file. An older client left beside a newer
  runtime cannot happen through the updater: both come from one archive.

## Effort

| Piece | Days | State |
|---|---|---|
| Per-OS runtime discovery and checks, delay-load, the macOS loader and helper binary | 1 | done |
| macOS main-thread pump and the NSApp protocol | 1 | done; CI result below |
| Packing (`cef-runtime-pack.sh`), release.yml, archive checks | 1 | done |
| e2e on three OSes (frames, cef, fallback) | 1 | done |
| Fixes found by the runners | 1 to 3 | see the table |
| Accelerated OSR per OS | 3 to 5 each | not started |
| Sandbox per OS | 2 to 4 each | not started |
| Developer ID signing and notarization (needs an identity, a .app or .dmg) | 2 to 3 | not started |

## Results

Filled from CI (`browser-e2e`) and from real machines:

| OS | frames | cef | fallback | Run |
|---|---|---|---|---|
| windows-latest | pending | pending | pending | |
| macos-latest | pending | pending | pending | |
| ubuntu-latest | pending | pending | n/a | |
