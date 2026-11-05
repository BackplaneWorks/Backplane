# Port classification: what stays in Bend, what moves to Rust

Author: Bend stream. Date 2026-11-05. Input to `docs/gpui/PORT.md`. Branch `gpui`; nothing here touches main.
Basis: `docs/gpui/NATIVE-BEND.md` (measurements), `LAWS.bend` (3,248 laws), `src/` sizes. Law attribution is heuristic: a law is counted against the source file whose defs it names most (script in the Bend stream's notes; 13 laws unattributed). Treat counts as +-10%.

## The rule

A function stays in Bend only if **all** hold: it is pure; its input and output are small (KB, never a whole state); it is not on a per-event or per-view hot path; and a law or a security/correctness property is worth more than its cost. Everything else moves. Measured cost of calling native Bend (NATIVE-BEND.md): about 0.7 ms to spawn a child, compute 1.4-95x slower than TS depending on the kernel. So a Bend rule may be called **per request or per connection, never per event per view**.

## Where the laws land (3235 attributed)

| Class | Laws | Meaning |
|---|---:|---|
| K Keep in Bend, called from Rust | 1470 (45%) | The laws stay checked by `bend PROOF.bend` unchanged |
| R Move to Rust | 963 (29%) | Each law becomes a Rust property test; the Bend code stays as a differential oracle (see below) |
| C Client-side Bend | 581 (17%) | Not hub logic: runs in the web bundle and phone bridge. Unchanged. The hub-drawn pages use it through the sidecar until each screen is ported |
| D Out of scope | 221 (6%) | Native-window code the GPUI client replaces |

Largest files (laws attributed, class, why):

| File | Laws | Class | Why |
|---|---:|---|---|
| `src/core/hub.bend` | 415 | R | hub step/turn/queue/settle/handoff state machine; Bend stays as differential oracle |
| `src/core/client.bend` | 348 | C | Ui client state shared by web, phones and the hub-drawn pages; runs in the Bend sidecar until each screen is ported |
| `src/core/model.bend` | 216 | R | State.apply/replay read model; Bend stays as oracle, replayed against Rust on every recorded log |
| `src/core/json.bend` | 88 | R | serde_json; round-trip and number-text laws as property tests |
| `src/core/cef.bend` | 88 | D | embedded Chromium window state; desktop app only |
| `src/core/far.bend` | 75 | K | batch ordering / gap pulling / epoch rules: pure, correctness-critical; IO in Rust |
| `src/core/order.bend` | 63 | K | ordering prompts/cards: pure text rules |
| `src/core/ref.bend` | 51 | K | reference slugs and parsing |
| `src/app/nui.bend` | 49 | D | native window input |
| `src/core/fm.bend` | 48 | K | file kinds and Fm.safe path rule (security): keep, cache results |
| `src/core/google.bend` | 47 | K | OAuth/state rules: pure; HTTP and keyring in Rust |
| `src/core/kserve.bend` | 43 | K | KiCad IPC server restart/version/command rules; process IO in Rust |
| `src/core/notice.bend` | 38 | K | which events alert and under which key |
| `src/core/bothub.bend` | 36 | R | bots world state and routing: state over time |
| `src/mobile/hubs.bend` | 34 | C | phone multi-hub client |
| `src/core/fold.bend` | 29 | C | timeline fold rows (client rendering) |
| `src/core/rview.bend` | 28 | K | remote view paths |
| `src/core/kihealth.bend` | 28 | K | KiCad health words |
| `src/core/wnote.bend` | 27 | K | webhook signing/policy |
| `src/core/onboard.bend` | 25 | C | tour |
| `src/core/cmds.bend` | 25 | K | slash/command table |
| `src/core/botui.bend` | 25 | C | bot screens |
| `src/app/layout.bend` | 25 | D | native window layout |
| `src/core/mail.bend` | 23 | R | mail store state |
| `src/core/fview.bend` | 22 | K | far viewer request rules |
| `src/core/prompt.bend` | 21 | K | agent prompt text assembly |
| `src/core/plane.bend` | 21 | K | status plane rows (pure projection of small inputs) |
| `src/core/outbox.bend` | 21 | R | outgoing frame batching: part of broadcast |
| `src/core/attach.bend` | 21 | K | attachment rules |
| `src/mobile/notify.bend` | 20 | C | phone alerts |
| `src/core/subs.bend` | 20 | C | subagents panel |
| `src/core/signal.bend` | 20 | K | signal words |
| `src/core/farlive.bend` | 20 | R | live far sync state |
| `src/core/auth.bend` | 20 | K | token/credential decisions: tiny, security-critical |
| `src/server/thin-actor.bend` | 19 | R | hub view actor: revisions, memo, fan-out |
| `src/core/fmcp.bend` | 19 | K | cross-machine MCP request rules |
| `src/core/ubersync.bend` | 17 | K | op-log merge rules (CRDT-like): the most law-valuable pure code |
| `src/core/teach.bend` | 17 | K | browser task skills state machine |
| `src/mobile/view.bend` | 16 | C | phone view selection |
| `src/core/thin-pace.bend` | 16 | R | hot per-event/per-view; native Bend measured 3.6x slower than TS |
| `src/core/review.bend` | 16 | K | review comment folding |
| `src/core/refs.bend` | 16 | K | references ranking |
| `src/core/telemetry.bend` | 15 | K | telemetry plan/allowlist/signature rules |
| `src/core/issue-report.bend` | 15 | K | report assembly |
| `src/gfx/tiles.bend` | 14 | D | native window raster tiles |
| `src/core/usage.bend` | 14 | K | usage accounting |
| `src/core/autosettle.bend` | 14 | K | settle policy |
| `src/core/todo.bend` | 13 | K | todo rows |

Files not listed default by location (`src/core` K, `src/mobile`/`src/web` C, `src/app`/`src/gfx` D, `src/server` R).

## What moves to Rust, and what proves it

| Area | Rust owns | Equivalence evidence required before the Bend path is retired |
|---|---|---|
| Event store, windowed history, search | SQLite (rusqlite, FTS5), `events.jsonl` append, page/lookahead/count queries, blocks | Same SQL as `thinq.bend`; page bodies identical on a recorded store (done for 4 pages in the spike) |
| Read model | `State` and its indexes (threads, asks, tasks, queues, settings, work), `Side.ix` / phase | Replay every recorded `events.jsonl` through Bend `State.replay` and Rust; compare a canonical state digest. Unmeasured gate: native `State.apply` rate (see "Open gates") |
| Broadcast and fan-out | Touch/level/Pace, flush timers, per-client outbox, backpressure | Event-by-event decisions compared with Bend (digest-equal in the kernel harness today: TS and Bend agree on 1.45M x 4) |
| Protocol | HTTP, authenticated WebSocket, CBOR with the `cbor.bend` dictionaries, join chunks, deltas | Frame-level replay against the JS hub (below) |
| Threads/turn machine | `hub.bend`'s step: turn, queue, settle, handoff, asks | Property tests per law + differential replay of recorded logs including outs |
| Agents, ptys, files, git, KiCad IPC, vault, accounts, push, locks | Processes and OS APIs | Per-effect fixtures; see NATIVE-BEND.md section 3 table |

## What stays in Bend, and the call boundary

Kept (K): auth/credential decisions, notice and alert policy, settle policy, far batch/epoch rules, uber merge rules, ordering/review/teach/task state machines, path-safety and link/reference parsing, KiCad IPC restart rules, telemetry allowlist, webhook signing policy, time/cron/zone rules, secret masking. About 1470 laws.

Boundary, concretely:
1. **One persistent rules process** per hub (native Bend, loopback socket or stdio, one JSON line in, one out: `{"v":1,"fn":"Auth.credential","args":{...}}`). No per-call spawn (0.7 ms measured). Replies are bounded (64 KB). Versioned; the Rust side refuses an unknown version.
2. **Pure by construction.** Bend 2.0.36's C emitter refuses any def whose effect has only a `.js` twin (`no .c import`), so a rules binary that links no effects cannot touch IO. The build enforces the "pure rules" invariant.
3. **State never crosses.** Rules take the few fields they need. The one exception is the view sidecar (`bp-policy`: frames in, one picture out), which exists only to produce hub-drawn `shown` pages for parity; it is retired screen by screen. Its measured weakness (screen build O(threads^2); slim stream: 1000 threads 4.5 s at -O0 vs JS 0.5 s) is why it is not the long-term home for the sidebar.
4. **Hot checks are cached or ported.** A rule needed per request is memoised per connection; a rule needed per event is ported with a property test.
5. **Golden corpora.** For every K function Rust calls, the Bend side generates a JSONL corpus (inputs, outputs) on Boat; Rust unit tests replay it, so a rule changed in Bend and not in its Rust caller fails a test.

## Law handling (never silently dropped)

* K: unchanged, still proven by `bend PROOF.bend`.
* R: each moved law gets a Rust property test with the law's name, listed in `PORT.md`; plus the differential oracle runs on every recorded log. A law with no feasible property test is listed as **unported guarantee** with the reason, not removed.
* C and D: untouched or retired with their code.

## Open gates (unmeasured, decide before porting the big R files)

1. Native Bend `State.apply` / `State.replay` rate on a realistic event mix. If native Bend replays within a small factor of Rust, `model.bend` could stay as the state machine in the rules process (state in Bend heap, never crossing); if not, it moves. Harness: `native/bend/apply.bend`.
2. Persistent-child call latency (stdio/TCP line protocol) vs the 0.7 ms spawn.
3. Share of real hub CPU in these paths (no recorded 20-agent profile attributes it yet).
