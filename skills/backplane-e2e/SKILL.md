---
name: backplane-e2e
description: Run Backplane's end-to-end user stories (stories.md) through the real UI — web page, desktop app and Android app — against an isolated test hub with stand-in agents, timing every press and reporting failures. Use for a release check or after UI changes ("run the user stories", "e2e the release").
---

# Backplane user stories, end to end

`stories.md` lists what a person does with Backplane (32 short stories). Run
them through the real UI, the way a person would: clicks, keys, swipes. For
each step, check that the right thing happened and time how long it took. You
end up with one JSON result per story and run, plus a report with the totals
and every failure.

A run, in order:

1. Catch the stories up with what changed since they were last edited (below).
2. Build what you test: `scripts/build.sh`, or take a release's
   `backplane-serve`. Note its `/hello` version: that is the run's `build`.
3. Run one agent per story, with the hubs from `hub.sh` ("Running them all").
4. Write the report, and stop everything you started.

## Before you start: catch the stories up

Features land faster than the stories change. Before a run, list the commits
since `stories.md` was last edited:

```sh
S=$(git log -1 --format=%H -- skills/backplane-e2e/stories.md)
git log --no-merges --format='%h %s' "$S"..HEAD
```

Read the titles, and open a commit (`git show --stat <hash>`, then the diff)
when the title doesn't tell you what a person would see. For each change a
person can see (a button, view, setting, shortcut, gesture, or what one of
them now does):

- a new feature: add a step to the story it belongs to, or write a new story
  (the next free number, with its platforms) if none fits;
- changed behaviour: rewrite the steps that describe the old behaviour;
- a removed feature: drop its steps.

Skip what nobody sees: tests, refactors, CI, docs, and fixes inside the hub.
Keep each story as brief as the others. Commit `stories.md` on its own
(`backplane-e2e: stories for <what>`) before the run, so the next run starts
counting from there. If nothing visible changed, say so and go on.

## Rules

- Never touch the person's own Backplane: `~/.backplane-bend`, port 3787, or
  their open window. Every hub here comes from `hub.sh`, which uses its own
  home, port, fixture projects and stand-in agents. It runs with no tailnet, no
  self-update and no keyring.
- Stop only what you started: `hub.sh stop NAME`, plus the PIDs you captured
  for browsers, the desktop app and the emulator. Never `pkill -f` a pattern.
- Real models are never run: `BACKPLANE_AGENTS` holds only stand-ins
  (`test/tools/claude_standin.ts`). Other providers fail on purpose.
- Keep memory in check: a browser is about 0.5 GB and a hub about 0.3 GB.
  Run at most 3-4 stories at once. There is one emulator (3 GB), and phone
  stories take turns on it.
- A failure is a result, not something to fix here. Write it down with a
  screenshot and go on to the next step if you can.

## Setup

Needs `node` (22+), `bun`, `python3`, `curl`, Chromium, and
`playwright-core` (`tools/browser/node_modules` after `bun install` there).
Phone stories also need an Android emulator with `adb` and the APK under test.

```sh
export BP_E2E_BIN=/path/to/backplane-serve      # the build under test (web/ beside it)
export BP_E2E_FIXTURE=/usr/share/kicad/demos/ecc83   # any KiCad project folder
skills/backplane-e2e/hub.sh start S04-W         # prints PORT= TOKEN= URL= PAIR= ... PID=
skills/backplane-e2e/hub.sh long S04-W          # only for S07: an 80-exchange thread, ~4 min
skills/backplane-e2e/hub.sh trial try           # a workspace that looks used, ~3 min (below)
skills/backplane-e2e/hub.sh restart S31-W       # S31
skills/backplane-e2e/hub.sh stop S04-W
```

Give every story its own hub (the name is the story and platform), so that
stories never see each other's threads.

`hub.sh trial NAME` fills a hub (starting it if needed) with a workspace
that looks used: five projects (`proj`, `multi`, `amp`, `sensor-node`,
`psu`), about twenty threads of 1-5 exchanges, three long ones (24-36), and
replies that start with tool calls. Some threads are pinned, settled,
snoozed or archived; `$BP_E2E_ROOT/NAME/trial` lists them. Use it to try
things by hand (open its `URL`), or for a story that needs a lived-in
sidebar. The seeding is in `trial.mjs`.

