# Backplane GPUI desktop client (experiment)

A native GPUI (Zed's Rust GPU UI, `gpui =0.2.2`) client speaking the same bounded view protocol as the phones:
WebSocket `/ws?view=1&mobile=1`, CBOR frames, `renderer: mobile`. It opts in to the bounded native sidebar
(`nativeSidebar: 1`, at most 48 rows, Newer/Older pager rows), reassembles `view.control` segmented replies, and
keeps every queue bounded. Linux (x11) is the target; macOS builds too (`scripts/gpui/mac-build.sh`, Metal via
`runtime_shaders`) but is not tested.

## Run on your own screen

    scripts/gpui/run-here.sh            # shared hub over the tailnet, your display and GPU

or directly: `build/gpui/linux/backplane-gpui --url http://HOST:PORT --token-file FILE`.

## Flags

- `--url http://HOST:PORT` (also `ws://`; `--hub` is an alias). Refuses port 3787 and the live home.
- `--token-file FILE` (default `~/.backplane-gpui/token`; the token is never logged).
- `--exit-after SECONDS` quit after that long.
- `--script FILE` or env `BACKPLANE_GPUI_SCRIPT=FILE`: replay input steps (below). The app quits 300 ms after the last step.

## Metrics contract (stderr, one JSON object per line, prefix `GPUI_METRIC `)

Every line has `event`, `t_ms` (since process start) and `epoch_ms`.

- `process_start` at the top of `main`.
- `gpu`: `device`, `driver`, `driver_info`, `software` (the Vulkan adapter actually used; all numbers depend on it).
- `first_frame`: the frame after the first render was presented (measured at the next frame's start, so up to one frame late).
- `usable`: sidebar rows (`threads`) drawn and the editable composer on screen; `entries` is the open thread's count (0 when none is open).
- `input`: one per scripted step: `step`, `ok`, `latency_ms` from issuing the step to the start of the frame that shows its result
  (`select:N` thread's picture arrived and drawn; `popup:/` popup rows drawn; `type:N` text visible; `send` the user's message appears). A step whose result never shows is reported `ok:false` after 15 s.
- `script_done` when all steps finished.
- Not metrics: `GPUI_LOG {"disconnect": "..."}` real connection errors (token masked).

Script file: a JSON list of steps, run after `usable`, one at a time:
`{"select":N}` (Nth thread row, sidebar order), `{"popup":"/"}` (type that text and wait for popup rows),
`{"type":"text"}` (append to the composer), `{"send":true}`, `{"wait":MS}`.
Example: `test/gpui/shot-popup.json`.

## Build (never on the desktop)

    scripts/boat-job.sh --size large --artifact build/gpui-artifacts -- sh scripts/gpui/boat-build.sh        # full build + tests
    scripts/boat-job.sh --size large -- sh scripts/gpui/boat-build.sh check                                   # cargo check only

Release profile: `opt-level=s`, fat LTO, `panic=abort`, one codegen unit, stripped. `native/gpui/vendor/xattr` is
xattr 0.2.3 with `ENOATTR` -> `ENODATA` on Linux (libc 0.2.191 dropped it).
Needs at run time: libxcb, libxkbcommon, libxkbcommon-x11, libgcc_s, libm, libc; Vulkan is loaded at run time.

## Test on an isolated display (never :0)

Xvfb is extracted from the .deb without root (`build/gpui/xvfb`); run it on `:61` with a throwaway i3 (an empty
config): without a window manager the window stays black. Then
`scripts/gpui/desktop-shot.sh NAME SCRIPT.json SECONDS [ICD]` runs the app against the seeded stand-in hub on
127.0.0.1:3821 (`scripts/gpui/desktop-hub.sh`) and saves a PNG under `build/gpui/shots/desktop/`.
GPU choice by env: `VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/{lvp,radeon,nvidia}_icd.json`
(llvmpipe, radv on the AMD iGPU, NVIDIA). See `build/gpui/results/desktop-gpu.md`.

## Known gaps

3D/STEP view, board/schematic viewer panels, project home, settings screens, Markdown inline styles, icons (text glyphs stand in),
multi-line composer. The timeline and sidebar are bounded by the protocol (<= 32 entries, <= 48 rows per page), not virtualized.
