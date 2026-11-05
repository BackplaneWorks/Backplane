# What each client must offer the e2e harness

The harness (`run.mjs`) is client-agnostic: `journeys.json` says what to do, an adapter in `adapters/` makes one client do it and
returns measurements. A capability an adapter lacks makes the journey `blocked: <capability>`; nothing is ever marked pass on
missing evidence. Statuses map to `docs/gpui/final-gate.json` as: pass -> pass, fail -> fail, blocked/not run -> open.

## Step vocabulary (journeys.json)
`pair`, `open_project`, `select_thread`, `scroll`, `type`, `paste_image`, `send`, `wait_reply`, `watch_stream`, `switch_burst`, `popup`,
`open_bots/open_bot/open_rooms/open_room/post_room`, `open_viewer{board|schematic|3d}`, `pan_zoom`, `orbit`, `open_files`, `download`,
`open_settings`, `click_every_control`, `sleep_network/wake_network`, `agents`, `session`, `expect`.

## Measurements and how they are defined (lib/frames.mjs, unit-tested)
- **Click -> next frame.** `ack` = first presented frame that differs from the one before the click (pressed, hover, focus or the new
  state); `result` = the frame that shows the expected state (that one includes any hub round trip). "Every click answers within one
  frame" is judged on `ack` against one refresh period (60 Hz GPUI desktop, 120 Hz phones where the screen allows; the budget is in
  `journeys.json`). `result` is reported next to it so a slow hub is not hidden behind a fast ack.
- **Frame times during scroll, pan, orbit and streaming:** presentation timestamps -> fps, p50/p95/p99 interval, janks (interval over
  two periods), dropped-frame estimate. Pass: at least 10 frames, janks <= 1 %, p99 <= 2 periods.
- **Layout shift:** snapshots of every stable element's rectangle; shifts that happen without a user input in the previous 500 ms
  count, scored like the web CLS (impact x distance); pass <= 0.1.
- **Dead controls:** inventory of visible, enabled controls on each screen; each is clicked once; a control whose click changes neither
  the shown state nor emits an action is dead. A control never clicked is not assumed alive.
- **Memory over the long session:** RSS/PSS of the client tree sampled every 30 s, slope after 5 min.

## GPUI (native/gpui) — exists today
Script file (`--script`, `BACKPLANE_GPUI_SCRIPT`) steps: `select N`, `popup TEXT`, `type TEXT`, `send`, `wait MS`.
Metrics on stderr, one `GPUI_METRIC {json}` per line: `process_start`, `gpu`, `first_frame`, `usable`, `input {step, ok, latency_ms}`
(= `result`), `script_done`. That is enough for: switch-fast, popups, select, type, send; not for ack, frames, layout or controls.

### Requested additions (for the `desktop` stream; the GPUI adapter calls them as soon as they exist)
| id | request | feeds |
|---|---|---|
| G1 | event `ack {step, ack_ms}`: first presented frame that differs after the injected event | click -> frame |
| G2 | event `frame {t_ms}` for every presented frame (or `frames [..]` batched per second) while a script runs; step `{"record":"start|stop","name":..}` | scroll/stream frame times |
| G3 | event `layout {t_ms, rects:{id:{x,y,w,h}}}` for the stable elements, on every change | layout shift |
| G4 | event `controls [{id,label,enabled,visible}]` per screen, and step `{"click":"<id>"}` that reports `{changed, action}` | dead buttons |
| G5 | steps `scroll N`, `paste_image PATH`, `open viewer KIND`, `open settings`, `files download NAME`, `net_sleep SECS` (drop the hub socket, reconnect) | the other journeys |
Until G1-G5 land those journeys are `blocked` with the id above.

## Android — tooling that exists on any device
`adb shell input tap/swipe`, `uiautomator dump` (controls: clickable/enabled/bounds, so the dead-control sweep and layout snapshots need
no app support), `dumpsys gfxinfo <pkg> framestats` (per-frame timestamps and jank), `screenrecord`. Pairing, popups and viewers are driven by
tapping the controls found in the dump; a journey step the dump cannot find is `blocked: <control not found>`.

## iOS — Mac only (`mac-heavy` lock), simulator
`xcrun simctl launch/io recordVideo/io screenshot`; control inventory and taps need either an XCUITest bundle or the app's debug action
hook (`BACKPLANE_ACTS`); frame data from `xctrace` hitches. None of that is wired yet: the adapter reports `blocked: ios_driver`.

## Hub
`lib/hubfx.mjs` starts the hub under test (Rust `backplane-hub` or the JS hub) on a fresh home with stand-in agents only (the hub's
`BACKPLANE_AGENTS` is read back from `/proc/<pid>/environ` first) and seeds a project, threads with history, an image, a board, a
schematic, a STEP, a text file, bots and rooms. Any seed RPC the hub refuses is written to `seedReport`; journeys that need it are
`blocked: hub:<method>`.
