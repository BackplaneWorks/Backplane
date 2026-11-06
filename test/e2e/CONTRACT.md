# What each client must offer the e2e harness

The harness (`run.mjs`) is client-agnostic: `journeys.json` says what to do, an adapter in `adapters/` makes one client do it and
returns measurements. A capability an adapter lacks makes the journey `blocked: <capability>`; nothing is ever marked pass on
missing evidence. Statuses map to `docs/final-gate.json` as: pass -> pass, fail -> fail, blocked/not run -> open.

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
(= `result`), `script_done`, and now `ack`, `frames`, `layout`, `controls`, `click` (see the table below).

### Hooks (G1-G5): owner and landing commit
Owner of every GPUI hook: the GPUI desktop-client stream (thread t1793832589-1684371); the screens lane (t1793862844-1750692) owns the popup range and the screens behind G5. Landing commits are on the `gpui` branch.
| id | hook | owner | landing commit | status |
|---|---|---|---|---|
| G1 | event `ack {step, ack_ms}`: the first presented frame after the injected event. **G1 decides the one-frame claim** ("every click answers within one frame" is judged on this `ack_ms` against one refresh period, never on `input.latency_ms` and never on the recording) | GPUI desktop stream | `c373ceb37` | landed; not yet consumed by the adapter on a real run |
| G2 | steps `{"record":"start","name":..}` / `{"record":"stop"}`; event `frames {name, n, t_ms:[..]}` with a timestamp per drawn frame (**defect at `c373ceb37`: `metric()` overwrites the payload's `t_ms` with the event time, so the array never reaches the log; it must be sent as `frames_ms`**) | GPUI desktop stream | `c373ceb37` | landed |
| G3 | event `layout {rects:{id:{..}}}` on change (probes painted last frame) | GPUI desktop stream | `c373ceb37` | landed; emitted only when rect reporting is enabled (env `BACKPLANE_GPUI_RECTS=1`, or the check modes), which the adapter sets for the layout and dead-control journeys |
| G4 | event `controls [{id,label,enabled,visible,wired,disabled_look}]` on change (`disabled_look:true` = no handler and drawn disabled, from `839a84025`); step `{"click":"<id>"}` -> event `click {id, found, changed, action}` (a click with no change is reported after 1.5 s: a dead control) | GPUI desktop stream | `c373ceb37` | landed; same rect-reporting caveat |
| G5 | steps `scroll px` and `net_sleep secs` (drop the hub socket and wait for the reconnect) | GPUI desktop stream | `c373ceb37` | landed |
| G5 | steps `paste_image`, `open_viewer`, `open_settings`, `files_download` | screens lane | not landed | answer `ok:false, error:"not_implemented"` today; pair, bots/rooms and the `@ > %` popup range are not in the script contract yet either |
The adapter maps the not-landed steps to `blocked` and reads the landed hooks (`ack`, `frames`, `layout`, `controls`, `click`); `adapter.test.mjs` drives it with a fake client speaking this contract. No real run has used them yet: the Linux binary in `build/gpui/linux/` (10:41 of 5 Nov 03:41) predates `c373ceb37` (03:58) and needs a rebuild on Boat.

### Pass rule (the person, 2026-11-05)
No journey is `pass` until it has run against the **Rust hub** on **real hardware** (hardware GPU or a device, not llvmpipe). `run.mjs` enforces it: a result the adapter would call pass is demoted to `partial` unless `--hub-kind rust` and `--real-hardware` are given and the client reported `software:false`. Boat runs (software, JS hub) can therefore never close a gate item.

### Pending reruns
When the screens lane lands the popup range (`@`, `>`, `%`, `#`), rerun the `popups` journey and the `long-session` random walk and confirm each trigger opens with the hub's own rows (the GPUI popup step only succeeds when the thread's skills/refs list is non-empty).

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

### Dead buttons, how the sweep reads `disabled_look`
A visible control with a handler that, when clicked, changes nothing and emits nothing is **dead**. A visible control with no handler is reported separately: `disabledNoHandler` when GPUI draws it disabled (`disabled_look:true`), `deadLooksEnabled` when it is drawn as live. The gate says every visible button must do something, so `dead.pass` is false while any visible control lacks a handler; `dead.passIfDisabledAccepted` is the reading in which disabled-looking placeholders are fine. That choice belongs to the reviewer; the harness reports both and never decides it.
