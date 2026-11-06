---
name: backplane-e2e
description: Run Backplane's end-to-end harness (test/e2e) — the gate journeys, the user stories and a day-in-the-life walk — through the GPUI desktop client (and the Android/iOS adapters) against an isolated Rust hub with stand-in agents, timing every press and ranking what would hurt a person. Use for a release check, after UI changes, "run the user stories", "e2e the release" or "a day in the life".
---

# Backplane end to end

Everything lives in `test/e2e` (the harness CI and `docs/e2e/MATRIX.md` use); this file is how an agent runs it and reads it.

- `journeys.json`: the 12 **gate** journeys (`docs/final-gate.json`), the **story** journeys (`stories`, one per story in
  `stories.md`) and the **day** walk (`day-hardware`), all in one step vocabulary (`CONTRACT.md`).
- `run.mjs`: starts a hub (`lib/hubfx.mjs`), a display, the client adapter (`adapters/desktop.mjs` for GPUI, `android.mjs`, `ios.mjs`),
  runs the journeys and writes `summary.json` and `report.md`.
- Hub-level rows, no client: `f-rows.mjs` (F01-F04), `settings-walk.mjs` (S01-S14), `cad-rows.mjs` (C02, needs KiCad 10).

## Rules

- Never the person's own Backplane: not their hub (ports 3787, 3788, 3797, 57808), not their homes (`~/.backplane*`,
  `~/.local/share/backplane`), not their screen (`:0`). Every hub comes from `lib/hubfx.mjs` on a fresh root under `out/`, with its own
  HOME, XDG folders and file vault; every window opens on your own Xvfb.
- Simulated agents only: `BACKPLANE_AGENTS` holds the stand-ins (`test/tools/perf_standin.ts`, `claude_standin.ts`); the harness reads it back
  from the hub's `/proc/<pid>/environ` before any turn. Never a real model.
