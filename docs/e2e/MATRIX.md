# End-to-end matrix

Rust-only Backplane on `main`: hub `native/host`, client `native/gpui`. Results are `pass`, `fail` or `not run`. `deferred, no VM` marks Windows: the person dropped it for now on 2026-11-06, and it is neither a pass nor a fail. `pass, hub only` means the hub did it with no client involved, and `pass, part` names what ran: neither counts as a full pass. `persists, effect not proven` means the control saved and survived a restart, but nothing showed it doing anything, and that is not a pass. `wine` results sit in their own section: Wine on Boat is not Windows, and they never fill a Windows cell. A result needs evidence: a commit, the rig, and a log path or test name. `not run` says why. A Linux result never stands in for another platform.

The grids are kept by the lead. Each lane writes only its own section at the end of this file, so parallel rebases do not collide.

Other lanes own these, and their rows here are filled from their reports: onboarding, CI, `$` skills, voice and other parity additions, hub resilience.

## Features

| ID | Feature | Linux | macOS | Windows |
|---|---|---|---|---|
| | **Core** | | | |
| F01 | Hub boots on a temp home, `/hello` answers | pass | pass | deferred, no VM |
| F02 | Client connects; sidebar and projects load | pass | not run | deferred, no VM |
| F03 | Threads: create, rename, pin, archive, delete, settle, snooze, order | pass, hub only | pass, hub only | deferred, no VM |
| F04 | Stand-in turn streams; restart ends running turns as interrupted | pass, hub only | pass, hub only | deferred, no VM |
| F05 | Providers via stand-ins: Claude, Codex, Grok, OpenCode, Muse, Antigravity, Pi, Oh My Pi | not run | not run | deferred, no VM |
| F06 | Runtime modes, approvals, questions, plans | not run | not run | deferred, no VM |
| F07 | Queue, steer, stop, held queue resume | not run | not run | deferred, no VM |
| F08 | Fork, bring back, provider switch | not run | not run | deferred, no VM |
| F09 | Todos, context meter, drafts survive a crash | not run | not run | deferred, no VM |
| F10 | Inboxes, orchestration tools, subthreads panel | not run | not run | deferred, no VM |
| F11 | Model picker, effort, project picker, project badges | not run | not run | deferred, no VM |
| F12 | Git: worktree threads, checkpoints, diff, revert, commit and push, PR | not run | not run | deferred, no VM |
| F13 | Titles and the text generation model | not run | not run | deferred, no VM |
| F14 | Terminal | not run | pass, hub only | deferred, no VM |
| F15 | Attachments and image paste | not run | not run | deferred, no VM |
| F16 | Keybindings | not run | not run | deferred, no VM |
| F17 | Viewers: board, schematic, 3D, STEP | not run | not run | deferred, no VM |
| F18 | Browser tab (CEF) | not run | not run | deferred, no VM |
| F19 | Bots and rooms | not run | not run | deferred, no VM |
| F20 | File browser and downloads | not run | not run | deferred, no VM |
| F21 | Reconnect, resume from sequence, outbox, long session | not run | not run | deferred, no VM |
| F22 | Ordering flow up to the quote; no order placed | not run | not run | deferred, no VM |
| F23 | A killed hub leaves no agent behind | not run | fail | deferred, no VM |
| | **Install and CAD** | | | |
| C01 | Package installs and starts on a clean user | not run | not run | deferred, no VM |
| C02 | KiCad: install, Settings → CAD path, kicad-cli export and DRC run from the hub | pass | pass, part: ERC and DRC, no export | deferred, no VM |
| C03 | FreeCAD: install, Settings → CAD path, script run and STEP export from the hub | fail | pass, part: install not run | deferred, no VM |
| | **Remote operation** | | | |
| R01 | Restart the hub from another machine | not run | not run | deferred, no VM |
| R02 | Update: channel, Check now, download, checksum, swap, restart | not run | not run | deferred, no VM |
| R03 | Rollback after a bad update | not run | not run | deferred, no VM |
| | **Settings: each control persists across restart and takes effect** | | | |
| S01 | Appearance | persists, effect not proven | not run | deferred, no VM |
| S02 | Threads | persists, effect not proven | not run | deferred, no VM |
| S03 | Agents | persists, effect not proven | not run | deferred, no VM |
| S04 | Writing | persists, effect not proven | not run | deferred, no VM |
| S05 | CAD | pass, part: KiCad only | not run | deferred, no VM |
| S06 | Ordering | persists, effect not proven | not run | deferred, no VM |
| S07 | Devices | pass | not run | deferred, no VM |
| S08 | Network | persists, effect not proven | not run | deferred, no VM |
| S09 | Voice | fail | not run | deferred, no VM |
| S10 | Keys | not run | not run | deferred, no VM |
| S11 | Timezone | persists, effect not proven | not run | deferred, no VM |
| S12 | Usage | fail | not run | deferred, no VM |
| S13 | About | pass | not run | deferred, no VM |
| S14 | Privacy | pass | not run | deferred, no VM |

