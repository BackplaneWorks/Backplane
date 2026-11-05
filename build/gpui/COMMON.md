# Backplane GPUI: rules for every stream

The experiment: how much faster and smaller Backplane gets with a GPUI (Zed's Rust GPU UI) desktop app, native SwiftUI and Compose phone apps, and the Bend backend compiled to native machine code. The UI must look more or less the same as today's Backplane: layout, sidebar, thread view, composer, project home, viewers, theme tokens (src/web/style.css). Read AGENTS.md and docs/gpui/PLAN.md first.

Worktree: /home/h/Documents/i2cjak/backplane-gpui, branch `gpui`. Work there; `cd` to it if your folder differs.

## Keep the desktop quiet
The person may be gaming on this desktop. Builds and long runs go to Boat, the Mac or h@backplane. On this desktop only small, short, capped tests: 3.7 GiB, zero swap, no GPU use, nothing that competes for CPU. Starting Boat jobs from here is fine. Store-copy reads wait for the coordinator.

## Compiling
- Builds run on Boat or the Mac, never on this desktop, including Android and the Rust hub. Linux builds go to Boat; iOS goes to the Mac.
- Boat starts are rationed: 840 a day across everything, and we hit it. Keep one warm sandbox per lane: always pass `--idle 3600`, never `--no-reuse`, and batch build, tests and replay into one job. A reused warm sandbox costs no start.
- Boat, from the person (2026-11-05): reuse, don't start. One warm sandbox per lane, run repeatedly (`--idle 3600`, reuse by thread); never `--no-reuse`, never delete a warm sandbox between jobs. One job per run: check, proof, tests and build together. Prefer h@backplane or the Mac when they can do the job. A lane with no warm sandbox asks mechanics before spending a fresh start, so two lanes don't burn one on the same thing. No retry loops on a refused start.
- `scripts/gpui/bp-build.sh` has three slots of 5 GB and 5 cores on h@backplane. Don't pass `lock: bp-build` to `work_run`. Run focused `cargo test <module>`, not the whole crate. Put multi-line commands in a script.
- `boat-job.sh --include-file` is a whitelist, not an addition: with any given, ONLY those files upload. Untracked, unignored files (`stage/`) go up by default, so pass none for a whole-tree run, and check the `source:` line's file count before reading a result.
- h@backplane is an extra build machine for finding failures early: `scripts/gpui/bp-build.sh LANE 'cmd'` through work_run with lock `bp-build`, one at a time, 12 GB, -j8. Results are provisional: a gating result must run once on Boat. Stop at the first ICE, SIGILL or segfault and tell the coordinator.
- Release a Boat sandbox when a lane's batch is done; don't leave `--idle 3600` stacked up.
- Boat: `scripts/boat-job.sh`, 8 GiB big jobs, 3.7 GiB otherwise, swap 0.
- Running a finished binary or the hub here is fine.
- Mac access: `build/gpui/mac-ssh hs-mac-mini '<cmd>'`, and `RSYNC_RSH=$PWD/build/gpui/mac-ssh rsync ...`. Plain ssh fails.
- The Mac: Apple silicon, 10 cores, 16 GiB, macOS 26.5, Xcode 26.6, iOS 26.5 runtime, Tailscale 100.97.97.92, console session logged in. `xcode-select` points at the command line tools: do not change it, `export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer`. `xcrun simctl` is not on PATH over plain ssh without that.
- The Mac is shared with release threads. Use only your own folder `~/bp-gpui-<stream>` and your own simulator or AVD. You may read other `~/backplane-*` folders but never change them.
- Heavy jobs on the Mac take the named lock `mac-heavy`: start them with `work_run` and `lock: "mac-heavy"`, or `lock_acquire`, end your turn, `lock_release` after. Never poll or sleep-wait. Release the lock promptly.
- Disk: run `df -h /System/Volumes/Data` before a heavy step. Under 20 GiB free, stop and tell the coordinator. Only the coordinator clears space. Do not delete anything outside your own folder.

## Hubs and data
- Shared isolated hub: http://desktop.tail9e0cf2.ts.net:3799/ (home ~/.backplane-gpui, user unit backplane-gpui-hub, binary build/gpui/backplane-serve). Token: ~/.backplane-gpui/token here, ~/bp-gpui-secrets/hub-token on the Mac. Never print or commit it. Do not restart that hub without asking the coordinator.
- NEVER touch ~/.backplane-bend, port 3787 or 3788, ~/.local/share/backplane, or the person's real log. The one store copy is ~/.backplane-gpui-copy, made by the coordinator, read-only and restored to its verified state (sha256 44daa6a7...545da). Nothing runs on it directly: any hub that writes, baselines included, runs on a throwaway copy of it under ~/.backplane-gpui-scratch/ (mode 700, outside the repo, so no sync or build upload can reach it), deleted afterwards. Never put real store data under build/. It holds real conversations, so it stays on this machine, never goes to Boat or git, and never appears in an image. Run any hub on such a throwaway copy with BACKPLANE_COPY=1 and simulated agents. No Rust hub on real store data until its copy guard lands.
- HOLD: no lane starts a Rust hub on a copy of the real log, not even a throwaway one. Only Bub lifts this, in the coordinator's thread, after all of these are met and the evidence sent: (1) files' audit landed and the hub under test built from it; (2) the reviewer's final recorded OK on `SIDE-BY-SIDE.md`, given after every row in its actor table is held by code and test (the rev 3 design OK does not count); (3) the bots gates pass: ticker, `uber_init`, mail recovery; (4) the state ticker check passes; (5) `restart.continue` off, history no-ops on a copy, and no watched work re-armed on a copy, proven by the agents child-process test. Measure on the synthetic real-size log in `~/.backplane-gpui-scratch/` until then. `BACKPLANE_COPY=1` alone doesn't make such a run safe.
- A hub of your own: a temp home under build/gpui/, your assigned port, `--no-tailscale --foreground`, `BACKPLANE_AGENTS=<stand-in folder>`, `BACKPLANE_NO_UPDATE=1`, in its own `systemd-run --user --scope -p MemoryMax=3700M -p MemorySwapMax=0`. Stand-in agents only: no model calls in tests.
- Load tests and soaks use simulated agents only. Every hub you start must be checked before its first turn: read BACKPLANE_AGENTS from /proc/<pid>/environ of the hub itself, and after a turn confirm the log's TurnUsage has no real model. A prefix assignment on a line that continues into `cd` does not reach the hub: this ran a real Claude turn once.
- Nothing heavy runs on this desktop. Every process here runs under the 3.7 GiB cap, with zero swap. Scale, fold and out-of-memory tests run on Boat under a hard cap, on synthetic logs only, because the store copy never leaves this machine. The person has approved test runs on the store copy on this desktop, and small seeded GPUI tests, and the Electron vs GPUI comparison, each capped at 3.7 GiB with zero swap, simulated agents only, never real credentials. If a run needs more, stop and tell the coordinator.
- Kill only PIDs you captured, never by name or pattern. Any window on the desktop goes on an isolated display, never the person's screen.

## Git and publishing
- Every lane works in its own worktree on its own branch: `git worktree add ~/Documents/i2cjak/bp-gpui-lanes/<lane> -b gpui-<lane> gpui`. The main checkout `backplane-gpui` is not a place to edit or commit. Commit only your own paths. Pick up others' work with `git rebase gpui`.
- `gpui` is guarded: a hook refuses every commit, cherry-pick or ref move onto it, in any worktree. It moves only by `scripts/gpui/land.sh <tip>`, run from your worktree with your branch rebased on `gpui`: that takes the `gpui-merge` lock, checks the reviewer recorded an OK (`scripts/gpui/review-ok.sh`) for every commit in `gpui..<tip>`, and fast-forwards. No exceptions: test harness, docs and regenerated files go through review too. Asking for a review is not an OK. A rebase that changes a commit's diff needs a new OK.
- Don't commit `docs/gpui/PARITY.md` in a lane range: edit rows in `scripts/gpui/parity-features.py` only. A regenerated `PARITY.md` changes on every rebase and voids the commit's recorded OK. Mechanics regenerates it in its own small reviewed range at milestones.
- Never commit binaries, tarballs or generated output: nothing over 1 MB. Boat picks up untracked, unignored files, so leave them untracked, or list them with `--include-file`.
- Do not push. The coordinator pushes.
- No TestFlight or Play upload, no signed release build, no merge to main, no install over the real app, no `bend update`, no release tag.
- Edits to `src/**.bend` affect the law checks: do not change them without asking the coordinator.

## Cost
Implementation lanes run on Sonnet. Mechanical work (test reruns, parity diff counts, screenshots, status relays, file moves) goes to the Haiku `mechanics` thread. Do not spawn sub-tasks on stronger models. One reviewer reads each lane's commit batch before it lands.

## Reporting
- Raw numbers in build/gpui/results/<stream>/, PNG images in build/gpui/shots/<stream>/.
- A measurement states its machine, commit, sample count and median/p95. Anything not measured is called unmeasured. A failure is reported with its output, never smoothed over. A smoke test is not called a load test.
- Report at milestones only, in a few lines. No progress narration, no receipts, no hashes unless asked.
- Keep working through your milestones without stopping to ask. End your turn when you finish, or when you truly need a decision. Your last message says briefly: what works, real numbers, image paths, what is left or blocked.
