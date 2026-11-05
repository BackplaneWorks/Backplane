# Porting the hub to Rust

Decision (2026-11-04): the backend moves to Rust. Bend keeps only the small, pure
rules worth keeping in it: policy, validation and state-transition rules. Everything
that changes over time goes to Rust: the event store, sockets, files, processes,
broadcast and caches.

Scope: the `gpui` branch and the Backplane GPUI project only. `main` and the live
Backplane stay untouched, and nothing merges to main without the person's OK.

Why, from `NATIVE-BEND.md`: native Bend is 1.4-2.7x faster than Bend on JS and uses far
less memory, but stays 3.6-95x slower than plain TypeScript on hub hot paths, and the
whole hub cannot be emitted to C (212 JS-only effect defs; the emit exceeds 8 GiB).

## Target

```
web, phones, GPUI  ──  today's protocol, unchanged  ──  backplane-hub (Rust, native/host)
                                                         ├ event store, windowed history
                                                         ├ broadcast, fan-out, view sessions
                                                         ├ threads, sidebar status
                                                         ├ agent processes
                                                         ├ files, git, KiCad IPC
                                                         └ calls: Bend rules; presenter
```

`events.jsonl` keeps its format: persisted events stay decodable forever, and the Rust
hub reads a log the Bend hub wrote, and the reverse.

## The presentation layer

Exact parity needs more than sockets. The hub draws what clients show: the phones and
GPUI get a screen model (`Render.mobile`), and the web page gets nodes (`src/web/view.bend`).
That code is client.bend, screen.bend, hubs.bend and view.bend: about 27,000 lines, far
from small, and it depends on much of core.

It is pure, so the plan is staged:

1. **Presenter sidecar (transitional).** Rust owns all state and asks a presenter for a
   picture. `bp-policy` (native Bend) serves `renderer: mobile`. The web renderer may
   need the existing Bend-on-JS code in a child process until it is ported. A hub with that
   child is not yet JS-free, and every report must say so.
2. **Screen model to Rust.** Row by row and screen by screen, each compared with the Bend
   output on recorded traffic. This also removes the O(threads^2) sidebar render.
3. **Web nodes last.** The largest and least valuable part to port.

Measured so far on the presenter: `-O0` only (114 MB, an optimized build exceeds the cap
until the C is split into units with `cc-split.py`); render is O(threads^2) in shared
Bend code, and the JS hub has it too (1,000 threads: 503 ms).

## Milestones

Each milestone ends with its gate passing and its numbers reported. Later milestones
start only when the one before passes, except where noted.

| # | Work | Gate |
|---|---|---|
| M0 | Parity harness; baselines; law map; store copy | Harness replays recorded traffic against today's hub and reports zero differences against itself |
| M1 | Protocol: HTTP, authenticated WebSocket, join, view open/update/action, CBOR frames | Recorded sessions match the JS hub; web, phone and GPUI clients connect to the Rust hub on the store copy |
| M2 | Event store and windowed history | Same log, same pages; torn tail, restart and 1M-event tests pass |
| M3 | Broadcast and fan-out | Keyed deltas match; slow clients bounded; 20-agent load valid |
| M4 | Threads and sidebar status | Sidebar and settle state match on recorded logs; 300 and 6,666 threads stay fast |
| M5 | Agent process management | Stand-in turns match event for event; interrupt, resume and restart tested |
| M6 | Files and KiCad IPC | Files tab, viewers and `kicad_ipc` match; plots served to all clients |
| M7 | 24 h run | 24 h with 20 simulated agents on the store copy, with no rising memory floor |

## Measurement

Every milestone reports against the JS hub on the same store copy and machine:

- Startup to usable
- Steady memory (24 h at M7; a shorter simulated run before that, labelled as such)
- CPU under the 20-agent load
- Click and popup latency: sidebar click, thread switch, composer popup
- Build time and peak build memory; binary size

The live hub is only observed, read-only, through `/proc`. Load tests use simulated
agents only, never real ones.

## Laws

`LAWS.bend` holds 3,248 laws and stays as it is: humans own it, and no law is weakened
or deleted. Each law has one of these statuses, listed for every law below:

- **bend**: the logic stays in Bend and Rust calls it. The law still checks that code.
- **rust**: the logic moved. A named Rust property test states the same claim.
- **unported**: the logic has not moved. The law still stands on the Bend code.
- **client**: client-side Bend in the web and phone bundles, unchanged.
- **window**: native-window code GPUI replaces. The entry says what GPUI does instead,
  or that it has no equivalent yet.

First classification, by the file whose defs a law names (within about 10% until each
law is checked; detail in `PORT-CLASSIFICATION.md`): 1,470 stay in Bend, 963 move to Rust,
581 are client-side and 221 are native-window code.

A function stays in Bend only if it is pure, takes and returns kilobytes rather than a
state, is off the per-event and per-view hot paths, and has a law or security property
that justifies it. Those rules run in one persistent process that can do no IO. Bend's
`State.replay` stays as a differential oracle: every recorded log must give the same
canonical state digest in Bend and in Rust.

Porting a piece of logic changes its laws from `unported` to `bend` or `rust` in the same
commit. `scripts/gpui/law-map.py` fails when ported code has a law with neither, or when
a named test does not exist. A law with no honest Rust statement stays in Bend, or the
port waits for the person's decision.

## Build and test rules

- Builds run on Boat (8 GiB cap, zero swap) or the Mac, never on the desktop.
- The hub runs on a copy of the event log on its own port, with phone access over the
  tailnet. It never touches `~/.backplane-bend` or port 3787.
- The copy holds real conversations: it stays on this machine, out of git and off
  Boat. Parity recordings sent to Boat come from fixtures only.

<!-- law-map: generated below by scripts/gpui/law-map.py -->
