# iOS e2e journey driver

The 12 gate journeys (`../journeys.json`, same ids) on the iOS simulator with **real taps, typing and a real pasteboard paste**: an XCUITest
bundle (`E2E.xcodeproj`, `UITests/`) drives the installed app (`dev.backplane.gpui`) by bundle id through accessibility identifiers
(`thread.row.*`, `thread.composer`, `thread.send`, `viewer.open`, `viewer.choice.*`, `pair.address`, ...). Screen recording is `simctl io recordVideo`.

- `run.sh JOURNEY...|all` (Mac, under the `mac-heavy` lock): env `HUB_HOST HUB_PORT HUB_TOKEN_FILE SIM_UDID OUT`, optional `HUB_CTL LONG_SECS APP`.
  Writes `$OUT/<journey>.{json,mp4}`, per-step screenshots, `<journey>.xcresult`, `<journey>.assert.txt` (the XCTest failure text),
  `long-session.mem.csv` (footprint every 30 s) and `summary.json` in `../run.mjs`'s shape (+ `gate`).
- `drive.sh JOURNEY...|all [HUB_BIN]` (desktop side, ssh and rsync only): starts `../serve-fixture.mjs` on h@backplane (never this desktop), copies the
  token to the Mac, runs `run.sh`, brings the results back to `build/gpui/results/ios/e2e/`, stops the fixture by its pid and deletes its folder.
  Start it with `work_run {lock: "mac-heavy"}`.
- A step in the result json: `{do, tap_epoch_ms, tap_ms, result_ms, ok}`; `metrics.video_start_epoch_ms` is taken right before recording starts, so
  `lib/video-frames.mjs` can find the first changed frame after each tap in the mp4.

## Honest limits
- `tap_ms` includes XCUITest's wait for the app to be idle, and `result_ms` (tap -> the expected element exists) includes it too: they are an upper bound
  on latency, not the click-to-frame time. Ack and fps/jank come from the video.
- The journeys start paired through the app's debug hook `BACKPLANE_LINK` (only `pair` types the link into the pairing screen).
- `long-session` here is a loop of open thread / scroll / `@` popup for `LONG_SECS`; the 20 simulated agents are NOT started by `serve-fixture.mjs`, so it
  is a long session on the fixture's idle threads, not under the 20-agent load.
- Simulator timings, not a device's; software-rendered video.

## Blocked by the fixture, not by the app
- `bots-rooms`: the hub has no room (`rooms.create` refused or absent), so the Rooms list is empty (Bub opens fine).
- `viewers`: the fixture's project has no KiCad project the viewer lists ("No board/schematic in this project yet"); it needs `*.kicad_pro` (and
  `.backplane.json` naming it) next to the board/schematic, and a STEP the hub converts for 3D.
