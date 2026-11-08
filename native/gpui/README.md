# Backplane GPUI desktop client (experiment)

A native GPUI (Zed's Rust GPU UI, `gpui =0.2.2`) client speaking the same bounded view protocol as the phones:
WebSocket `/ws?view=1&mobile=1`, CBOR frames, `renderer: mobile`. It opts in to the bounded native sidebar
(`nativeSidebar: 1`, at most 48 rows, Newer/Older pager rows), reassembles `view.control` segmented replies, and
keeps every queue bounded. Linux (x11) is the target; macOS builds too (`scripts/mac-build.sh`, Metal via
`runtime_shaders`) but is not tested.

## Run on your own screen

    scripts/run-here.sh            # shared hub over the tailnet, your display and GPU

or directly: `build/gpui/linux/backplane-gpui --url http://HOST:PORT --token-file FILE`.

## Flags

- `--url http://HOST:PORT` (also `ws://`; `--hub` is an alias). Refuses a token file inside 0.14's home (`~/.backplane-bend`); port 3787 is allowed, since an updated 0.14 install keeps it.
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
Example: `test/shot-popup.json`.

## e2e hooks G1-G5 (test/e2e/CONTRACT.md)

Script steps (JSON, one per list item): `{"record":"start","name":"scroll"}` / `{"record":"stop"}`, `{"click":"<control id>"}`, `{"scroll":PX}`, `{"scroll_state":"<name>"}` (event `scroll {name, y, max, entries, dups, first, last, top, more}`),
`{"net_sleep":SECS}`; `{"open_viewer":"files"}`, `{"files":"select:N|open:N|key:down|filter:text|new:name|rename:N:name|delete:N|confirm_delete|menu:N|upload:PATH|download:N|zip|copy:N|up|back|cancel|shot:PATH"}` and `{"files_download":N|"name"}` are implemented (Files tab); `{"paste_image"|"open_settings": ...}` answer `input {ok:false,error:"not_implemented"}` (viewers, settings
and downloads do not exist in GPUI yet), so the harness marks those journeys blocked. Events (stderr, `GPUI_METRIC`):
- **G1 `ack {step, ack_ms}`**: for every scripted step, the first frame presented after the event was injected: its local effect (pressed/selected row, typed text),
  before any hub round trip. `input.latency_ms` stays the frame that shows the result. GPUI draws on demand, so "differs from the frame before" is
  the first frame after the state change, not a pixel diff.
- **G2 `frames {name, n, frames_ms:[...]}` (the key is not t_ms: the event time overwrites that)** at `record stop`: the start time of every frame drawn while the record was open (GPUI asks for frames while recording, so
  this is the achievable rate, not the idle rate).
- **G3 `layout {rects:{topbar,sidebar,main,rail,composer:{x,y,w,h}}}`** (physical px) whenever a stable region moved or resized; check modes only (`BACKPLANE_GPUI_RECTS=1`).
- **G4 `controls {controls:[{id,label,enabled,visible,wired}]}`** per screen on change (same switch); `enabled`/`wired` is false for a control with no handler yet (the
  rail buttons, Improvements, search, settings, attach, mic ... are listed and are dead today). `click {id, changed, action}` answers a click step: `changed` is true when the
  client's shown state moved or an action was sent; `action` is what was sent to the hub ("none" = dead).
- **G5**: `net_sleep` drops the socket, keeps the hub unreachable for SECS, reconnects with the normal backoff; events `net {event:"hub_reachable_again"}` and
  `net {event:"reconnected", ms}` (ms from wake to the first picture).
The input metric also carries the latency phases: `hub_rtt_ms, decode_ms, process_ms, apply_ms, render_build_ms, applied_to_frame_ms, bytes, delta`.

## Performance overlay

Ctrl+Shift+H (a choice in Settings, This device) shows the last 5 s of the window's work, top right: `src/perf.rs`. A row is `n`, mean, p50, p99 and max in ms.

- `frame`: render's start to `window.present()` returning (the GPU may still be working). Its steps: `build` (`Backplane::render`), `layout` (child views and the layout pass), `prepaint`, `paint`, `present`.
- `input>px`: a key, press or scroll to the next frame's end. An input that changes nothing is charged to the next frame within 1 s, because GPUI does not say whether it left one to draw.
- `hub>px`: a picture's frame arriving from the hub to the next frame's end. `hub rtt`: a view request sent to its reply; then `decode`, `merge` (delta or whole body) and `apply`.
- `fps`: frames drawn per second over the window, not the display's rate: GPUI draws on demand, so an idle window is near 0. `cpu`, `rss`, `peak`: the process (a Mac has `peak` only).
- `wire`: frames and bytes from the hub. `events`: what the hub thread gave the window. `pictures`, `deltas`: pictures applied, and how many came as deltas.
- `slowest paints`: regions (topbar, sidebar, main, composer, viewer, rail, over) by prepaint and paint time, heaviest first; timeline rows are built in `prepaint main`. `slowest events`: the window's handling of hub events. `overlay`: its own cost per frame.

Hidden, nothing is measured or kept and no mark or listener exists. Up, it updates with the frames the window draws anyway, at most 4 times a second, and asks for none but one after it opens, so an idle window shows its last busy moment. `{"read":"perf"}` in a script gives its text as a `read` event.

## Build (never on the desktop)

    scripts/boat-job.sh --size large --artifact build/gpui-artifacts -- sh scripts/boat-build.sh        # full build + tests
    scripts/boat-job.sh --size large -- sh scripts/boat-build.sh check                                   # cargo check only

Release profile: `opt-level=s`, fat LTO, `panic=abort`, one codegen unit, stripped. `native/gpui/vendor/xattr` is
xattr 0.2.3 with `ENOATTR` -> `ENODATA` on Linux (libc 0.2.191 dropped it).
Needs at run time: libxcb, libxkbcommon, libxkbcommon-x11, libgcc_s, libm, libc; Vulkan is loaded at run time.

## Test on an isolated display (never :0)

Xvfb is extracted from the .deb without root (`build/gpui/xvfb`); run it on `:61` with a throwaway i3 (an empty
config): without a window manager the window stays black. Then
`scripts/desktop-shot.sh NAME SCRIPT.json SECONDS [ICD]` runs the app against the seeded stand-in hub on
127.0.0.1:3821 (`scripts/desktop-hub.sh`) and saves a PNG under `build/gpui/shots/desktop/`.
GPU choice by env: `VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/{lvp,radeon,nvidia}_icd.json`
(llvmpipe, radv on the AMD iGPU, NVIDIA). See `build/gpui/results/desktop-gpu.md`.

## Known gaps

3D/STEP view, board/schematic viewer panels, project home, settings screens, Markdown inline styles, icons (text glyphs stand in),
multi-line composer. The sidebar is bounded by the protocol (<= 48 rows per page), not virtualized. The timeline is a list that draws only the rows on screen; it reads older pages as the reader nears the top (screens/history.rs) and holds at most 8192 rows.
