# Backplane GPUI: rules for every stream

The experiment: how much faster and smaller Backplane gets with a GPUI (Zed's Rust GPU UI) desktop app, native SwiftUI and Compose phone apps, and the Bend backend compiled to native machine code. The UI must look more or less the same as today's Backplane: layout, sidebar, thread view, composer, project home, viewers, theme tokens (src/web/style.css). Read AGENTS.md and docs/gpui/PLAN.md first.

Worktree: /home/h/Documents/i2cjak/backplane-gpui, branch `gpui`. Work there; `cd` to it if your folder differs.

## Compiling
- Builds run on Boat or the Mac, never on this desktop, including Android and the Rust hub. Linux builds go to Boat; iOS goes to the Mac.
- Boat: `scripts/boat-job.sh`, 8 GiB big jobs, 3.7 GiB otherwise, swap 0.
- Running a finished binary or the hub here is fine.
- Mac access: `build/gpui/mac-ssh hs-mac-mini '<cmd>'`, and `RSYNC_RSH=$PWD/build/gpui/mac-ssh rsync ...`. Plain ssh fails.
- The Mac: Apple silicon, 10 cores, 16 GiB, macOS 26.5, Xcode 26.6, iOS 26.5 runtime, Tailscale 100.97.97.92, console session logged in. `xcode-select` points at the command line tools: do not change it, `export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer`. `xcrun simctl` is not on PATH over plain ssh without that.
- The Mac is shared with release threads. Use only your own folder `~/bp-gpui-<stream>` and your own simulator or AVD. You may read other `~/backplane-*` folders but never change them.
- Heavy jobs on the Mac take the named lock `mac-heavy`: start them with `work_run` and `lock: "mac-heavy"`, or `lock_acquire`, end your turn, `lock_release` after. Never poll or sleep-wait. Release the lock promptly.
- Disk: run `df -h /System/Volumes/Data` before a heavy step. Under 20 GiB free, stop and tell the coordinator. Only the coordinator clears space. Do not delete anything outside your own folder.

## Hubs and data
- Shared isolated hub: http://desktop.tail9e0cf2.ts.net:3799/ (home ~/.backplane-gpui, user unit backplane-gpui-hub, binary build/gpui/backplane-serve). Token: ~/.backplane-gpui/token here, ~/bp-gpui-secrets/hub-token on the Mac. Never print or commit it. Do not restart that hub without asking the coordinator.
- NEVER touch ~/.backplane-bend, port 3787 or 3788, ~/.local/share/backplane, or the person's real log. The one store copy is ~/.backplane-gpui-copy, made by the coordinator: make no others. It holds real conversations, so it stays on this machine, never goes to Boat or git, and never appears in an image. Run any hub on it with BACKPLANE_COPY=1 and simulated agents.
- A hub of your own: a temp home under build/gpui/, your assigned port, `--no-tailscale --foreground`, `BACKPLANE_AGENTS=<stand-in folder>`, `BACKPLANE_NO_UPDATE=1`, in its own `systemd-run --user --scope -p MemoryMax=3700M -p MemorySwapMax=0`. Stand-in agents only: no model calls in tests.
- Load tests and soaks use simulated agents only. Every hub you start must be checked before its first turn: read BACKPLANE_AGENTS from /proc/<pid>/environ of the hub itself, and after a turn confirm the log's TurnUsage has no real model. A prefix assignment on a line that continues into `cd` does not reach the hub: this ran a real Claude turn once.
- Nothing heavy runs on this desktop. Every process here runs under the 3.7 GiB cap, with zero swap. Scale, fold and out-of-memory tests run on Boat under a hard cap, on synthetic logs only, because the store copy never leaves this machine. The person has approved test runs on the store copy on this desktop, and small seeded GPUI tests, and the Electron vs GPUI comparison, each capped at 3.7 GiB with zero swap, simulated agents only, never real credentials. If a run needs more, stop and tell the coordinator.
- Kill only PIDs you captured, never by name or pattern. Any window on the desktop goes on an isolated display, never the person's screen.

## Git and publishing
- All streams share this worktree and branch. Commit only your own paths: `git commit -m "..." -- <paths>` (add new files with `git add <paths>` first). Retry if the index is locked. No stash, rebase, reset, or checkout of files you do not own.
- Do not push. The coordinator pushes.
- No TestFlight or Play upload, no signed release build, no merge to main, no install over the real app, no `bend update`, no release tag.
- Edits to `src/**.bend` affect the law checks: do not change them without asking the coordinator.

## Reporting
- Raw numbers in build/gpui/results/<stream>/, PNG images in build/gpui/shots/<stream>/.
- A measurement states its machine, commit, sample count and median/p95. Anything not measured is called unmeasured. A failure is reported with its output, never smoothed over. A smoke test is not called a load test.
- Keep working through your milestones without stopping to ask. End your turn when you finish, or when you truly need a decision. Your last message says briefly: what works, real numbers, image paths, what is left or blocked.
