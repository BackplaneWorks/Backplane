# Click-to-render: how long a click takes to show its result

A person's click, key or wheel goes in through Chrome DevTools Protocol input (the same path as a real mouse), and a
recorder in the page times it until the frame that shows the result. It works on the web client in headless Chromium
and on the desktop app over its DevTools port. It only connects to a hub you started; it never starts one and has no
default home. Run it against a COPY of a history (`BACKPLANE_COPY=1`, a copied `events.jsonl`), never a live home.

```sh
# a hub on a copy (any way you like); then:
python3 test/perf/click-to-render/fixture.py COPY/events.jsonl > /tmp/fixture.json   # which threads play A/B/C/D/FAR
node test/perf/click-to-render/run.mjs --kind web --origin http://127.0.0.1:PORT --token TOKEN \
  --fixture /tmp/fixture.json --reps 8 --cpu 1 --out /tmp/c2r-before
python3 test/perf/click-to-render/table.py /tmp/c2r-before          # markdown, slowest first
# desktop app started with --remote-debugging-port=CDP --ozone-platform=headless --ozone-override-screen-size=1920,1080
#   (and its window bounds set first; under Xvfb software GL adds ~40 ms to every frame):  --kind desktop --cdp CDP
#   Start the app fresh for every run: init scripts from an earlier DevTools session stay in a running app.
# a slower machine: --cpu 4 (Chromium CPU throttling during the measured reps; setup runs unthrottled)
# a person's press: --press 80 (ms between press and release; the client may act at the press)
# the phone arrangement: --device iphone15 (touch, the PHONE list in interactions.mjs)
# a network between client and hub: node relay.mjs 3910 PORT 100 (RTT 100 ms), then --origin http://127.0.0.1:3910 --rtt 100
```

Needs `playwright-core` (`desktop/node_modules`, or `PLAYWRIGHT_CORE=path`) and Chromium (`CHROMIUM=path`, default
`/usr/bin/chromium`). Restart the hub on a fresh copy between a before and an after run: opening threads settles them,
so the sidebar changes as a run goes.

## Bigger states, slower machines, a network

- `python3 mkx.py COPY/events.jsonl BIG/events.jsonl [ID,ID]` multiplies a log x3 (ids offset, titles suffixed " ·2"/" ·3",
  interleaved in time order); the listed threads' copies fold into the originals (3x their messages and tool output).
- `node bots.mjs PORT TOKEN PROJECT 20 ids.txt` starts 20 stand-in claude threads streaming (the hub's environment needs
  the claude stand-in from test/tools/claude_standin.ts first on its agent PATH, BP_STREAM_N=900 BP_STREAM_MS=1000:
  one delta a second for 15 minutes); run it again to restart them.
- Slower machines: `--cpu 4` / `--cpu 6`, and the hub's cgroup CPUQuota=100% / 50% (systemd-run --scope -p CPUQuota=...).
- Network: `relay.mjs` delays every chunk by RTT/2 each way; `board.py RUNDIR` puts cells side by side (one row per
  interaction: p50/p95 per cell, within 16/50/100 ms, content shown, settle, round trip y/n, hub wait, where the time goes).

## What one measurement is (rec.js, installed before any page script)

1. The recorder is armed with the interaction's predicate (a JS expression over the DOM that is true when the result is
   on screen, e.g. the main view is thread X, not busy, and its timeline shows text).
2. Input: a click is the pointer arriving, resting 150 ms (a person's hover; hover prefetch is real), press, release.
3. `t0` = the first trusted input event's `timeStamp` (when the browser received it, so a busy main thread counts).
4. `dom` = the predicate first true (checked on every DOM mutation batch and every animation frame).
5. `px` = the end of the frame that holds the DOM change (host.js patches inside a rAF callback, so the pixels come
   at the end of that frame; otherwise the next one), and `paint` = a task posted from the next rAF after `dom`.
   **px - t0 is the headline number** (input to pixels, minus the compositor's few ms); paint is kept for runs
   made before px existed. Threads also report `confirm` (the hub's view replaced a cached picture), viewers and
   menus `content` (the last DOM change or canvas change inside them after they appeared).
6. `settle` = the last DOM mutation or layout shift once quiet for 300 ms (cap 6 s).
7. Also: every WebSocket send / receive (the breakdown: pre-send = client handler, hub = send -> first reply, patch =
   last reply -> DOM, frame = DOM -> pixels; "round trip" = a hub reply arrived before the result showed), layout-shift
   entries and the elements that moved, "late" content (visible text / elements added after the result's frame),
   Event Timing (handler processing), long animation frames with script attribution, and host.js's own render phases
   (`__bpPerf`, `__bpRender`).

Warm = median / p95 over reps 1..N (the interaction toggles, or an unmeasured undo returns to the start); cold = rep 0,
the first time after a page load.

## Interactions

See `interactions.mjs`: sidebar thread switches (short/long/other project), the palette jump to a far thread, project
home, thread menu, palette open and typing, Search conversations open and typing, Settings and its pages, a bot, each
workspace viewer (Files, Diff, Board, Schematic, 3D, Browser), tool-call fold, wheel up in a long thread, a composer
keystroke, send (Enter -> my bubble), Esc back, window resize.

## Content, not the frame (round 2)

Each rep also records **CONTENT** (`cnt`): the content the person came for is on screen and final, not just its
frame. A step returns `cpred` (JS: the content is there: Files rows, the Diff summary and file list, the plotted
board + history bar + Close button, the 3D model's stats, a popover's body, the thread's rows in view with a message
body, a Settings page with its controls, palette / search results) and `csig` (JS: a string of that content: text
length + head + tail, canvas signatures via `__c2r.cv(selector)`). The recorder takes the last time `cpred` became
true or `csig` changed while it was true (a stale picture the hub corrects counts at the correction), to the end of
that frame. Steps without a `cpred` use the frame predicate. `cLog` lists the content changes per rep; `late` lists
the visible text / elements added after the frame's paint (`lateTop` in summary.json: grouped over the warm reps).
`C2R_GL=rtx` runs Chromium on the machine's GPU (ANGLE GL) instead of SwiftShader, for the 3D viewer.

`python3 m2table.py RUN_DIR [MAIN_CELL]` prints frame / CONTENT per interaction and cell, ranked by content, with
the late list; `m2owners.py` splits it by owner area.

Added interactions: viewer-review, viewer-switch (Files open -> Diff), viewer-board-sch (Board <-> Schematic), fm-in /
fm-up (a Files folder in / Up), reload-fm / reload-board / reload-diff / reload-thread (reload the page, then the
click), shelf-open (a project's Settled shelf); phone: ph-viewer-fm / -diff / -schematic / -3d.

## Results

See [RESULTS.md](RESULTS.md) (before / after for the 2026-11-05 snappiness branch, per interaction, state and machine).
