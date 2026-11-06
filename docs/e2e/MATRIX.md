# End-to-end matrix

Rust-only Backplane on `main`: hub `native/host`, client `native/gpui`. Results are `pass`, `fail` or `not run`. `deferred, no VM` marks Windows: the person dropped it for now on 2026-11-06, and it is neither a pass nor a fail. `wine` results sit in their own section: Wine on Boat is not Windows, and they never fill a Windows cell. A result needs evidence: a commit, the rig, and a log path or test name. `not run` says why. A Linux result never stands in for another platform.

The grids are kept by the lead. Each lane writes only its own section at the end of this file, so parallel rebases do not collide.

Other lanes own these, and their rows here are filled from their reports: onboarding, CI, `$` skills, voice and other parity additions, hub resilience.

## Features

| ID | Feature | Linux | macOS | Windows |
|---|---|---|---|---|
| | **Core** | | | |
| F01 | Hub boots on a temp home, `/hello` answers | not run | not run | deferred, no VM |
| F02 | Client connects; sidebar and projects load | not run | not run | deferred, no VM |
| F03 | Threads: create, rename, pin, archive, delete, settle, snooze, order | not run | not run | deferred, no VM |
| F04 | Stand-in turn streams; restart ends running turns as interrupted | not run | not run | deferred, no VM |
| F05 | Providers via stand-ins: Claude, Codex, Grok, OpenCode, Muse, Antigravity, Pi, Oh My Pi | not run | not run | deferred, no VM |
| F06 | Runtime modes, approvals, questions, plans | not run | not run | deferred, no VM |
| F07 | Queue, steer, stop, held queue resume | not run | not run | deferred, no VM |
| F08 | Fork, bring back, provider switch | not run | not run | deferred, no VM |
| F09 | Todos, context meter, drafts survive a crash | not run | not run | deferred, no VM |
| F10 | Inboxes, orchestration tools, subthreads panel | not run | not run | deferred, no VM |
| F11 | Model picker, effort, project picker, project badges | not run | not run | deferred, no VM |
| F12 | Git: worktree threads, checkpoints, diff, revert, commit and push, PR | not run | not run | deferred, no VM |
| F13 | Titles and the text generation model | not run | not run | deferred, no VM |
| F14 | Terminal | not run | not run | deferred, no VM |
| F15 | Attachments and image paste | not run | not run | deferred, no VM |
| F16 | Keybindings | not run | not run | deferred, no VM |
| F17 | Viewers: board, schematic, 3D, STEP | not run | not run | deferred, no VM |
| F18 | Browser tab (CEF) | not run | not run | deferred, no VM |
| F19 | Bots and rooms | not run | not run | deferred, no VM |
| F20 | File browser and downloads | not run | not run | deferred, no VM |
| F21 | Reconnect, resume from sequence, outbox, long session | not run | not run | deferred, no VM |
| F22 | Ordering flow up to the quote; no order placed | not run | not run | deferred, no VM |
| | **Install and CAD** | | | |
| C01 | Package installs and starts on a clean user | not run | not run | deferred, no VM |
| C02 | KiCad: install, Settings → CAD path, kicad-cli export and DRC run from the hub | not run | not run | deferred, no VM |
| C03 | FreeCAD: install, Settings → CAD path, script run and STEP export from the hub | not run | not run | deferred, no VM |
| | **Remote operation** | | | |
| R01 | Restart the hub from another machine | not run | not run | deferred, no VM |
| R02 | Update: channel, Check now, download, checksum, swap, restart | not run | not run | deferred, no VM |
| R03 | Rollback after a bad update | not run | not run | deferred, no VM |
| | **Settings: each control persists across restart and takes effect** | | | |
| S01 | Appearance | not run | not run | deferred, no VM |
| S02 | Threads | not run | not run | deferred, no VM |
| S03 | Agents | not run | not run | deferred, no VM |
| S04 | Writing | not run | not run | deferred, no VM |
| S05 | CAD | not run | not run | deferred, no VM |
| S06 | Ordering | not run | not run | deferred, no VM |
| S07 | Devices | not run | not run | deferred, no VM |
| S08 | Network | not run | not run | deferred, no VM |
| S09 | Voice | not run | not run | deferred, no VM |
| S10 | Keys | not run | not run | deferred, no VM |
| S11 | Timezone | not run | not run | deferred, no VM |
| S12 | Usage | not run | not run | deferred, no VM |
| S13 | About | not run | not run | deferred, no VM |
| S14 | Privacy | not run | not run | deferred, no VM |

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

### macOS

| ID | Result | Commit | Rig | Evidence |
|---|---|---|---|---|

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

### Pairing and remote operation

| ID | Result | Commit | Rig | Evidence |
|---|---|---|---|---|
