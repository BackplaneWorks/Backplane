# Remaining mixed-load hub cost

A matched 20-agent/1,000-history run on actual main `7d4d1fdfb` still used
131.39% of one core during mixed tool/message streaming, 84.36% during pure
streaming and 115.73% overall. The acceptance limit remains 80%.

The next diagnostic copied that exact public binary (SHA-256
`1246c418d96f1ce7b7517fcca5726fc8402a67a264fdc5a167949a2e1b37b1bf`)
to h@backplane and attached its loopback Bun inspector only to the owned,
synthetic test hub. ScriptProfiler sampled the twenty-second mixed phase.
There were 17,945 stacks: 3,154 (17.58%) included the Bend JSON parser,
3,855 (21.48%) included full web-fit rendering and 2,209 (12.31%) included
String.reverse.go. These inclusive groups overlap and are not additive CPU
percentages. Agent input, delta coalescing, DB projection and outbound CBOR
encoding all repeated JSON lexical work.

The fix scans tokens without slicing and rebuilding a string tail for every
character. It preserves raw number lexemes, field order and duplicate keys;
unsupported escape forms, malformed/incomplete strings and raw lone
surrogates retain the pure lexer. Escaped orphan surrogates retain U+FFFD
repair. The pure parser is unchanged. String.reverse.go reverses Unicode code
points in one pass. Both substitutions are server-only and require the exact
emitted entry/callee shapes; the lexer also requires the unchanged JSON
module hash. Failed fences stop before output writes.

A live pending row is intentionally a static Working label. Once a stream is
nonempty in the same generation, another delta cannot change that picture.
The server keeps every stream byte, revision, checkpoint and legacy/far send,
but does not add another thin-view wake for that unchanged row. First text,
clear/presence changes and generation changes remain conservative notices;
canonical events retain their normal touches and delivery ordering.

The regression fixture now checks this actual UI contract. Mixed completed
messages must still advance with the original freshness bounds. Pure
streaming must retain exactly one valid pending row in every accepted reply,
with replies present and all twenty legacy streams meeting unchanged rate,
gap and delivery-lag limits. Missing, duplicate, wrong-thread or partial-text
pending pictures fail. CPU stays at 80% of one core; notice and latency limits
were not relaxed.

## Qualification receipts

The exact pure reference lexer/parser, native fast path/fallback, Unicode
reversal and missing/duplicate/callee fences passed. Existing native byte
counters passed 2,192 oracle comparisons (including 607 overflow cases), 21
whole-tree cases and 11 negative fences; structural comparison passed 246
oracle comparisons and five negative fences. The pending-stream fixture
passed 28 checks and its arbitrary-text pending-label theorem. The workload
self-test passed missing/invalid pending-row guards and production/delivery
checks. Final new lexer/reversal fences passed separately on h@ with the
repository-pinned Node binary.

The full PR Bend closure proof was actually run on base `61752742f` plus the
pending-notice changes. It failed at the unchanged `Laws.mail_file_one_conv`
proof (PROOF.bend line 12358), after 4:50.14, peak RSS 13,276,772 KiB, no swap.
The captured 422-file closure remained unchanged. No law or proof was
weakened, deleted or marked passed.

The current main's sidebar review pins have unrelated closure/content drift.
They are left untouched. To isolate the performance change without claiming
those pins apply to newer code, the measured build uses the qualified
`7d4d1fdfb` source plus only this hot-path patch and the same corrected fixture
for both binaries. That source guard passed with manifest
`d42062d2c317c16bc9d69bac6a446753df71f1b9ecea1b61ea3e8f369134beb1`.
The isolated Boat build uses a private entry that omits workstation-only
memory admission; canonical guards and the sidebar guard remain unchanged.
A lower compiler heap-sizing hint is used within the allocated 16 GiB VM.

The Boat source build succeeded in 279.6 s, with a 13.20 GiB emit peak. Its
binary SHA-256 is
`0024ab465ffde68836501a4d5c32d60040d97f6f729a5055a2994d9f1db509df`.
The same-host Boat before/after pair failed the load gate: 144.83% versus
147.95% combined CPU, with both phase barriers failing and substantial mixed
backlog. Mixed minimum delivered rates were 6.24/s versus 6.94/s against
40/s offered. These overloaded results do not establish acceptance or a CPU
improvement. Raw reports remain preserved under job
`j20261103010706-2938833`.

The complete native follow-up also replaces recursive Base lowercase/prefix
walks, ASCII search folding and UTF-8 encoding, preserving Unicode table
fallback, raw-surrogate bytes/errors and code-point boundaries. Base map
updates retain the original immutable root for an already-identical numeric
value; no cache or retained key/value is introduced. Cache authority, budgets,
revision keys and compact rendering are unchanged.

One stream-ended reader cycle now waits 200 ms instead of 60 ms. Its entire
batch is sent before waiting. Any final stream_event triggers this wait,
including non-text boundaries; a later-arriving non-stream line can wait the
remaining interval. Other already-read lines preserve their order. The source
review found no byte-loss, ordering, EOF-tail or lifecycle regression. The
unchanged two-second delivery/presentation budget remains an acceptance gate.

A fresh full paced-service emit on Boat was killed at its 16 GiB limit
(job `j20261103014112-3295274`, peak 15,698,584 KiB). The paced qualification
therefore retains the earlier fully emitted service and replaces exactly its
single `Delta.pace` literal body, after comparing that body with a fresh
focused Bend emit. `EMIT-DERIVATION.json` records both hashes and function
bodies. This is not a claim of a fresh full-service emit for the paced source.
The canonical stamping/native/compile steps run on that derived emit. The
installer still owns compilation of merged actual main.

The final focused Boat job `j20261103020644-3534051` passed all 15 tests,
including 32,164 Base map primitive comparisons, mixed operations/old-version
checks, 1,000 no-op numeric writes, and 51 map negative fences. The earlier
JSON/UTF-8/indexing oracle checks remain passed. The delta fixture proves the
200 ms budget and preserves ordering, block boundaries and subagent behavior.

## Strict matched h@ results so far

Both binaries use the same checkout's fixture, 20 producers at 40 deltas/s,
1,000 settled catalog threads, 20 seconds mixed and 10 seconds pure streaming.
The latest measured pair before the map no-op addition was:

| Measure | Public main 7d4 | Indexing/batching patch |
|---|---:|---:|
| Mixed hub CPU, % of one core | 152.63 | 91.90 |
| Pure-stream hub CPU, % of one core | 120.58 | 59.47 |
| Combined hub CPU, % of one core | 141.96 | 81.08 |
| Kernel hub high-water RSS, KiB | 754244 | 569600 |
| Hub swap, KiB | 0 | 0 |

The patch passed delivery, foreign-text isolation, static-pending and latency
checks. The baseline fell behind mixed production, so these are equal offered
loads, not equal achieved throughput. Both the mixed and combined CPU gates
still failed; the 80% budget has not been raised. The additional map no-op
binary's final matched run was refused by the occupied native lock before
starting. Its CPU effect is unmeasured. Raw reports are retained as
`build/perf/text-{baseline,after}-h.json`; no incident closure is claimed.

Each phase now executes in a separate function call: a recorded v3 run reused
mixed-phase filter bindings in the pure phase. Later v4 reports have correct
phase selection. Every report includes direct owned-PID CPU ticks and kernel
RSS/high-water/swap values. Inspector samples are diagnostic only and never
substituted for the strict non-inspector acceptance run.

No live hub was restarted or reconfigured by this work.
