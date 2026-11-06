# Mobile vs the GPUI desktop client (2026-11-06, second pass)

> Paths under `src/` below (`src/core`, `src/server`, `src/web`, `src/mobile`, `src/app`) name the Bend JS tree, removed from this checkout (tag `main-bend-final`). Where the current Rust code for each is, is unverified; look under `native/host/src/` and `native/gpui/`.

Audited against `native/gpui` (desktop), the Rust hub's phone API (`docs/PHONE-DATA.md`, proto `data/1`, `native/host/src/data/`), the iOS data app
(`mobile/ios/Backplane/Data*.swift`, branch gpui-ios, in `gpui`) and the new Android data app (`mobile/android/.../Data*.kt`).
Status: **done** (built and driven), **code** (written, not run: no Mac here), **gap** (not there), **hub** (the Rust hub gives phones no request for it yet).

## What changed
- The old phone apps spoke hub's view protocol through `bridge.js` (QuickJS / JavaScriptCore). The Rust hub speaks `data/1` to phones: plain JSON over
  `/ws?data=1&enc=json`, no presenter. iOS had a client for it; **Android now has one too** (`DataClient.kt`, `DataModels.kt`, `DataViews.kt`,
  `DataMarkdown.kt`, launcher `DataActivity`). Gradle `-PbpData=false` brings back the old launcher; the classic screens stay in the APK.
- The Android screens use the desktop's tokens (`BackplaneTheme.kt` = `src/web/style.css`, light and dark), JetBrains Mono for all text, square corners,
  flat panels, a `STATE ● finished` label, an `Ask anything: @bot, >thread, %project, $skill, /btw` composer with a blue Send, like `native/gpui`.

