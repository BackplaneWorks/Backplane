# Presenter port: the Bend helper leaves the runtime path

Decision (the person, 2026-11-05): no Bend in the running hub. `bp-policy-pager` (the policy) and `window` (the window helper) go;
what the hub needs moves to Rust. Start with exactly what opens the real log and serves GPUI desktop: sidebar (Rust already,
0 differences), thread view and pages, composer, sending, turns, thread create, settle, rename, archive. State is windowed and
indexed, never the whole log in memory. Bar: **same answers as today**, checked by replay against JS recordings. Measure on
`~/.backplane-gpui-scratch/synthetic-real-size.jsonl` (1,691,620 events, 1.14 GB) only, never a store copy. `gpui-compact` is stopped (branch kept).

## Decision needed from the person

Your bar was memory **flat** at two log sizes. This port cannot meet that word: the log index and the read model grow with events (the store keeps an offset per event, `Model` an entry per
message). Measured so far about 38 B/event all in (38 MB at 1M events; 18.7 B of RSS per committed event), so about 64 MB at 1.69M and 380 MB at 10M. What the port does give: no helper and no whole-log fold
(the 3.6 GB at 1.69M events is gone), `meta` bounded by threads/projects/tasks, and a measured slope. Proposed replacement bar: slope at most 64 B/event, measured at 0.85M and 1.69M synthetic events, and RSS
steady while no events arrive. Say if you want a different number, or flat as originally written (which would need the index on disk, a larger design).

## What the helper does now

Two Bend child processes, each a line protocol the hub drives (`presenter.rs`):

| Piece | Does |
|---|---|
| policy `F` (fold) | applies the log (`changes`) to a `U.Ui` (client.bend, 17.7k lines) + `M.State`; this is what grew to 3.6 GB on 1.69M events |
| policy `V` (view) | per request: `MS.Session` over that `Ui`, with the open thread's page rows, `now`, identity, `local` memory, reads. Returns `shown` (the mobile screen model, about 40 keys: `thread` with entries/blocks/stamps/images, `projects`, `sel`, `search`, `tour`, `settings`, `bots`, `rooms` ...), `session` (what the client sends back), `cmds` (RPCs the client runs), `nextTarget`, `overlay {j,key,name}`, `window` |
| policy `P` | ping |
| `window.bend` | the page's `window`: ids, messageIds, historyUnits, contextMessages, anchor, older/olderNext, hasMore, remaining, newer, total (`FO.Fold`, `MC.Context`, `LT.Tools` laws) |

Already Rust: the log store and offset index (`store.rs`), the entry indexes and pages (`model.rs`, equal to `store.db` on 100 pages),
sidebar rows/status/order (`sidebar.rs`, `signal.rs`, `shelf.rs`), stamps (`stamp.rs`), markdown/links/refs (`md/`), keyed deltas
(`delta.rs`), the view actor with per-view memory (`session/`), the state policies (`state/`).

## What replaces each piece

| Today | Rust | Owner |
|---|---|---|
| fold into `U.Ui`/`M.State` | **metadata projection** `present/meta.rs`: threads, projects, settings, asks, queues, bots, rooms, usage; updated per event, size bound by those objects, not by events; per-thread counters/preview/status kept at apply | bend |
| `Pol.view` dispatch, `Pol.target/after`, STALE_VIEW, `local`, reads | `present/mod.rs`: the router and the `Presenter` impl `RustPresenter` | bend |
| `shown.thread`: entries, fold of tool runs, blocks, images, live row, header | `present/thread.rs` | slim |
| `window` + `view.page` | `present/window.rs`, `present/pages.rs` over `model.rs` | slim |
| composer, drafts, slash and `$ @ > % #` popups, send command | `present/composer.rs`, `popup.rs` | parity-session |
| actions: select, thread create, settle, rename, archive (pin, snooze, delete follow) | `present/actions.rs`: `(meta, session, action, value) -> (session patch, cmds, nextTarget)` | commands (proposal) |
| `shown` keys GPUI does not read | emitted from a static table keyed by the same state, byte-equal under replay | slim lists them |
| replay oracle | `test/gpui/parity` (`record.mjs`, `compare.mjs`, `oracle/`) against JS recordings, scenario `session:gpui` | parity-session |

## Module layout (`native/host/src/present/`)

```
mod.rs        RustPresenter: impl Presenter; Router (rust | helper | not-ported error); flag handling
meta.rs       metadata projection (the only per-thread/project state), built from events
screen.rs     the shown root: sel, view, projects, search, tour... assembled from meta + parts
thread.rs     entries, blocks, fold, stamps, images, live row         (slim)
window.rs     window context (messageIds, historyUnits, anchor, cursors, remaining/newer/total)   (slim)
pages.rs      view.page cursors over model.rs                          (slim)
composer.rs   drafts, send command                                     (parity-session)
popup.rs      slash and mention rows                                   (parity-session)
actions.rs    the action table                                         (commands)
```
Pure functions: `fn view(cx: &Ctx, req: &Req) -> Out`. `Ctx` is read-only (`&Model`, `&Meta`, a page reader); `Req` holds target,
session, action, now, ids; `Out` holds `shown, session, cmds, next_target, overlay, window, wants`. The view actor (`session/`, parity-session) keeps
per-connection state (drafts, markers, held info, `local`, reads, page position); the presenter keeps none. `wants` (reads the host
runs: `files.dir`, `mail.*`, ...) go through `session::run_wants(hub, conn, target, wants)`, the host half of parity-session's `wants` stack, once it is re-based on this.

