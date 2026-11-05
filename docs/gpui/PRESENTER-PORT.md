# Presenter port: the Bend helper leaves the runtime path

Decision (the person, 2026-11-05): no Bend in the running hub. `bp-policy-pager` (the policy) and `window` (the window helper) go;
what the hub needs moves to Rust. Start with exactly what opens the real log and serves GPUI desktop: sidebar (Rust already,
0 differences), thread view and pages, composer, sending, turns, thread create, settle, rename, archive. State is windowed and
indexed, never the whole log in memory. Bar: **same answers as today**, checked by replay against JS recordings. Measure on
`~/.backplane-gpui-scratch/synthetic-real-size.jsonl` (1,691,620 events, 1.14 GB) only, never a store copy. `gpui-compact` is stopped (branch kept).

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

The contract is the one that exists, so nothing outside `present/` changes. `RustPresenter::view(&params) -> Option<Value>` takes the **same params the
hub builds for the helper** (`session/mod.rs view_request`): `version, generation, epoch, hub, requestRevision, target, session` (the actor's input), `action`/`value`,
`revision` (= `signal_seq`), `viewId`, `nativeKey`, `now`, `window`, `page {thread, before, below, above, rows}` (32 rows in the cells `seq ord id thread src kind
role akind text at`), `reads`, `local`, `seq`. It returns the **same object the helper's `V` returns**: `shown`, `session`, `cmds`, `wants`, `nextTarget`,
`overlay`, `local`, `refused`, `stale`, `window`. `None` means not ported: the router asks the helper (stage 1) or answers "not ported". The actor, `assemble`,
deltas, segmentation and `local` stay as they are.

This makes the oracle exact and cheap: with `BACKPLANE_PRESENTER_DUMP=<dir>` a hub (copy mode, synthetic log) writes each `(params, helper output)` pair; the
differential test feeds the dumped params to `RustPresenter` and compares outputs (after the harness' normalisations: clocks, ids). Each module lands at 0 differences over
the dumps of the `session:gpui` scenarios (the JS recordings' trace through the same requests), then the network replay (`test/gpui/parity`) confirms end to end.

Actions are a table in `actions.rs`: `fn act(cx: &Ctx, req: &Req) -> Option<ActOut>` where `ActOut { session_patch, cmds, next_target }` and `req.action`/`req.value` are the
`view.action` pair (`pin`, `row-pin`, `row-settle`, `row-archive`, `new-thread`, `rename-save` ...). The names and the `cmds` they produce are read from the dumps and
client.bend's `Ui.act` (the cases for `Menu.*`, `Ren.*`, `Made.*`); the RPCs under them (`thread.create/pin/settle/rename/archive`) are done and parity-tested (commands).
Nothing is guessed from the Bend source alone: an action's contract is its dumped `(action, value) -> (session, cmds, nextTarget)`.

## The indexed read model

- Log: offset index (28 B/event), unchanged. Entries are read by `seq` from the store through a bounded page cache (decoded pages, LRU, a few MB), never folded.
- `meta.rs`: threads (11k at real size), projects, settings, asks, queues, bots, rooms, usage. Per thread: counters (total, last message id/preview/time, status) updated at apply. Memory grows with objects, not with events.
- A thread's page = rows read via `model.rs` (`ids`, `ord`, lookahead 17), as today; `window` is computed from the page plus the thread's latest human/assistant text (`message-context`), read from the index tail.
- Start reads the log once to build `meta` and `model` (streaming, no retained lines). Targets to prove on the synthetic log, at two sizes (0.85M and 1.69M events): start-to-listening, peak and steady RSS, view latency, 20 stand-in agents. The Rust hub alone was 38 MB at 1M events; the helper was the 3.6 GB.

## `--policy-bin` and `--window-bin` go away in stages

1. **Router, flag `--presenter rust|helper|auto`** (default `helper`): `RustPresenter` serves what it has ported (sidebar page, thread view, window, pages, composer, the listed actions) and sends everything else to the helper, which is **started lazily** on the first request that needs it and folded then (compacted, from `gpui-compact`); GPUI's main screens never start it, so the real log opens without its memory. Unported and no helper: the reply says so (retryable error), never a wrong screen.
2. **Default `rust`**: the helper is only for screens GPUI desktop does not use (phones' extras, bots/rooms tabs, settings subpages, viewers) until they are ported in the same module layout, in the order the apps need them.
3. **Delete**: when the replay oracle shows 0 differences on every scenario and the phones need nothing from the helper: remove the helper, `window`, `native/bend/*` runtime use, `--policy-bin`, `--window-bin` (accepted and ignored with a warning for one build, then refused by `Cli`), the Boat/bp-build steps that build them.

## Laws and tests

Each ported def gets a Rust property test named after the Bend law it guards (seeded generator, no new crates), and a row in `scripts/gpui/law-map.tsv`
marking it `rust-test:<path>`; laws not ported stay `bend` and are not claimed. Bend proofs do not gate the runtime. The oracle is the bar, not the tests: a module
lands only with replay differences at 0 for its scenarios (or each difference named and approved).

## Order of work and honest limits

1. Day 1: slim inventories what `native/gpui/src/screens` reads from `shown` (keys, shapes) from the recordings; bend lands the `present/` seam with an empty router (no behaviour change).
2. Then in parallel: thread/window/pages (slim); composer/popups/oracle (parity-session); actions (commands); `meta.rs` and the lazy helper (bend).
3. Not known yet: the exact `shown` surface GPUI depends on, and how many of client.bend's behaviours (tour, star nudge, find, tabs) show up as non-constant output. Until slim's inventory, any estimate is a guess; I will give one after it.
4. The phones use the same `shown`; they stay on the helper until their screens are ported (stage 2), so the helper's memory problem for them remains until then (their data is small; `gpui-compact` is the stopgap if needed).
