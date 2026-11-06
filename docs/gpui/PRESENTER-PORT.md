# Presenter port: the Bend helper leaves the runtime path

Decision (the person, 2026-11-05): no Bend in the running hub. `bp-policy-pager` (the policy) and `window` (the window helper) go;
what the hub needs moves to Rust. Start with exactly what opens the real log and serves GPUI desktop: sidebar (Rust already,
0 differences), thread view and pages, composer, sending, turns, thread create, settle, rename, archive. State is windowed and
indexed, never the whole log in memory. Bar: **same answers as today**, checked by replay against JS recordings. Measure on
`~/.backplane-gpui-scratch/synthetic-real-size.jsonl` (1,691,620 events, 1.14 GB) only, never a store copy. `gpui-compact` is stopped (branch kept).

## The memory gate (accepted by the person)

From `docs/gpui/perf-gates.json`: **whole startup under 1 GB, steady hub RAM at most 500 MB, with about 10 agents and 5 views open; `/hello` answers and nothing runs out of memory; building the store counts as part of startup.
Measured as anonymous memory of the hub's scope, helpers included and agent processes excluded (run with stand-in agents): startup is its peak, steady is its value after the agents and views settle. File cache is reported beside it, not counted.**
This replaces "flat" and every earlier proposal (64 B/event, 500 MB at 10M); the 10M figures below are headroom information, not the gate.

**The real log, counted** (the store copy `~/.backplane-gpui-copy/events.jsonl`, 1,555,265,941 bytes; a count only: no hub, no content; `python3 scripts/gpui/count_events.py FILE` prints integers, event kind names and setting-key prefixes, never a value, id, text or full key):
1,691,619 lines (the number perf reported; an earlier count of 1,698,796 read a different file, an older backup under the scratch folder, and is withdrawn); 74 kinds, no line that is not an event; `SettingSet` 620,596 (36.7%), `ActivityLogged` 301,120, `MessagePosted` 156,608, `UsageSet` 103,704, `TurnChanged` 97,769.
**12,446 `ThreadCreated`**, created steadily: 1,898 by 25% of the events, 5,714 by 50%, 9,317 by 75%, 12,446 by 100% (about 7.4 per 1,000 events, no sign of flattening). So threads grow in proportion to events in the real log (unlike the synthetic one, which makes its 11,277 threads in its first 13k lines):
about 74k threads at 10M events if that rate holds, an extrapolation. Threads with entries: 12,738. Messages and activities: 457,728 (27.1% of events).
**Settings keys: 186,511 distinct keys, mean 39.7 bytes, from 620,596 `SettingSet` events.** By prefix the largest are `fallback` 23.5k, `run` 20.8k, `summary` 18.9k, `turn` 16.7k, `account` 16.3k, `restart` 11.5k, `provider` 11.5k, `tok` 11.3k: keys that grow with the log, so `Model.settings` (a `String` key per distinct setting) is a growth row, not a constant.

**Against the gate, at the real log's 1.69M events** (code estimates, none measured for today's hub; this is a likely outcome, not a result):
- Index and read model, from the code: 16 B per event (the index tuple) + 0.271 x 12 B per entry (`all` 4 B + `Ent` 8 B) = about 19 B/event, **about 33 MB**. The "38 MB at 1M events" is a different thing: a whole older hub's RSS (commit 89a5d610ed), kept as that.
- `Model.settings`: 186.5k keys x (39.7 B of text + 24 B `String` + 4 B value + hash-table overhead, est. 16 B) = about 17 MB, est.
- Folds: `Sum` up to 6 KB x 12.4k threads = 75 MB worst case, real far less; outboxes 5 views x up to 8 MiB = 40 MB worst case; the views' kept bodies 5 x 3 x 300 KB = 5 MB (**guess**: the 300 KB is a loose reading of "336 KB per body at 301 threads" in PORT.md, which grows with threads; at 12k threads the body is far larger and must be measured); hub baseline about 10 MB (the earlier spike host, a guess for today's hub); page cache 8 MB (a plan).
  Those rows add up to about 200 MB; **the agents' `World`, the bots' `World`, `Feed`, `Bindings` and `meta` (`Extra`) are not in it and are unmeasured.** So: about 200 MB plus the folds nobody has measured, against a 500 MB gate. A pass is likely, not shown.