## Pairing

Rows are the hub, columns the client. Each cell is LAN / tailnet. Same-platform cells count too.

| Hub ↓ Client → | Linux | macOS | Windows | iOS | Android |
|---|---|---|---|---|---|
| Linux | not run / not run | not run / not run | deferred, no VM | not run / not run | not run / not run |
| macOS | not run / not run | not run / not run | deferred, no VM | not run / not run | not run / not run |
| Windows | deferred, no VM | deferred, no VM | deferred, no VM | deferred, no VM | deferred, no VM |

## Push notifications

Owned end to end by the push lane (branch `respectful-push`, thread t1793985890-103). No E2E lane takes these rows. Evidence goes in that lane's own section under `docs/e2e/`.

| ID | Feature | Owner | Result |
|---|---|---|---|
| P01 | APNs sender | `respectful-push` | not run |
| P02 | Uberbot-only gate | `respectful-push` | not run |
| P03 | Dedupe | `respectful-push` | not run |
| P04 | Quiet hours | `respectful-push` | not run |
| P05 | Deep link | `respectful-push` | not run |
| P06 | Badge | `respectful-push` | not run |
| P07 | Clear on any device | `respectful-push` | not run |
| P08 | Simulator evidence | `respectful-push` | not run |
| P09 | TestFlight | `respectful-push` | not run |

## Builders

| Platform | Builder | State on 2026-11-06 |
|---|---|---|
| Linux | desktop, `backplane`, Boat sandboxes | Builds are allowed on all three. UI rigs run on `backplane` or Boat, which have Xvfb. |
| macOS | `hs-mac-mini` (arm64) | Rust and Xcode present. KiCad and FreeCAD not installed. |
| Windows | none | deferred, no VM. The port was never finished: neither crate had `cfg(windows)` code when this was checked, and `docs/windows.md` still describes the removed Electron and Bun stack. |
| iOS | simulators on `hs-mac-mini` | Real phones only with the person present. |
| Android | emulator; Pixel 9 on the desktop | Real phones only with the person present. |

## Lane results

Each lane's rows: `| ID | result | commit | rig | evidence |`.

### Linux

