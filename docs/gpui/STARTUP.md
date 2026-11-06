# Start-up: checkpoints, not folds

The hub used to read the whole log several times at start (the line index, the read model, the policies' state, the agents' and bots' worlds, the wake book, the command fold, the far feed and the peer table). On the 1.69M-event copy that was about 7 s to `/hello` and about 410 MB idle. Each of those is a deterministic function of the first N lines, so each is now saved with N and a fingerprint of the log at N, and a start folds only the lines past N.

## What is saved (`<home>/ckpt/`)

| file | holds | written by |
|---|---|---|
| `index.bin` | offset and length of every line (`Store`) | `Store::save_index` |
| `model.bin` | the read model: entry indexes, threads, projects, settings (`Model`) | `Model::save` |
| `state.json` | the policies' lean fold (`State`), which the native sidebar draws from | `statehost::refresh_state` |
| `agents.json`, `bots.json`, `commands.json`, `wakes.json`, `feed.json`, `peers.json` | the other folds | each module's `save`, registered with `ckpt::register` |

A checkpoint is used only when its version matches, it covers no more lines than the log has, and the fingerprint of the log at its line count (the count, the first line, the line it ends on; for the index, hashes of the first and last 4 KB it covers) is what it was when saved. Anything else is ignored and the fold runs over the whole log as before. Files are written beside and renamed. Delete `ckpt/` any time; the next start folds everything and the files come back.

## When they are written

`ckptrun::start`: 10 s after start, then every 2 minutes, a fold is written again once it is 5,000 lines (or more) past its file. The policies' state is brought forward from its own file by `fold_line`, so it is exactly what a fold of the log gives, never a copy of the live state. A start that had to fold 20,000 lines or more of state saves it at once.

## What a start does

1. `Store::open`: load `index.bin`, scan only the bytes past it.
2. `Model::open`: load `model.bin`, fold the lines past it.
3. In parallel from the moment the store is open (`ckpt::preload`): the agents' world, the bots' world, the policies' state.
4. The rest registers and the listener binds. Nothing waits on a fold of history.

## Proof and measurement

- `BACKPLANE_CKPT_CHECK=1`: after each checkpoint loads (and its tail is folded), the whole log is folded the old way too and the two are compared; the log says `ckpt-check <name>: equal` or names what differs. Run on a truncated copy so the tail is 391,000 lines: model, policy state, agents world, bots world, command fold and wake book all equal.
- Every start prints `startup: +<ms> <phase> (rss <MB>)` lines.
- Tests: `store::tests::a_saved_index_*`, `model::tests::a_saved_model_loads_and_takes_the_tail`.

## Numbers (1.69M-event copy of the real log, 3.7 GiB cap)

See the report that came with this change; run `scripts/gpui/startup-bench.sh HUB_BIN HOME` for your own.

## Timing the window

`CLIENT_BIN=.../backplane-gpui scripts/gpui/startup-bench.sh HUB HOME` also opens the window on a private Xvfb display and prints the client's own `first_frame` and `usable` metrics (ms from its process start) and checks a screenshot is not blank. The old "blank window" was a missing window manager: GPUI on X11 gets no frame callback without one (no `first_frame`, nothing painted). The harness starts an empty-config `i3`. Lavapipe (`llvmpipe`, `VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json`), scale factor 1 and the bundled JetBrains Mono load correctly (`GPUI_LOG font loaded`).

## Actions (click to first paint) and this device's settings

Measured on the throwaway copy with `BACKPLANE_GPUI_SCRIPT=test/gpui/perf/actions-switch.json` (20 threads opened, then again) and `python3 test/gpui/perf/actions-stats.py` over the client's log, software Vulkan (each frame on llvmpipe is 16-32 ms, so these are upper bounds):

| action | before | after |
|---|---|---|
| open a thread, first time (hub round trip, reply 50-135 KB) | 18 of 20 threads refused ("not ported: row.malformed-or-foreign"); the 2 that opened: 86-128 ms | all 20 open; p50 96 ms, p95 128 ms |
| back to a thread seen before | p50 96 ms, p95 112-128 ms (the cache held 12 threads, and a thread was cached only when left) | p50 32 ms, p95 32 ms (32 threads kept) |
| send: message on screen | 212 ms (waits for the hub's picture) | 20 ms (drawn at once; the hub's picture replaces it, never doubles it) |
| fold/unfold sidebar, Files/Board tab | 16-43 ms | same |
| new thread, pin (hub actions) | 68-83 ms | same |
| older page after scrolling to the top | first rows in about 60 ms, next pages every 50-100 ms | same |

The 18 refusals were `ents()` returning "foreign" for a thread that had sent messages into other threads (most threads on a real log). Such rows are now left out of that thread's page instead of refusing the screen.

Not done: prefetch on hover (the protocol has one open view per connection, so a prefetch would move the hub's shown view; it needs a second connection), and the hub's project page, bots, rooms, settings pages and the `/` popup, which the hub still answers "not ported" (they are the presenter lanes').

### This device (`native/gpui/src/local.rs`)

Per-device settings live in `$XDG_CONFIG_HOME/backplane-gpui/local-settings.json` (`BACKPLANE_GPUI_LOCAL` overrides): theme (follow the hub, dark, light), text size, font, sidebar width, sidebar folded or open, default view (where I left off, or the thread list), notifications (kept; the client draws no alerts yet), and keyboard shortcuts (send, close a popup, fold the sidebar, search). They are read at start, applied at once and written when they change; `local.rs` builds no hub command (a test reads its source for that), so they never round-trip. The gear opens the dialog with "THIS DEVICE" first, marked "Kept on this computer only", and the hub's pages under "ALL DEVICES (HUB)", marked "Stored in the hub: the same on all your devices". With no hub pages, the device page opens alone. The two-pane layout (nav with icons and search, rows on the right) is the existing one.
