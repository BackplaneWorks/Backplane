# Due-deadline scheduler experiment

An already-due `io_park_on` deadline represents a cooperative yield. The Bun
runtime sent it through `setTimeout(..., 0)`, paying the timer's minimum delay
after every 64 KiB static-file piece. `bp_idle` now yields once with
`setImmediate` for those deadlines. Future sleeps, descriptor polling, sender
buffer bounds, backpressure, timeout and file-descriptor cleanup stay intact.

Boat job `j20261104074929-750846`, Bun 1.3.10, AMD Ryzen 9 9950X, kernel-enforced
memory.max **3,699,998,720 B**, swap **0**. Three rounds, alternating before/after
order; two warmup transfers then ten measured transfers per encoding per round.
Every raw and decompressed body matched the original SHA-256. Raw samples and
source-function hashes are in [RESULTS.json](RESULTS.json).

| Component measurement | Before median | After median | n per variant |
|---|---:|---:|---:|
| 7,119,973 B identity transfer | 218.64 ms | 8.63 ms | 30 |
| 1,279,817 B gzip transfer | 40.98 ms | 3.56 ms | 30 |
| Small HTTP reply during four identity transfers | 0.63 ms | 1.61 ms | 15 |
| Slowest of those four concurrent transfers | 221.99 ms | 37.82 ms | 15 |

Identity range: before 217.61–221.15 ms, after 4.77–14.94 ms. Gzip range:
40.29–42.27 ms, after 1.45–5.36 ms. Concurrent small-request maximum was
1.48 ms before and 3.82 ms after: faster bulk serving consumes more CPU per
event-loop turn, but the sender still cooperatively yields every piece. The
cooperative-park counts stayed equal except four extra backpressure parks in
one after round. No zero-yield bulk-send shortcut was introduced.

Scope: **component experiment, not whole-hub/UI acceptance**. This runs the exact
production `sock_send_static` and `bp_idle` functions with real files and Bun TCP
sockets. The adapter implements Bun writable-drain waits and the scheduler's
cooperative park boundary; it excludes Bend actors, the HTTP request parser,
rendering, full startup, and provider execution. Lane C independently measured
the packaged RC4 sender at 189.35/32.32 ms identity/gzip, n=20, which supports
the timer-floor hypothesis but is not this experiment's paired baseline.

Concurrent load: this runtime-only job used its own `hub-latency` lock while a
coordinated Bend job could run in another Boat VM. Physical-host placement is
unknown. Baseline round medians were 218.41/218.96/218.76 ms; after medians
9.59/8.72/7.69 ms. The static sender used here is byte-identical to PR #897's
merge `6b031e406`; its function hash is recorded in RESULTS.json. The old
scheduler body also matches main's original scheduler exactly. The retained
RC4 whole-hub comparison was cancelled because that emission predates #897's
static sender. A valid whole-hub comparison needs an exact current-source
artifact; none is claimed here.

Validation on the same capped Boat job: **8 scheduler regressions + 29 existing
static-sender tests passed**, including future deadlines, early Bun wakes,
descriptor-only/mixed polling, exact bytes, partial writes, EAGAIN, disconnect,
timeout, truncation and descriptor cleanup. Peak process RSS was 597,316 KiB;
command wall time 20.52 s. No compiler, proof or test ran on the development
machine. No laws changed and no new whole-hub emission was made.

## Reproduce on Boat

Stage public web assets from the same built artifact (never user data) under
`test/perf/hub-latency/input/web/`. Stage the pinned original scheduler:

```sh
mkdir -p test/perf/hub-latency/input
git show 70c642b71ef713386858a66fd584c5a4dc89fed8:src/server/effects/loop.js \
  > test/perf/hub-latency/input/baseline-loop.js
```

Run through the sanctioned pinned Boat runner under the appropriate lock and
kernel cap; never run these tests locally:

```sh
scripts/heavy-boat.sh sh -c '
  node --test test/tools/idle_deadline_test.cjs &&
  node --test test/tools/static_file_test.mjs &&
  bun test/perf/hub-latency/effect_transfer.ts
'
```

Retain `build/hub-latency/effect-transfer/` plus Boat provenance. The input
directory is experimental staging and must not be committed. The scheduler
baseline function is hash-fenced, and the component script refuses an uncapped
environment. Hashes/results are aggregate and contain no private store content.