## Feature table
Status: **done** (built and driven on the emulator), **built** (iOS: compiled on the Mac, not run), **gap**, **hub** (the Rust hub gives phones no request yet).
| Feature | GPUI desktop | Android | iOS | Note |
|---|---|---|---|---|
| Pair, lists, cache, reconnect, open thread, stream, send, stop | done | **done** | built | data/1 base |
| Asks: approve (Allow / for session / Decline), option, answer | done | **done** (driven: stand-in `approve:`; card, Allow, "answered") | built | `ask.answer` |
| Pictures: attach (system picker), send, show | done | **done** (driven: picker, send, history, `attachment.get`) | built | hub fixed: the agents' `turn.start` dropped `attachments` |
| Markdown in replies (paragraph, heading, list, fence, quote, inline) | done | **done** | gap | tables: gap |
| New thread, add a project (folder browser), Improvements | done | **done** (driven) | built | |
| Thread actions: pin, settle, archive, rename, delete | done | **done** (row long-press, header menu; driven: pin, rename, settle) | built | `thread.act` |
| Project rename / remove | done | **built** (long-press menu; not driven) | built | `project.act` |
| Search projects and threads | partial | **done** (driven) | built | `search` |
| Picker under the composer (model, effort, provider, permissions), also for a new thread | done | **done** (driven) | **done** (run on a simulator) | `thread.picker`, `thread.pick` |
| Mention popup (@ > % # &) | done | **done** (driven: `@a` → `@ada`) | built | `mentions.list`; slash commands: gap |
| Bots and rooms: list, a bot's thread, a room's posts and post | done | **done** (driven) | built | uberbot Status tab, routines, spaces: gap |
| Alerts: a turn ended, failed, an ask opened (app in the background) | done | **done** (driven: notification in the shade, tap opens the thread) | built | hub `notify` push |
| Settings: hub, alerts, appearance, unpair | done | **done** | built | accounts, keys, voice, tailscale: gap |
| Viewer tab: Board (pinch, pan, tap to inspect, layer menu) | done | **done** (driven: plot drawn, a drag moves it, a tap on a track highlights it and shows net/layer/width, layer menu) | built | the hub's plot frames, drawn by the GPU view |
| Viewer tab: Schematic, 3D (the board's model, orbit) | done | **done** (driven) | built | |
| Viewer tab: Mech (STEP parts, orbit) | done | **done** (driven: bracket.step) | built | needs console FreeCAD on the hub (see below) |
| Viewer tab: Files (browse, image, PDF, text, Markdown; board/schematic/STEP open in their tab) | done | **done** (driven) | built | `files.entries`, `/files/get` |
| Viewer tab: Browser (frame, take over, tap = click, address, back, hand back) | done (frames; input partial) | **done** (driven against a planted frame; the click reaches the hub) | built | no browser helper ran here, so a live page was not driven |
| Viewer tab: Split (schematic, board, 3D; stacked, side by side in landscape) | done | **done** (driven: three panes drawn) | built | `viewer.watch split` |
| Viewer tab: Diff (unified, + green, - red) | done | **done** (driven: a change made after the first turn shows) | built | `viewer.diff` |
| Viewer tab: Review (Checks with the checklist, BOM, Comments: add, resolve) | done | **done** (driven: DRC findings and the checklist, a comment added and resolved) | built | `review.*`; Release page: gap |
| Pinch zoom on the board | done | **done** (driven: a two-finger pinch by `sendevent`, `scripts/mobile/android-pinch.sh`) | built | the emulator image allows `adb root` |
| Browser tab against a live page | done (CEF child window) | **done** (driven: the helper's Chrome page shown, a tap clicked its button, the page's server heard it) | built | frames route (Option A of docs/gpui/BROWSER-EMBED.md); the CEF lane's embedded window does not publish to `screen.jpg` |
| Terminal | done | gap | gap | |
| Thread tree, fork, snooze, reorder, badges | done | gap | gap | no `data/1` request |
| Tool-call folds, stamps | done | **done** (driven) | **done** (run on a simulator) | `DataFold`, `test/mobile/fold_labels.json` |
| To-do list, subagents panel | done | gap | gap | |
| Desktop's right rail, tabs, ctx meter | done | n/a on a phone | n/a | |

## Evidence
- Hub: Rust `backplane-hub` built here (`cargo build --profile fast`, own target dir), seeded by `scripts/mobile/data-hub.sh` (a project, threads, a bot, a room; stand-in agent).
  `cargo test --profile fast --bin backplane-hub -- data:: notify_for turn_start capabilities`: 12 pass (two new: additions, notify). `bun test/mobile/data_protocol.mjs`: 20 checks pass (base, additions, picture round trip).
- Android: `scripts/mobile/android-e2e.sh` on the API 34 emulator drives the UI through uiautomator and asserts at every step (pair → search → pin → rename → model picker → settle → mention popup → ask approve → bot thread → room post → add project → attach and send a picture → background notification → Improvements → a thread started). "all steps passed"; screenshots `build/mobile/shots/e2e-*.png`.
- iOS: `scripts/mobile/ios-check.sh` rsyncs `mobile/ios` to its own folder on hs-mac-mini and runs xcodebuild (Debug simulator, `-DDATA_APP`): the build succeeds and the new views are in the binary. Nothing was run in the simulator.
- Hub bugs found by driving it: the agents' `turn.start` ignored `attachments`/`agentText` (a picture was never in the history); a model chosen within a provider was not what `thread_json` showed (ModesSet, not ProviderSet); a hub entry without an id crashed the Android list (now keyed by position).

## Not tested
- iOS: nothing run (simulator or device). Swift for the new screens is compile-checked only.
- Android: project rename/remove, the load-earlier page, reconnect after sleep, a physical device. Only the emulator.
- The stand-in agent never fails a turn, so the "fail" notification is covered by the hub unit test only.

## Viewers (third pass)
- The hub side is `native/host/src/data/ext.rs` (features `viewers`, `files`, `browser`): the plot frames the desktop gets go to the phone as binary frames on its data socket. `bun test/mobile/data_protocol.mjs`: 30 checks, among them a board plot arriving, `files.entries`, `mech.list`, `/files/get` (and its refusal outside the project), the browser frame and the click rule.
- Android reuses the app's own plot decoding and GPU view (`PlotStore`, `PlotSurface`), not a new Canvas renderer: it already did pinch, pan, orbit, tap-to-pick, fades and layer masks for the old protocol, so the new screens (`DataViewer.kt`) only wire the data/1 requests to it and add the desktop's look (mono text, square panels, the same tokens).
- iOS: `DataViewer.swift` does the same over `PlotStore`/`PlotCanvasView` (Metal). Compiled on the Mac (`scripts/mobile/ios-check.sh`); never run.
- A machine note: on this desktop FreeCAD's GUI mesher never finishes headless (`ImportGui.insert` waits forever with the person's ImportMode; Bub's fix is in tools/step2glb on gpui-next, not merged here). `scripts/mobile/data-hub.sh` runs its test hub with the console FreeCAD only (`BACKPLANE_FREECAD`, a PATH without the GUI binary), which converts a STEP at once.

## Next
1. When the viewer/files/browser requests land in `native/host`, add them to `data/1` as features and build the tabs on both phones (the classic `PlotView.kt`/`.swift` draw boards from chunks; reuse them).
2. iOS: run the data app in the simulator against `scripts/mobile/data-hub.sh` and drive it; Markdown renderer; the desktop's mono styling.
3. Android: reconnect-after-sleep and long-session runs, slash commands, thread tree.