## The seam: a drop-in for `Presenter::view`

The contract is the one that exists, so nothing outside `present/` changes. A `Screen::view(&params) -> Option<Value>` (inside `Routed`, which implements `Presenter::view -> Fut<Value>`) takes the **same params the
hub builds for the helper** (`session/mod.rs view_request`): `version, generation, epoch, hub, requestRevision, target, session` (the actor's input), `action`/`value`,
`revision` (= `signal_seq`), `viewId`, `nativeKey`, `now`, `window`, `page {thread, before, below, above, rows}` (32 rows in the cells `seq ord id thread src kind
role akind text at`), `reads`, `local`, `seq`. It returns the **same object the helper's `V` returns**: `shown`, `session`, `cmds`, `wants`, `nextTarget`,
`overlay`, `local`, `refused`, `stale`, `window`. `wants` is not emitted by any Bend on `gpui` (it is gpui-slim dbadca0f2e, unbuilt, now dropped): the port introduces it as the Rust presenter's way to ask the host for read-only RPCs (`files.dir`, `mail.*`, ...), served by parity-session's `session::run_wants`. `None` means not ported: `Routed` asks the helper if it is allowed to run (stage 1 rules below), else answers a retryable "not ported". The actor, `assemble`,
deltas, segmentation and `local` stay as they are.

The other `Presenter` methods under `Routed`: `fold` feeds `meta` (state::State lean), the info map and the live rows, and also the helper if it is running; `wants_log` is false while the helper is not
running (the hub then does not fold the whole log into anything but `Model`/`meta`); `ping` answers for the Rust side; `is_down`/`restart` concern only the helper, and only when it was started.
Mirrors: `rebuild_presenter`, `forget_mirror` and far events change presenter state only, so `meta.rs` keeps `<link>~<id>` threads in a map of their own (per link, dropped by `forget_mirror(prefix)`), never in `Model`.

**The oracle, in two tools and a gate.**
1. *Dumps (working tool, not the gate).* The helper's `V` depends on more than the params: everything folded through `Presenter::fold` (`log, changes, delta, info, live`).
   With `BACKPLANE_PRESENTER_DUMP=<dir>` a hub writes `frames.jsonl` (every fold frame, in order) and `views.jsonl` (each `(params, helper output, frames-so-far index)`).
   Dump safety: it holds message text, drafts and `session`, so it is for **synthetic logs only**; the hub writes it 0600, and only on a copy run, only while the helper runs, and never for a log above `copy::BIG_LOG` (256 MiB). The code writes `views.jsonl` today; `frames.jsonl` and the frame index are the next step and are what make this oracle work. The differential test replays the frames up to that index into a `RustPresenter` (its `fold` builds `meta`/info/live; the events go into a temp store so `Model` indexes them) and compares
   the view output. Exact comparison after only these normalisations: clock fields (`at`, `now`, `rid`, `clock`), view ids and message/thread ids by first appearance. Anything else is a difference.
2. *The gate is the JS replay, not the helper.* The helper differs from JS today (`settingspages` 347 and `projecthome` 33 differences, still UNSETTLED), so 0 differences against dumps can mean
   copying the helper's bugs. A module lands when `test/gpui/parity` (scenario `session:gpui`, normalisations N1-N19 as listed in its README, none added) shows 0 differences against the JS recordings,
   on the baseline rebuilt from the frozen main. Where the helper and JS disagree, JS wins and the difference is listed with the module.
3. *Scale.* The helper cannot open the real-size log, so `session:gpui` never reaches multi-page threads, cursors or `remaining`/`total` at scale. Add a synthetic log the helper can still fold
   (about 100k events, threads of 100+ entries, several pages each) to the dumps and the replay, and a JS-hub spot check on the 1.69M synthetic log on Boat for `window`/`pages` if it fits in the budget; if it does not, say "unmeasured".
4. *No table fitted to the scenarios.* A `shown` key GPUI does not read is either computed from state or left out and named as an approved difference with the reason; never a constant that matches only because the recording is small.
   Actions get a coverage list generated from client.bend's `Ui.act` cases (every action name, with: covered by a replayed pair / ported with a hand-written case / not ported -> helper). A dumped pair is a test, not the contract: branches no scenario reached are listed, not assumed.

Actions are a table in `actions.rs`: `fn act(cx: &Ctx, req: &Req) -> Option<ActOut>` where `ActOut { session_patch, cmds, next_target }` and `req.action`/`req.value` are the
`view.action` pair (`pin`, `row-pin`, `row-settle`, `row-archive`, `new-thread`, `rename-save` ...). The names and the `cmds` they produce are read from the dumps and
client.bend's `Ui.act` (the cases for `Menu.*`, `Ren.*`, `Made.*`); the RPCs under them (`thread.create/pin/settle/rename/archive`) are done and parity-tested (commands).
An action's contract is JS behaviour: the dumped pair is evidence for the branches a scenario reached, the coverage list above says which ones those are.

## The indexed read model

- Log: offset index (28 B/event), unchanged. Entries are read by `seq` from the store through a bounded page cache (decoded pages, LRU, a few MB), never folded.
- `meta.rs`: threads (11k at real size), projects, settings, asks, queues, bots, rooms, usage. Per thread: counters (total, last message id/preview/time, status) updated at apply. `meta` grows with objects (threads, projects, tasks, kept settings), not with events; the log index does grow with events.
- A thread's page = rows read via `model.rs` (`ids`, `ord`, lookahead 17), as today; `window` is computed from the page plus the thread's latest human/assistant text (`message-context`), read from the index tail.
- Start reads the log once to build `meta` and `model` (streaming, no retained lines). The real slope is not zero: `store.rs` keeps an offset per event and `Model` keeps `all: Vec<u32>` plus `own`/`sent` entries (8 B each) per event. Measured: 38 MB RSS at 1M events with no helper alive (the earlier 71 MB figure came from a hub whose helper had died, so it is not a steady number). The measurement states B/event and checks it against a number: at 0.85M and 1.69M synthetic events, start-to-listening, peak and steady RSS, slope between the two sizes, view latency. Stated plainly: memory grows linearly with events. Measured so far: about 38 B/event all in (38 MB at 1M events, 18.7 B of RSS per committed event); that is about 64 MB at 1.69M and 380 MB at 10M. The bar is the measured slope at most 64 B/event, and RSS steady while no events arrive; not "flat".
- `meta.rs` keeps only the settings `fam::policy::lean_keeps` lists, with `lean_value` caps, because values reach 10-20 KB (state needed it); everything else is read from the log or the hub when asked.

## `--policy-bin` and `--window-bin` go away in stages

1. **Router, flag `--presenter rust|helper`** (default `helper`; `rust` also when `--policy-bin` is absent): `RustPresenter` serves what it has ported. **There is no lazy helper on a real-size log:** with `--presenter rust`
   the helper is never started, and a screen that is not ported answers a retryable "not ported" and starts nothing. (`helper` is today's behaviour, for small logs and the phones' sake; it is not allowed on a log above
   `BACKPLANE_HELPER_MAX_EVENTS`, default 150,000 events, where the hub refuses to start it and says why.) `gpui-compact` is not part of this.
2. **Default `rust`**: the helper is only for screens GPUI desktop does not use (phones' extras, bots/rooms tabs, settings subpages, viewers) until they are ported in the same module layout, in the order the apps need them.
3. **Delete**: when the replay oracle shows 0 differences on every scenario and the phones need nothing from the helper: remove the helper, `window`, `native/bend/*` runtime use, `--policy-bin`, `--window-bin` (accepted and ignored with a warning for one build, then refused by `Cli`), the Boat/bp-build steps that build them.

## Laws and tests

Each ported def gets a Rust property test (seeded generator, no new crates) named after the Bend law it guards, and a line `law NAME rust TEST` in `scripts/gpui/law-map.tsv`
(the tool's own format; `python3 scripts/gpui/law-map.py --check` must pass in every PR). Laws still served by the helper stay `law NAME bend`; at stage 3 every
such row must become `rust TEST`, `unported` or `retired <reason>` in the same PR that deletes the helper, since `bend` means Rust calls it and PROOF still checks it. No row is left claiming a
check that no longer runs. Bend proofs do not gate the runtime. The gate for a module is the JS replay (see the oracle), not these tests.

## Order of work and honest limits

1. Day 1: slim inventories what `native/gpui/src/screens` reads from `shown` (keys, shapes) from the recordings; bend lands the `present/` seam with an empty router (no behaviour change).
2. Then in parallel: thread/window/pages (slim); composer/popups/oracle (parity-session); actions (commands); `meta.rs` and the `--presenter` flag (bend).
3. Not known yet: the exact `shown` surface GPUI depends on, and how many of client.bend's behaviours (tour, star nudge, find, tabs) show up as non-constant output. Until slim's inventory, any estimate is a guess; I will give one after it.
4. The phones use the same `shown`, and the helper folds the hub's log whichever client connects: **phones do not work on a real-size Rust hub until stage 2 or 3** (every screen not yet ported is "not ported" there). On the small hubs the helper can fold (`--presenter helper`) they keep working.