## Driving each platform

**W, web.** Use playwright-core with Chromium, headless, at 1400x900:
`page.goto(URL)`.

**D, desktop app.** Run the packaged app against the story's hub:
`env -u ELECTRON_RUN_AS_NODE BACKPLANE_HOME=$HOME_DIR <app>/desktop/runtime/backplane-desktop --remote-debugging-port=<free port>`.
Attach with `chromium.connectOverCDP(...)` and take the page whose URL is the
hub's. A desktop app older or newer than the hub stops at "Service version
differs": add `BACKPLANE_SERVICE_VERSION=<the hub's /hello version>` to its env. Use a display: Xvfb on Linux, `--ozone-platform=x11`. D differs from W
in its real window: smooth scrolling, its own startup and cache. Scroll it
with `mouse.wheel` in 100 px notches.

**A, Android.** On the emulator, `adb install -r <apk>`, then
`adb shell am start -a android.intent.action.VIEW -d '<PAIR>'` (the emulator
reaches the host as 10.0.2.2). To find elements, use
`adb shell uiautomator dump`, then read the node bounds. Act with
`adb shell input tap/swipe/text/keyevent`, and take screenshots with
`adb exec-out screencap -p`. Grant notifications first
(`pm grant dev.backplane.mobile android.permission.POST_NOTIFICATIONS`), so
the permission dialog doesn't cover the screen.

**I, iOS.** Only on a Mac with the simulator. Otherwise mark it `skip`.

Handy hooks on the web and desktop:
- The tour has a Skip button.
- A thread in a fold opens once "Settled N", "Older N" or "Conversation history" is opened.
- The composer is `#composer`, and the timeline is `#timeline`, with entries as `[data-id]`.
- Viewer tabs are buttons named Board, Schematic and 3D.
- The board picker is `details.board-selector`.
- The stand-in streams numbered lines: reply text ends with the last word of your message, followed by a number.

## Timing

For every step, note the time just before the action. Then poll until what
the story expects is on screen: `page.waitForFunction` on web and desktop, a
`uiautomator dump` loop on Android, where timings are coarse (~300 ms) and
should be noted as such. Record the milliseconds.

Budgets, unless the story says otherwise:

| what | ok | slow | fail |
|---|---|---|---|
| a press shows its effect (button state, menu, toggle) | ≤ 300 ms | ≤ 2 s | > 2 s |
| a navigation (thread, tab, settings page) | ≤ 1 s | ≤ 3 s | > 3 s |
| a viewer draws (board, schematic, 3D) | ≤ 5 s | ≤ 15 s | > 15 s |
| the stand-in's reply starts after send | ≤ 3 s | ≤ 8 s | > 8 s |

A step is **fail** when what it expects never shows, something else shows
(an error, a wrong view, a jump), or it passes the fail budget. It is **slow**
past "ok". A story fails if any of its steps fails.

## Results

Write `skills/backplane-e2e/results/<run>/<story>-<platform>.json`. The
results folder is not committed. Each file looks like this:

```json
{"story": "S07", "platform": "D", "result": "pass|fail|skip", "build": "<hub /hello version>",
 "steps": [{"step": "scroll to the first message", "ms": 8400, "verdict": "ok|slow|fail", "note": ""}],
 "failures": [{"step": "...", "expected": "...", "got": "...", "screenshot": "results/<run>/S07-D-3.png"}]}
```

## Running them all

Run one agent per story (or per story and platform). Give each agent this
file, its story's text, the build under test, its hub name and its
platforms. Run 3-4 at once, and the phone stories one after another on the
single emulator. When all are done, write
`skills/backplane-e2e/results/<run>/report.md`:

- a table of story by platform, with each result and the slowest step in ms;
- the total failures and slow steps;
- each failure with its step, what was expected, what was seen, and its
  screenshot.

Last, stop every hub, browser, desktop app and emulator you started.
