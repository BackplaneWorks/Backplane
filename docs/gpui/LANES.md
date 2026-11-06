# Rust lanes

`main` is the integration branch: the Rust hub (`native/host`) and the GPUI client (`native/gpui`), no Bend. Every lane branches from `main` and lands on `main` through `scripts/gpui/land.sh` with the reviewer's recorded OK. (Until 2026-11-06 this branch was called `gpui`; the last Bend `main` is the tag `main-bend-final`. The shared lock and state names `gpui-merge`, `gpui-reviewed`, `gpui-reviewer`, `gpui-land.token` keep their old names so existing enrolment and locks carry over.)

One owner per file. New lanes work in their own git worktree, so an unfinished file in one
lane cannot break another lane's build.

| Lane | Thread | Owns |
|---|---|---|
| core (lead) | `bend` | `native/host/src/{main,hub,model,store,auth,cbor,presenter,api}.rs`, copy guard, the module split |
| sidebar | `slim` | `sidebar.rs`, `shelf.rs`, `signal.rs`, `state.rs`, `test/gpui/parity`,  |
| files | `files` | `native/host/src/files/`, `native/host/src/kicad/` |
| agents | `agents` | `native/host/src/agents/` except `providers/`, MCP tools |
| parity-session | new | `native/host/src/session/` |
| parity-edge | new | `native/host/src/wire/` |
| prov-acp | new | `native/host/src/agents/providers/{acp,grok,opencode}.rs` |
| prov-msp | new | `native/host/src/agents/providers/{msp,muse,agy}.rs` |
| prov-pi | new | `native/host/src/agents/providers/{pi,omp,dialog}.rs` |
| state | new | `native/host/src/state/` |
| soak | new | `test/gpui/soak/` |
| commands | new | `native/host/src/commands/`: thread, project and queue requests |
| settings | new | `native/host/src/settings/`: settings, accounts, telemetry, diagnostics, vault |
| bots | new | `native/host/src/bots/`: bots, rooms, routines, mail, uber ops, bot tools |
| net | new | `native/host/src/net/`: pairing, QR, devices, push, far threads, peers |
| term | new | `native/host/src/term/`: pty, terminals, browser helper, `preview_*`, teach, tasks |
| gpui-screens | new | `native/gpui/src/screens/`: settings, asks, images, multi-line composer, project home, viewers |

## How a lane works

- Worktree: `git worktree add ~/Documents/i2cjak/bp-gpui-lanes/<lane> -b gpui-<lane> main`.
  Copy nothing real into it. Binaries in the main worktree's `build/gpui/` can be used by
  absolute path.
- Build and test from that worktree with `scripts/boat-job.sh`. Boat or the Mac only.
- Pick up others' work with `git rebase main` in your own worktree.
- Before landing, rebase on main and compile the whole crate on Boat. A batch that breaks main's build is backed out.
- Land small and often, from your own worktree: rebase on main, then `scripts/gpui/land.sh <tip>`.
  It takes the `gpui-merge` lock, checks the reviewer's recorded OK for every commit in `main..<tip>`
  and fast-forwards. `scripts/gpui/ref-guard.hook` refuses any other move of `main`, so commits,
  cherry-picks and ref updates onto it fail in every worktree. Never force anything.
- To change a file you do not own, ask its owner by `thread_send`.
- A shared seam (one line in `main.rs`, one `mod` line) goes through the lead.
- Before landing a batch, send the `reviewer` thread its commit range. The reviewer records the OK with `scripts/gpui/review-ok.sh BASE..TIP`; only that session can. Land small batches, not every commit.

## The ready push (2026-11-05)

Ready means: (1) the Rust hub with no Bend helper opens a throwaway copy of the person's real data; (2) GPUI desktop shows the threads; (3) a real Claude turn works; (4) Claude launches and drives a thread; (5) Improvements hands off to a Claude thread. Plus the side-by-side basics: own port and home, no auto-resume or routines, history recording off, real folders only for turns the person starts. Deferred until the person is using it: the 10M-event and 1 h memory tests, parity counts, the gallery, broad review batches, phones, extra protocol scenarios. Each lane works in its own worktree under `~/Documents/i2cjak/bp-gpui-lanes/` and lands only through `scripts/gpui/land.sh`. The reviewer reads the copy guard and side-by-side; it records other ranges on the lane's test result.

Memory bar: `docs/gpui/perf-gates.json`: hub RSS only, startup peak under 1 GB, at rest under 500 MB.

| Lane | Thread | Owns | Worktree |
|---|---|---|---|
| 1. Thread view and pages | slim `t1793834628-1688992` | new `native/host/src/present/` thread view and pages, Files and Mech replies | `slim` |
| 2. Composer, send, turn state | parity-session `t1793852397-1739751` | `present/` composer, send, popups; the replay oracle | `parity-session` |
| 2b. Thread actions | commands `t1793862825-1750573` | `native/host/src/commands/`: create, pin, settle, rename, archive as served by the Rust presenter | its own |
| 3. Bend off the runtime (done) | bend `t1793832639-1684539` | indexed read model, `hub.rs`, `main.rs`, `--policy-bin` removal (done 2026-11-06, with all Bend source) | `endstrip` |
| 4. Real Claude turns | agents `t1793842041-1700235` | `native/host/src/agents/`: full prompt, skills, images, `Kind::PersonTurn` | `agents` |
| 5. Core MCP tools | mcp-core | `agents/mcp*` tool handlers for threads, subthreads and reports, `work_*`, `wake_on`, files, memory | `mcp-core` |
| 6. Side-by-side and switch script | switchover `t1793897533-1897003` | `scripts/gpui/switch.sh`, `side-by-side.sh`, `SIDE-BY-SIDE.md` | `switchover-dev` |
| 7. Real-log open test | perf `t1793832632-1684495` | the test harness; runs on synthetic until the coordinator lifts the hold | `perf` |
| 8. Improvements | improvements `t1793897721-1900263` | `native/gpui` Improvements panel, recording, `settings/improvements.rs`, `issue_report.rs`, the hand-off | `improvements` |