- Start peak: the index `Vec` doubles while it grows (1.69M x 16 B = 27 MB, up to 54 MB while it is copied), plus `Model`, the folds replaying the log, and the baseline: est. 150-350 MB. Which loaders stream today: `store.rs open` (a 1 MiB buffer, one reused line buffer, nothing retained) and `Model::build` (4096-line chunks through `store.range`) stream; the state policies' boot fold, `net/feed.rs` and `agents/tasks.rs` also read through `store.range` windows; the bots' world fold was not checked. Dropping the log's page cache while reading (`posix_fadvise(DONTNEED)`) is a **plan**; no such call exists in `native/host/src` today.
- (b), the cheaper index and entries (est. 3-4 days, low risk, no on-disk format change): headroom for 10M (est. 70-125 MB for the index and model, plus about 74k threads of folds), not a pass condition. (c), a flat on-disk index (est. 1-2 weeks, larger design): not needed for this gate.
- What the gate cannot show: 10M events. The per-structure breakdown at 0.85M and 1.69M gives B/event and B/thread, and the 10M number is that slope times 10M, labelled an extrapolation. Duration: the 24 h soak was dropped, so the claim is the steady value after settling, as the gate says.

### Headroom: what 500 MB at 10M events would take (not the gate)

Budget: 500 MB / 10M events = 50 B/event, minus everything that is not the log index. That is more than the index and the read model, and **it is the likeliest place for this plan to be wrong**; none of the rows below the line is measured for today's hub.
Allowance for the index and read model = 500 MB - (baseline + folds + page cache + outboxes). Assumption stated: outboxes cut from 8 MiB to 2 MiB x 20 clients = 40 MB (at 8 MiB they are 160 MB and the allowance is 320 MB, 32 B/event; at 4 MiB, 80 MB).

| Row | Size | Source |
|---|---|---|
| hub baseline | about 10 MB | the earlier spike host; today's hub unmeasured (guess) |
| page cache | 8 MB | a plan |
| outboxes | 40 MB | assumption above; `OUTBOX_MAX` is 8 MiB today |
| `state::State` (lean) per thread/project/task | unmeasured | the real log has 12,446 threads at 1.69M events, created at a steady rate (see the gate section): about 74k at 10M if that rate holds, an extrapolation (the synthetic log makes all 11,277 of its threads in the first 13k lines and says nothing about growth) |
| `state/` `Sum` per thread (`last_user`/`last_reply` up to 2000 chars each, 64 recent message ids) | up to about 6 KB/thread worst case, 390 MB at 65k threads: the whole budget; real values far smaller, unmeasured | `state/summary.rs` |
| agents `World`, bots `World`, `Feed` `Index` and `Bindings` (each its own fold) | unmeasured | `agents/`, `bots/`, `net/` |
| `meta` for the presenter (`Extra`: runtime, fork source, usage, todos, background rows) | unmeasured | `present/meta.rs`, to be added |

The per-structure heap breakdown that comes first must cover **every row**, not just `store` and `model`, at 0.85M and 1.69M events, and report B/thread as well as B/event.

Log index and read model, from the code:

| Structure | Today | Change | After (estimate) |
|---|---|---|---|
| `store.rs` `idx: Vec<(u64, u32)>` per event | 16 B (tuple padding) | offsets in blocks of 64: one `u64` base per block, `u32` lengths delta-coded | about 4.5 B |
| `model.rs` `all: Vec<u32>` per entry | 4 B | kept | 4 B |
| `model.rs` `own`/`sent` `Ent{seq,ord}` per entry | 8 B | drop `ord` (it is the position in `all`, which is sorted; recover by binary search) | 4 B |

Entries are messages and activities only: 27% of the real log's events (458k of 1.69M). Estimate: 4.5 B per event plus 8 B per entry, so about 6.7 B/event on the real mix (12.5 B/event if every event were an entry), 67 to 125 MB at 10M.
These are estimates until measured: the first deliverable is a per-structure heap breakdown at 0.85M and 1.69M events, then the changes above, then the same run again. Also needed: start streams the file without retaining lines, so the start peak stays in the same
budget (checked in the measurement); outboxes counted in the budget. If the estimate is wrong the fallback is a sidecar index file read through `mmap`/`pread` (anonymous memory stays flat; touched file pages count as reclaimable cache, which a cgroup limit may or may not
count as used), a larger design that I would bring back to you before starting.

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
- Start reads the log once to build `meta` and `model` (streaming, no retained lines). The real slope is not zero: `store.rs` keeps an offset per event and `Model` keeps `all: Vec<u32>` plus `own`/`sent` entries (8 B each) per event. Measured: 38 MB RSS at 1M events with no helper alive (the earlier 71 MB figure came from a hub whose helper had died, so it is not a steady number). The measurement runs the memory gate (above) on a copy of the real log, with about 10 stand-in agents and 5 views, and reports start peak, steady RSS, B/event and B/thread at 0.85M and 1.69M events and view latency. Memory grows linearly with events: say so, never "flat".
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
