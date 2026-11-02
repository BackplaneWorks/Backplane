# Active-thread lag investigation — 2026-11-02

The running hub was inspected read-only. No signal, inspector mutation,
restart, settings change or thread action was used. `perf stat` was denied
by `perf_event_paranoid=4`; there was no inspector listener. Evidence comes
from existing `/debug/perf.json`, `/debug/log`, `/proc`, a bounded event-log
tail and read-only SQLite queries. No private transcript is included here.

The installed executable identified itself as `0.13.1-dev.local.138b2731e`.
Between uptime 850.876 and 863.400 seconds (12.524 s):

| Measurement | Result |
| --- | ---: |
| Hub CPU | 71.15% of one core |
| RPCs | 22; 4.735 s total; 215 ms mean |
| Minute tick | 1; 1.775 s |
| Agent lines | 43; 0.627 s total |
| Commit spans | 94; 0.542 s total |
| Network writes | 49; 17,531 bytes total |

Spans are inclusive and must not be added together as disjoint CPU time.
Since boot, RPCs had used 380.6 seconds and averaged 269.6 ms. The log
repeatedly reported 200–300 ms RPCs. Peak RSS was 8.26 GiB; RSS at the end
of the interval was 4.62 GiB. These are process measurements, not evidence
that memory pressure caused the delay.

A recent minute contained 441 events, including 239 SettingSet events;
127 were liveness stamps. The thin-notice planner previously classified
all settings as global urgent changes, bypassing its 500 ms sidebar gap.
The live metadata snapshot contained about 9,101 threads (9,055 ended),
472 historical tasks, and 8,279 worktree paths sharing only 75 locations.
The tick repeatedly looked up the parent of every ended thread by scanning
all tasks: roughly 4.27 million comparisons. Only 27 worktree-drop candidates
were observed; do not confuse all saved worktree paths with expired folders.

Source inspection also found that `Hub.step.fallback` reparsed the model
catalog on every step, including a nil-change step containing only streamed
text. The stream bypass runs after lifecycle, lock and watchdog hooks, and
retains normal validation for changes or other effects. An arbitrary-state,
arbitrary-capability proof checks the exact skipped transforms for any finite
sequence of live outputs. New phase counters distinguish selected-history
reads, thin methods, and tick sampling/planning from aggregate RPC time.

On the client, an ordinary accepted update scheduled a full paint from both
session and readiness callbacks, even when its delta changed only the
sidebar. The regression fixture exercises actual socket/ThinViews/ThinHost
callbacks and the host paint scheduler: fifteen updates used to request
fifteen full paints and now request zero, with both two and 600 timeline
rows. They patch the sidebar, without timeline projection or timeline hooks.
The timeline region also runs the same unread/bottom reconciliation as a full
paint, including pill-only changes with unchanged row geometry. A dedicated
region-path regression covers this integration with the existing reading
controller. This fixture does not measure real-browser input latency.

## Validation records

The first isolated installed-binary experiment offered twenty 40 Hz streams
and three thin views, with a full legacy observer. It used 156.9% of one CPU
core. Quiet views received about 9–14 notices/s, above the 2/s budget; delivery
fell behind, so old mixed-phase commits leaked into the next phase. That run
fails the workload checks and is diagnostic evidence only. It used no
historical metadata fixture and must not be used as the matched comparison
for the final default 1,000-metadata-thread workload.

The committed load test records exact fixture/client and executable hashes,
per-thread offered and received rates, lag/gaps, actual process CPU, notice
rates before client coalescing, request/byte rates and performance-counter
windows. It fails on zero/dead streams or backlog at phase boundaries. The
stream-only isolation assertion accounts for observed periodic watchdog
metadata writes; those writes and all traffic still count in the CPU and
notice budgets. The live hub is never used by this test.

## Tick planner comparison

Boat ran the original and indexed planner back to back over the same
synthetic state and compared their complete serialized changes and effects.
The fixtures include historical tasks and 12–20 running threads. These are
raw emitted-JS planner measurements, not production-hub CPU or user latency:

| Threads | Active | Original | Indexed | Exact output |
| --- | ---: | ---: | ---: | --- |
| 128 | 12 | 318 ms | 15 ms | equal |
| 512 | 12 | 5,354 ms | 59 ms | equal |
| 1,024 | 20 | 24,465 ms | 117 ms | equal |

The same job passed eight notification checks, all 24 tick/Boat-chip parity
cases, and the arbitrary-state/capability stream-bypass theorem. It then
stopped on an ownership annotation in the stream test fixture; that fixture
error was corrected before the next validation run. Benchmark source is
`test/native/tickplan_bench.bend` and parity fixtures are in
`test/tickplan_test.bend`. Job: `j20261102194722-383034`.

## Matched compiled-hub comparison

The matched default fixture (twenty 40 Hz producers, 1,000 historical metadata
threads, three thin views and a legacy observer) ran both binaries serially on
`h@backplane`, inside a 20 GiB no-swap scope with the native build lock and
pinned build tools. Both runs have the same fixture SHA-256. The after binary
was built from the source subsequently merged in PR #711.

| Metric | Installed 138b2731e | After #711 |
| --- | ---: | ---: |
| Actual hub CPU, combined phases | 145.27% | 144.43% |
| Load gate | FAIL | FAIL |

This does **not** demonstrate a meaningful overall hub CPU improvement. The
after run still delivered only about 22–30 deltas/s per thread against 40/s
offered, accumulated multi-second lag and sent quiet views about 11 notices/s.
The 80% CPU and 2/s sidebar targets remain unmet; the CI test should continue
to expose them. No rate, liveness or CPU assertion was relaxed to obtain a pass.

The new phase counters attribute 11.30 s of the after mixed phase to 375 RPCs
and 8.69 s to 4,044 agent-line messages in a 20.03 s interval (inclusive spans).
Source tracing then found a separate feedback path: a reply/info/sheets update
changes one actor's private Ui, but `Cfg.thin.dirty(True)` invalidated every
view via a global bump. Its refreshes can issue further private follow-up
reads, invalidating every view again. A recipient-scoped follow-up is being
qualified separately. Do not infer that PR #711 fully resolves app lag from
its planner benchmark or client paint checks.

The after executable SHA-256 is
`d1ed7e5be2565d5064ee256993bf1286ad01e43f2e27510d5d9e011ae42075cd`.
Raw reports are retained in this worktree's ignored `build/perf/` directory.
The complete repository PROOF invocation for #711 finished with only its
2,470 unsafe/foreign-effect declarations and was accepted by
`scripts/bendok.sh`'s exact `bend_verdict`; this is not a claim that foreign
implementations are mathematically verified. The three sidebar full-output
fixtures also passed on Boat (31 checks). Local and remote build locks were
released for the installer, and no live hub was restarted by this work.


## Scoped commit and private-result follow-up

Source tracing found that both durable and queued nonempty commits called
`Live.with`, which rebuilt every live stream and added an urgent global `*`
notice before the canonical commit added its own notices. Liveness, usage and
tool metadata therefore bypassed the sidebar's soft cadence even though their
live buffers were unchanged. The follow-up preserves the original live state
and stream map for metadata, and snapshots only threads whose assistant
message or terminal turn changes their buffer. Checkpoint synchronization,
steering tails, generation changes, and canonical commit ordering are retained.
The canonical commit now owns the single revision bump and normal notices.

The same change carries requesting connection IDs through private actor result
drains. Private replies/info/sheets can wake their requesting connection without
creating sidebar work in other connections, including views of the same thread.

Boat job `j20261102201107-677175` passed the selective stream parity fixture
(including the arbitrary no-ending identity proof), 24 private-notice checks,
and eight existing progress-notice checks. Peak RSS was 3,435,200 KiB. Independent
source review found no lifecycle gap. Full repository proof work
`w-t1793648028-1245355-1258404` is still running; its result is not claimed here.
The private follow-up build/load is deferred at the boundary after that proof
so the installer can build the next combined, exact main. No compiled CPU
improvement is yet established for this follow-up.