| ID | Result | Commit | Rig | Evidence |
|---|---|---|---|---|
| F01 | pass | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | `test/e2e/f-rows.mjs`: temp home, `/hello` 200, data/1 hello frame, BACKPLANE_AGENTS read from `/proc/<pid>/environ` |
| F02 | pass | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | real client on Xvfb+i3: sidebar, 3 threads, bots, rooms, "Connected" (`rigsh/f02.sh`, shot kept by the lane). Without a window manager the window maps 1400x900 but never presents a frame (no `first_frame`); needs a WM, `XDG_RUNTIME_DIR` is not the cause |
| F03 | pass | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | hub only, through data/1 (`f-rows.mjs`): rename, pin, settle, archive, unpin, unarchive, delete; state survives a restart. Client UI not driven; order and snooze not run |
| F04 | pass | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | hub only (`f-rows.mjs`): stand-in turn streams an assistant entry; restart with `restart.continue` on resumes it ("Continuing after the restart"), off ends it (state idle, "Backplane restarted ... during this"). Client journeys `stream`, `reconnect` ran the real client: inputs ok, frame/click budgets fail on llvmpipe, so no client pass |
| F05 | not run | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | no driver for provider stand-ins other than claude (the stand-ins for the rest exit 1) |
| F06 | not run | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | no driver |
| F07 | not run | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | no driver |
| F08 | not run | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | no driver |
| F09 | not run | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | no driver |
| F10 | not run | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | no driver |
| F11 | not run | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | no driver |
| F12 | not run | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | no driver |
| F13 | not run | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | no driver |
| F14 | not run | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | no driver |
| F15 | not run | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | journey `send-image` blocked: client script step `paste_image` not implemented (G5) |
| F16 | not run | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | no driver |
| F17 | not run | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | journey `viewers` blocked: client script steps `open_viewer`, `pan_zoom` not implemented (G5) |
| F18 | not run | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | CEF build not made in this rig |
| F19 | not run | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | journey `bots-rooms` blocked (G5) |
| F20 | not run | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | journey `downloads` blocked (G5) |
| F21 | not run | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | journey `reconnect` ran the client (net_sleep step ok) but its budgets fail on llvmpipe; no resume/outbox check |
| F22 | not run | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | no driver |
| C01 | not run | 37c30e1cf8 | - | no package or clean-user install made |
| C02 | pass | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL), KiCad 10.0.8 from ppa:kicad/kicad-10.0-releases | `test/e2e/cad-rows.mjs`: `kicad.path` set, CAD page reads "version 10.0.8 · /usr/bin/kicad-cli", hub ran ERC+DRC (`review.checks`: ERC 43, DRC 23 warnings) and exports (`review.release`: gerbers, bom, cpl, pdf, step). Needed fix: the page never showed the CLI (landed meanwhile as 7e6eb6ff7a by the macOS lane). KiCad 7 from apt has no `sch erc`/`pcb drc`: checks fail there |
| C03 | fail | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL), FreeCAD 1.1.4 AppImage | hub meshes a STEP with FreeCAD only (`BACKPLANE_STEP2GLB=off`, wrapper logged the call; renders written; FreeCAD hidden: "could not mesh"; script run + STEP export via freecadcmd ok, `rigsh/c03.sh`, `c03b.sh`). Fails the row as written: Settings > CAD has no FreeCAD path control, only `BACKPLANE_FREECAD` or PATH |
| S01 | pass | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | appearance: 2 chips applied + persisted (`test/e2e/settings-walk.mjs`); theme effect in the client not checked |
| S02 | pass | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | threads: 29 chips and the webhook address applied + persisted; effect proven for `restart.continue` (F04 both ways), not for the others |
| S03 | pass | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | agents: provider, model, thinking, fallback chips stored and persisted; effect on a turn not proven |
| S04 | pass | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | writing: text provider and model stored and persisted; effect not proven |
| S05 | pass | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | cad: `kicad.path` Set stored, persisted, run by the hub (C02); `kicad.ipc` chip stored. The page showed "checking"/"not found" whatever was installed until 7e6eb6ff7a. No FreeCAD control (see C03) |
| S06 | pass | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | ordering: `order.enabled`, `order.estimates` applied + persisted (the estimates chips vanish while ordering is off, by design); effect on Buy it not driven |
| S07 | pass | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | devices: Add a device makes an invitation row; pairing itself is the pairing lane |
| S08 | pass | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | network: update channel chips stored + persisted; no update run (R lanes) |
| S09 | fail | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | voice: not in the hub menu (hub lists 12 of the client's 14 pages; `present/settings.rs` PAGES has no voice) |
| S10 | not run | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | keys: page opens; Save of a key not walked |
| S11 | pass | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | timezone: Set Europe/Helsinki shown on the page, stored, persisted |
| S12 | fail | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | usage: not in the hub menu (same as voice) |
| S13 | pass | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | about: one read-only row with the version; no controls |
| S14 | pass | 37c30e1cf8 | Boat bx_vnxbr9nr (Ubuntu 24.04, 4 vCPU, software GL) | privacy: Off applied; On did NOT re-enable (the chip sent `setting.set`, which the hub refuses for `telemetry.share=on`; no client sent `telemetry.choice`). Fixed in efa3eb26df and the presenter whitelist; Reset id applied + persisted |

### macOS

Hub-only rows drove the hub over its websocket with stand-in agents; no client ran (no window may open on the console session). Hub test suite on macOS: green after the fixes below (1919 passed, 0 failed, 7 ignored; `~/bp-e2e/logs/test-host.log`).

| ID | Result | Commit | Rig | Evidence |
|---|---|---|---|---|
| F01 | pass, hub only | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | `test/e2e/mac-hub-rows.mjs` on the Mac, log `~/bp-e2e/rows/`: GET /hello 200 CBOR, stand-ins only checked with `ps eww` (no /proc) |
| F02 | not run | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | no window may open on the console session (lead, 2026-11-06): GUI not run; the GPUI client builds on the Mac (`~/bp-e2e/dist/backplane-gpui`) but has no headless mode |
| F03 | pass, hub only | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | `test/e2e/mac-hub-rows.mjs` on the Mac, log `~/bp-e2e/rows/`: rename, pin, unpin, reorder, settle, unsettle, snooze, archive, unarchive, delete each logged |
| F04 | pass, hub only | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | `test/e2e/mac-hub-rows.mjs` on the Mac, log `~/bp-e2e/rows/`: stand-in turn streams (live deltas, assistant message, turn ends); with `restart.continue` off a hub restart leaves the running turn `interrupted` (default on: resumes it) |
| F05 | pass, hub tests only | b3ecb1d730 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | in-process hub tests with stand-in CLIs on macOS: `cargo test --release --bin backplane-hub providers` 149 passed (Claude, Codex, Grok, OpenCode, Muse, Antigravity, Pi, Oh My Pi), `~/bp-e2e/logs/test-host.log`. No process-level run of each provider through a websocket, no client |
| F06 | not run | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | needs ask/plan stand-in modes and the client |
| F07 | not run | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | `test/e2e/mac-hub-rows.mjs` on the Mac, log `~/bp-e2e/rows/`: steer-on-send and interrupt pass; the held queue (TurnQueued, queue.resume) was not reached: the message went into the running agent. Client half not run |
| F08 | not run | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | `test/e2e/mac-hub-rows.mjs` on the Mac, log `~/bp-e2e/rows/`: fork and provider switch accepted; `thread.back` refused on a non-fork. Bring back and the client half not run |
| F09 | not run | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | `test/e2e/mac-hub-rows.mjs` on the Mac, log `~/bp-e2e/rows/`: todos.edit logged. Context meter and drafts surviving a crash are client state: no window may open on the console session (lead, 2026-11-06): GUI not run |
| F10 | pass, hub only | b3ecb1d730 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | `test/e2e/mac-hub-rows.mjs` on the Mac, log `~/bp-e2e/rows/`: mail.send and mail.list, subthread create, detach, reparent all logged. Orchestration tools called by an agent and the subthreads panel: not run |
| F11 | not run | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | no window may open on the console session (lead, 2026-11-06): GUI not run |
| F12 | pass, part: no push, PR or checkpoint revert | b3ecb1d730 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | `test/e2e/mac-hub-rows.mjs` on the Mac, log `~/bp-e2e/rows/`: worktree thread (branch `backplane/...`), files.op write, git.status shows the new file, git.diff, git.act commit (git log shows the message), history.refs. Revert, push and PR not run (no remote, no GitHub) |
| F13 | not run | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | needs the text model; no real model turns |
| F14 | pass, hub only | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | `test/e2e/mac-hub-rows.mjs` on the Mac, log `~/bp-e2e/rows/`: `term.open`, input `echo bp-mac-term-$((40+2))`, shell output frame with 42 seen (pty on macOS) |
| F15 | not run | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | attachments and image paste are client side: no window may open on the console session (lead, 2026-11-06): GUI not run |
| F16 | not run | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | no window may open on the console session (lead, 2026-11-06): GUI not run |
| F17 | not run | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | no window may open on the console session (lead, 2026-11-06): GUI not run. Hub side: `mech.render` drew iso/front/top/right PNGs of a STEP on the Mac |
| F18 | not run | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | no window may open on the console session (lead, 2026-11-06): GUI not run |
| F19 | not run | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | `test/e2e/mac-hub-rows.mjs` on the Mac, log `~/bp-e2e/rows/`: bots.create x2 and bots.room logged (BotSet, RoomSet). Bot turns and the rooms UI not driven |
| F20 | not run | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | `test/e2e/mac-hub-rows.mjs` on the Mac, log `~/bp-e2e/rows/`: files.list, files.dir, files.read answered. Browser UI and downloads: no window may open on the console session (lead, 2026-11-06): GUI not run |
| F21 | not run | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | `test/e2e/mac-hub-rows.mjs` on the Mac, log `~/bp-e2e/rows/`: a join with `since` gets log frames. Outbox and long session not run |
| F22 | not run | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | order flow needs the client and the order server |
| F23 | pass | b3ecb1d730 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | `test/e2e/mac-hub-rows.mjs` on the Mac, log `~/bp-e2e/rows/`: stand-in agent pid alive, hub SIGKILLed, agent gone within 8 s (watcher process, `agents/proc.rs` `reaper`); unit `a_killed_hub_leaves_no_agent_behind` passes on macOS too (kills the group, grandchild included) |
| C01 | not run | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | no package built or installed on a clean user; binaries run from `~/bp-e2e/dist` |
| C02 | pass | b3ecb1d730 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | KiCad 10.0.8 in `~/Applications/KiCad/KiCad.app`, PATH=/usr/bin:/bin. Hub publishes `kicad.cli`; `review.checks` ran ERC and DRC (43 and 23 warnings); `review.release` (publish false) exported gerbers, board PDF, schematic PDF, BOM, CPL and STEP through kicad-cli into `.backplane/releases/v0.1` (`~/bp-e2e/cad5.sh`) |
| C03 | pass, part: install not run (FreeCAD 1.1.4 was already in `~/Applications`) | f06b78ce41 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | FreeCAD 1.1.4 in `~/Applications`. `test/e2e/mac-c03.mjs` (node), with `BACKPLANE_STEP2GLB=off` so FreeCAD is the only mesher and a wrapper in place of freecadcmd that logs each call: no `freecad.path` -> the page note shows the app's freecadcmd and `mech.render` writes 4 renders, wrapper not called; fpath typed + Set (`freecad-path`) -> `freecad.path` stored, note shows that path, `mech.render` starts the wrapper (logged) and writes 4 renders; a path that does not run -> the note shows the program the search found ("not found at that path" is only for when nothing is found), renders still written, wrapper not called; after a hub restart the stored path is used again (wrapper called). What an agent is told (`test/e2e/mac-c03-agent.mjs`): a stand-in agent wrote out its arguments (the system prompt) and environment and ran the FreeCAD named in the prompt with `--version`. No setting: the prompt names the app's freecadcmd and it printed `FreeCAD 1.1.4`; `freecad.path` set to the wrapper: the prompt names the wrapper, the wrapper logged `--version` and the real FreeCAD answered; a path that does not run: the prompt names the normal search result (before this fix it named the broken path, so the agent could not run FreeCAD; fixed by `told_freecad` in kicad/stepglb.rs, with a test) |
| S01 | not run | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | the Settings script of the Linux lane (settings-walk.mjs) drives the hub view over /proc-based helpers and was not given to this lane; GUI not run |
| S02 | pass, part: files.max.mb and restart.continue effects proven; the rest persist only | 6e44184379 | hs-mac-mini macOS 26.5 arm64, hub release build, no window | threads: 29 controls applied + persisted. Effects: restart.continue (F04); `test/e2e/mac-effects.mjs` (node, hub only): with files.max.mb=1 a 2 MB POST /attach is refused 413 `upload.cap` before the body is sent and nothing is stored, a 0.5 MB one is stored; raised to 3, the 2 MB upload is stored |
| S03 | not run | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | the Settings script of the Linux lane (settings-walk.mjs) drives the hub view over /proc-based helpers and was not given to this lane; GUI not run |
| S04 | not run | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | the Settings script of the Linux lane (settings-walk.mjs) drives the hub view over /proc-based helpers and was not given to this lane; GUI not run |
| S05 | not run | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | the Linux walk is not finished and reads /proc; the hub now publishes the CAD info the page shows (C02/C03) |
| S06 | not run | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | the Settings script of the Linux lane (settings-walk.mjs) drives the hub view over /proc-based helpers and was not given to this lane; GUI not run |
| S07 | not run | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | the Settings script of the Linux lane (settings-walk.mjs) drives the hub view over /proc-based helpers and was not given to this lane; GUI not run |
| S08 | not run | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | the Settings script of the Linux lane (settings-walk.mjs) drives the hub view over /proc-based helpers and was not given to this lane; GUI not run |
| S09 | not run | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | the Settings script of the Linux lane (settings-walk.mjs) drives the hub view over /proc-based helpers and was not given to this lane; GUI not run |
| S10 | not run | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | the Settings script of the Linux lane (settings-walk.mjs) drives the hub view over /proc-based helpers and was not given to this lane; GUI not run |
| S11 | pass | 6e44184379 | hs-mac-mini macOS 26.5 arm64, hub release build, no window | timezone: `test/e2e/mac-effects.mjs` (node, hub only): `setting.set person.tz` (now routed through zone.set, so the rule is stored with the zone; before, the time came out in machine time) for Asia/Tokyo, America/New_York, Europe/Helsinki; the hub's person_timezone tool, called as a bot's agent, reports that zone, offset and the local hour (matches the clock in that zone), and a daily 09:30 routine's next fire is 09:30 in that zone's offset (+09:00, -05:00, +02:00) |
| S12 | not run | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | the Settings script of the Linux lane (settings-walk.mjs) drives the hub view over /proc-based helpers and was not given to this lane; GUI not run |
| S13 | not run | 489505f966 | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | the Settings script of the Linux lane (settings-walk.mjs) drives the hub view over /proc-based helpers and was not given to this lane; GUI not run |
| S14 | pass, part: nothing is sent in any state | 6e44184379 | hs-mac-mini macOS 26.5 arm64, hub release build, no window | privacy: `test/e2e/mac-effects.mjs` walked Off (`setting.set`, stored off), On via `setting.set` (refused on purpose, `reserved` in settings/telemetry.rs: a stale click must not re-enable; stays off), On via `telemetry.choice` with the page's token (what the On button sends; stored on). The hub opened no non-loopback connection over 10 s in Off or On. This build has no sender or endpoint (`may_send` is always false), so the choice shows only in what is stored |

### Windows

Deferred, no VM. The lane stopped with nothing landed and never compiled or ran Windows.

Windows blockers, from reading the source at `ed50df6d35`, not from compile errors. The real error count is unknown.

- Hub terminal: POSIX pty only, needs ConPTY (`term/pty.rs`, `term/mod.rs`).
- Hub sockets: Unix sockets, and the KiCad IPC path needs a Windows transport (`kicad/nng.rs`, `kicad/kipc.rs`, `kicad/ipc.rs`, `proc.rs`).
- Hub processes: `kill`, `setsid`, `pre_exec` and `tokio::signal::unix` need Job Objects or an equivalent (`agents/proc.rs`, `agents/work.rs`, `term/pty.rs`, `settings/vault.rs`, `main.rs`, `copy.rs`, `agents/providers/mod.rs`).
- Hub file APIs: 24 `PermissionsExt`, 15 `symlink`, 5 `MetadataExt`, 4 `ExitStatusExt`, 4 `OpenOptionsExt`, 3 `DirBuilderExt`, 2 `FileExt`, plus `OsStrExt` and `AsRawFd`.
- Hub libc: 79 direct uses, including `flock`, `getifaddrs`, `localtime_r`, `setrlimit`, `O_NOFOLLOW` and `getuid`.
- Hub paths: Unix paths and tools hard-coded in at least 25 files, and 18 `HOME` or `XDG_` uses.
- Hub vault: only macOS keychain and Linux keyring backends (`settings/vault.rs`).
- Client: `Cargo.toml` declares `gpui` only for Linux and macOS. `gpui-0.2.2/build.rs` compiles Windows shaders only on a Windows build host. Linux or Unix-only paths in `screens/place.rs`, `screens/improvements.rs`, `screens/capture.rs` and `screens/timeline.rs`. CEF packaging is unproven.
- Helpers and packaging: no Windows handling in `tools/cef/CMakeLists.txt`, `tools/*/build.ts` or `scripts/`, apart from `build-step2glb.sh`.
- Docs: `docs/windows.md` and `docs/windows-cad.md` still describe the removed Electron and Bun stack.

| ID | Result | Commit | Rig | Evidence |
|---|---|---|---|---|

### Wine

Label `wine`. Wine is not Windows; nothing here fills a Windows cell.

Rig: Boat sandbox (4 vCPU, 8 GB, Ubuntu 24.04), source `26ba50a0ba49496a9baf89cba905e359f586ca5d` (origin/main when shipped), `cargo check --target x86_64-pc-windows-gnu`. Installed: the rust-std component for that target only. No Wine, mingw or xwin: `cargo check` needs none, and nothing built far enough to run.

| Check | Result | Evidence |
|---|---|---|
| Hub `cargo check`, Windows target | fail, 248 errors | `backplane-hub` bin 229, `bp-plot` 13, `screen-audit` 6. By area (all bins): files 55, settings 50, term 40, agents 36, bin 19, net 14, main.rs 13, bots 6, store.rs 5, commands 4, present 3, data 3. By kind: 98 E0599 no method (the Unix extension traits), 49 E0433 unresolved path, 78 E0425 missing value or function, 12 E0599 no associated item, 3 E0432 unresolved import. Needs the cfg(windows) work in "Windows blockers"; not small shims. |
| Client `cargo check`, Windows target | fail, 77 errors | `Cargo.toml` declares `gpui` only for Linux and macOS, so on Windows the `actions` macro and gpui types do not exist and every error is a cascade from that. Not a count of Windows porting work. |
| Hub starts, paths, API under Wine | not run | The hub does not compile. |
| GUI launches under Wine | not run | The client does not compile. |
| S01 | persists, effect not proven | cc53fca16a | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | `test/e2e/settings-walk.mjs` (node) on the hub's settings view: appearance: 4 controls applied, stored, lit after restart; effect in the client not checked |
| S02 | pass, part: files.max.mb and restart.continue effects proven; the rest persist only | 6e44184379 | hs-mac-mini macOS 26.5 arm64, hub release build, no window | threads: 29 controls applied + persisted. Effects: restart.continue (F04); `test/e2e/mac-effects.mjs` (node, hub only): with files.max.mb=1 a 2 MB POST /attach is refused 413 `upload.cap` before the body is sent and nothing is stored, a 0.5 MB one is stored; raised to 3, the 2 MB upload is stored |
| S03 | persists, effect not proven | cc53fca16a | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | `test/e2e/settings-walk.mjs` (node) on the hub's settings view: agents: 23 controls applied, stored, lit after restart; effect on a turn not proven |
| S04 | persists, effect not proven | cc53fca16a | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | `test/e2e/settings-walk.mjs` (node) on the hub's settings view: writing: 9 controls applied, stored, lit after restart; effect not proven |
| S05 | pass, part: KiCad only | cc53fca16a | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | `test/e2e/settings-walk.mjs` (node) on the hub's settings view: cad: kicad.path, kicad.ipc stored and persisted, run by the hub (C02); no FreeCAD control (C03) |
| S06 | persists, effect not proven | cc53fca16a | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | `test/e2e/settings-walk.mjs` (node) on the hub's settings view: ordering: 4 controls applied and persisted; effect on Buy it not driven |
| S07 | persists, effect not proven | cc53fca16a | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | `test/e2e/settings-walk.mjs` (node) on the hub's settings view: devices: page walked, controls applied and persisted |
| S08 | persists, effect not proven | cc53fca16a | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | `test/e2e/settings-walk.mjs` (node) on the hub's settings view: network: update.channel applied and persisted; no update run (R02) |
| S09 | fail | cc53fca16a | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | `test/e2e/settings-walk.mjs` (node) on the hub's settings view: voice: not in the hub menu |
| S10 | not run | cc53fca16a | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | `test/e2e/settings-walk.mjs` (node) on the hub's settings view: keys: page has no walkable control in the hub view |
| S11 | pass | 6e44184379 | hs-mac-mini macOS 26.5 arm64, hub release build, no window | timezone: `test/e2e/mac-effects.mjs` (node, hub only): `setting.set person.tz` (now routed through zone.set, so the rule is stored with the zone; before, the time came out in machine time) for Asia/Tokyo, America/New_York, Europe/Helsinki; the hub's person_timezone tool, called as a bot's agent, reports that zone, offset and the local hour (matches the clock in that zone), and a daily 09:30 routine's next fire is 09:30 in that zone's offset (+09:00, -05:00, +02:00) |
| S12 | fail | cc53fca16a | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | `test/e2e/settings-walk.mjs` (node) on the hub's settings view: usage: not in the hub menu |
| S13 | pass | cc53fca16a | hs-mac-mini macOS 26.5 arm64, hub release build, no window (console session logged in, no window allowed) | `test/e2e/settings-walk.mjs` (node) on the hub's settings view: about: read-only version row |
| S14 | pass, part: nothing is sent in any state | 6e44184379 | hs-mac-mini macOS 26.5 arm64, hub release build, no window | privacy: `test/e2e/mac-effects.mjs` walked Off (`setting.set`, stored off), On via `setting.set` (refused on purpose, `reserved` in settings/telemetry.rs: a stale click must not re-enable; stays off), On via `telemetry.choice` with the page's token (what the On button sends; stored on). The hub opened no non-loopback connection over 10 s in Off or On. This build has no sender or endpoint (`may_send` is always false), so the choice shows only in what is stored |

### Pairing and remote operation

| ID | Result | Commit | Rig | Evidence |
|---|---|---|---|---|