- Stop only what you started (the PIDs the harness records; `hub.lock` names the hub's). Never `pkill`/`killall`/`pgrep -f` a pattern.
- Memory: a hub is ~0.3 GB, a GPUI client on lavapipe ~0.5 GB. At most two runs at once on a desktop, each on its own `--display` and
  `--port`; builds one at a time (`nice -n 19`).
- Software-rendered frames are labelled so and are not GPU evidence. No journey is `pass` unless it ran on the Rust hub on real hardware
  (`--hub-kind rust --real-hardware`); otherwise a would-be pass is `partial`.
- A failure is a result: write it down with its evidence and go on. Fixes that come from what e2e found go in their own commits, titled
  `e2e: <concrete change>`.

## Build what you test

```sh
cd native/host && nice -n 19 cargo build --release -j 6 --bin backplane-hub     # or scripts/cargo-q build --release
cd native/gpui && nice -n 19 cargo build --release -j 6                         # the client; assets: BACKPLANE_GPUI_ASSETS
```

Note the commit (`git rev-parse --short HEAD`): it is the run's build.

## Run

Needs `bun`, `Xvfb` and a window manager on PATH (i3; else `twm`, or `E2E_WM=<command>`: GPUI presents no frame on a bare Xvfb), `ffmpeg` and
ImageMagick `import` for captures, and a Vulkan ICD that can present to Xvfb (lavapipe: `VK_ICD_FILENAMES=.../lvp_icd.json`). On the
person's desktop the harness refuses to start a display unless `E2E_ALLOW_DESKTOP=1` (their word, for your own Xvfb only).

```sh
export PERF_REPO=$PWD BACKPLANE_GPUI_ASSETS=<assets> VK_ICD_FILENAMES=<lvp_icd.json>
R="bun test/e2e/run.mjs --client gpui --hub-kind rust --hub-bin $HUB --gpui-bin $GPUI --display 91 --port 3914"
$R --journey all       --out out/e2e-gate                   # the 12 gate journeys (E2E_SESSION_MIN=2 shortens long-session)
$R --journey stories   --out out/e2e-stories                # every story journey, on the lived-in workspace
$R --journey story:S05 --out out/e2e-s05                    # one story
$R --journey day       --out out/e2e-day                    # the day in the life
E2E_LIVED_SCALE=0.2 $R --journey day ...                    # a smaller lived-in workspace (~1 min to seed instead of ~4)
bun test/e2e/lib/frames.test.mjs && bun test/e2e/lib/report.test.mjs && bun test/e2e/adapter.test.mjs   # the harness's own tests
bun test/e2e/f-rows.mjs $HUB out/f-rows 3915; bun test/e2e/settings-walk.mjs $HUB out/settings 3916   # hub-level rows
bun test/e2e/rig-hub.mjs $HUB out/rig 3917 6     # a seeded hub that stays up, to look at by hand; test/e2e/rpc.mjs HOME METHOD JSON pokes it
```

Android: `--client android` drives the emulator over adb (`android/driver.mjs`; `ANDROID_SERIAL`, never an attached phone). iOS: Mac only,
`ios/run.sh`; elsewhere every iOS journey is `blocked: ios_driver`.

## Read the results

`out/<dir>/<client>/report.md` first, `summary.json` for the detail.

- **Status** per journey: `pass` (every measured gate passed, on Rust + real hardware), `partial` (would pass but not on real hardware, or a
  measurement is missing), `fail`, `blocked: <capability>` (the client lacks a step; never a pass, CONTRACT.md lists the requests).
- **Steps**: every press with its result time, its ack (first frame after it) and where the time went (`hub_rtt_ms` ... `applied_to_frame_ms`:
  on lavapipe the last one is the rasteriser, the others are CPU-side and hold on any GPU). Verdict ok/slow/fail against `step_budgets_ms`
  (a press 300 ms / 2 s, a thread or project 1 s / 3 s, a viewer 5 s / 15 s, a reply 3 s / 8 s).
- **Friction, ranked by harm**: what a person would trip over, worst first (a press that never answers, over budget, a failed expectation,
  a layout jump, a dead control, slow, jank). This list is the report's point: fix from the top.
- `gate` in summary.json maps the gate journeys to `docs/final-gate.json` (pass/fail/open); story and day journeys never touch it.
- Captures (mp4, png) are listed at the end; look at the screenshot of anything that failed before believing the number.

## A day in the life

Now and then, besides the stories, walk a whole day: `--journey day` scripts one (catch up on the overnight threads, viewers and history,
start and steer and stop work in parallel, switch while things stream, review, settings, tidy up, the laptop sleeps and wakes), and its
report ranks the frictions. Then play the person yourself for the parts no script covers yet, on the same hub (`rig-hub.mjs`), and write a
narrative findings log: what you did, at what time, how long each press took, every bug or friction, ranked by how much it hurts the day.

On real data (the bugs a person reports come from real-scale use): `--copy-of <home>` runs the journeys on a copy (`BACKPLANE_COPY=1`, the
source only read, `restart.continue` off, stand-ins, nothing written into the copy's real project folders; CONTRACT.md "Copy runs"). A copy
of a person's real store also needs `E2E_COPY_REAL=person`, which only the person's own word for that run gives; `scripts/switch.sh` keeps a
separate hold on real logs.

## Before a run: catch the stories up

Features land faster than the stories change. List the commits since `test/e2e/stories.md` was last edited:

```sh
S=$(git log -1 --format=%H -- test/e2e/stories.md test/e2e/journeys.json)
git log --no-merges --format='%h %s' "$S"..HEAD -- native mobile
```

For each change a person can see: a new feature gets a step in its story (or a new story with the next free number) and its journey's
steps; changed behaviour rewrites them; a removed feature drops them. When a client gains a step a journey was blocked on, map it in the
adapter and CONTRACT.md and rerun that journey. Commit story changes on their own (`e2e: stories for <what>`).
