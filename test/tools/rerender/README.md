# Re-render harness

What each ordinary operation makes the web client (and the desktop app, which
loads the same page) redo, and what the hub does for it. Run it before and
after a change; `--gate` keeps the counts from creeping back.

```sh
# a hub and a web client of the tree under test
scripts/build-js.sh src/server/main.bend build/rr/backplane-serve
build/rr/build-web.sh build/rr/web          # (bend src/web/index.html + the post steps)

# synthetic fixture (safe to share results)
bp-admit.sh --mem 6 --cores 4 -- bun test/tools/rerender/rerender.ts --out build/rr/synthetic

# a real thread, from a slice of the real log (results stay on this machine)
bp-admit.sh --mem 8 --cores 4 -- bun test/tools/rerender/rerender.ts --fixture real \
  --log ~/.backplane-bend/events.jsonl --long <long thread id> --tools <tool-heavy thread id> --bg <another thread id> \
  --out ~/Documents/i2cjak/bp-coord/rerender-harness/real

# code paths: a CPU profile and the forced layouts per scenario (slower; don't
# read latencies from this pass)
... --deep

# the regression gate (synthetic only)
bun test/tools/rerender/rerender.ts --gate test/tools/rerender/budgets.json
```

Options: `--scenarios open,idle,type1,burst,paste,scroll,clock,bg,send,stream,toolgroup,switch`,
`--targets long,tools`, `--port 3811`, `--chrome PATH`, `--stream-n 40 --stream-ms 150`,
`--keep` (keep the temp home), `--headed`, `--write-budgets FILE`.

## What runs

- A hub (`--bin`) on a temporary home, serving `--web`, with the stand-in
  Claude (`test/tools/claude_standin.ts`, mode `stream`: 40 deltas 150 ms
  apart, `tools:N` runs N Bash calls first). A real log runs with
  `BACKPLANE_COPY=1` and only the stand-ins.
- The fixture (`fixture.ts`). `synthetic`: 30 projects, a long thread (900
  messages with tool activity), a tool-heavy thread, a background thread.
  `real`: the real log's projects, bots, settings and every thread's
  sidebar facts, plus everything of the threads named (`--long`, `--tools`,
  `--bg`), retitled `zqlong thread`, `zqtools thread`, `zqbg thread` and
  given the stand-in. Devices, push, hooks, peers, wakes, routines, locks
  and non-background work are dropped.
- The page in a full headless Chromium (Playwright), `instrument.js`
  installed before it loads, and a second page (a driver, not measured)
  that makes the background thread stream.

## Scenarios (each per target thread)

| name | what |
|---|---|
| open | the thread opened from the palette (from another thread) |
| idle | 4 s of nothing |
| type1, type1b | one character in the composer (twice) |
| burst | 28 characters, 30 ms apart |
| paste | 20 KB inserted at once |
| scroll-up, scroll-down | 20 wheel notches up, 24 down (blank-frame and loading checks on) |
| clock | the page's 30 s clock tick (a `visibilitychange`) |
| bg-stream | another thread streams a reply (the driver sent it) |
| send | Enter, until the message shows |
| stream | the reply streaming in, to its end |
| toolgroup | a message whose reply runs 8 tool calls, then streams |
| switch-away, switch-back | another thread, and back |

## What each window records

- **DOM**: every mutation (MutationObserver), by region (composer, asks,
  workbar, timeline-row, timeline, sidebar, thread-head, viewer, status,
  page), nodes added/removed, attributes by name, *net no-op* writes (an
  attribute or text that ends as it began), nodes *recreated identical* (one
  removed and an identical one added).
- **Rows**: timeline rows touched by a mutation, really changed (their markup
  differs at the end), touched but unchanged, mounted, unmounted.
- **Hub messages**: what the page decoded, by kind (`view.changed`,
  `reply:view.update`, ...), bytes and decode ms; requests by method with
  their reply time.
- **Chromium** (CDP Performance): layouts, style recalcs, their ms, script
  and task ms. With `--deep`: the hottest functions (self time) and the
  forced layouts by the line that forced them.
- **Frames**: rAF frames, long ones (> 25 ms), dropped, Long Animation Frames
  with their scripts.
- **Input**: key → next frame (every keydown/input), Event Timing entries,
  wheel → the frame after its scroll.
- **Scroll**: frames where rows leave the timeline less than 85 % covered
  (blank), and any "Loading earlier/newer messages" shown.
- **Hub**: its CPU ms (`/proc`), RSS, and `/debug/perf` rows that moved
  (`hub.rpc`, `thin.fit.renders`, ...).
- **Page counters** (host.js and timeline-scroller.js, only when the page has
  `window.__bpRender`): `render.full`/`render.part`, `dom.patch`,
  `dom.create`, `memo.make`/`memo.hit`, `view.apply.*`, `tl.update`,
  `tl.rows.newvnode`, `tl.row.patch`/`create`/`unmount`, `tl.measure.row`,
  `tl.plan.<mode>`, `tl.resize`.

## Waste

The report ranks scenarios by a waste score: writes outside the regions the
operation is about (a keystroke: composer and asks; a scroll: the
timeline's transforms; the clock: the workbar; a background stream: the
sidebar), plus 10 × rows touched that ended unchanged, plus nodes recreated
identical, plus net no-op writes, plus anything but transforms while
scrolling. The goal is zero.

## Private data

A real-log run's report names real threads and may quote their text: write
it under `~/Documents/i2cjak/bp-coord/rerender-harness/` (or another folder
outside the repository), never into the tree. `budgets.json` and anything
committed come from the synthetic fixture only.
