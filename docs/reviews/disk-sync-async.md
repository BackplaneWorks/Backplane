# Disk.sync asynchronous completion boundary

Draft slice 1; no merge authorization. The only production change is the
`Disk.sync` registration and its wrapper in `src/server/effects/storeio.js`.
The existing synchronous `store_sync` helper remains unchanged for direct
promotion/adoption callers. There is no loop.js, Bend, database, diagnostics,
fsync-policy or log format change.

In the async runtime, the effect parks via the existing `io_park_promise`
while real `fs.promises.open`, `FileHandle.sync` (full fsync) and `close`
complete. `finally` closes the acquired handle on fsync rejection. Completion
returns `ok` only after close; failures return `fail ...`. A missing park
bridge refuses before opening. Async failures do not fall back to blocking.
Explicit `BEND_ASYNC=false` or `BACKPLANE_JS_LOOP=sync` retains synchronous
execution. Windows still opens `r+`; POSIX opens `r`.

`Disk.sync(path) -> IO(String)` is unchanged. `Sto.committed` still waits
before `Sto.write` commits the SQLite checkpoint, and durable outputs remain
behind that path. No earlier ACK or weaker durability is introduced.

Parking frees the JS loop for sockets/other fibers. **Hub.loop still awaits
the current Hub.handle before accepting its next queued view or RPC.** This
slice does not implement group commit, speculative state, concurrent actor
handling, or a complete latency/memory fix.

The boundary suite has 19 cases: controlled open/fsync/close ordering,
exactly-once completion, immediate/delayed duplicate negative controls,
success and each failure stage, Windows/POSIX flags, both explicit sync-mode
selectors, missing bridge, synchronous error paths, unchanged internal
helper, and 32 real UTF-8 durable prefixes plus missing-file/descriptor checks.
It uses the exact extracted production park helper with a minimal component
continuation scheduler; it does not qualify Bend compilation/marshalling.
The existing 25 real SQLite/checkpoint cases also guard direct helper callers.

The Boat component A/B reuses byte-identical `server()` and `get()` helpers
from lane A's PR925 `test/perf/hub-latency/effect_transfer.ts` (SHA256
`dff6b994065190f3cdeaed62781e4ad296dbb4c7f6361a52f6db6942b7808633`).
The public template is staged unchanged; no PR925 file is edited or forked.
Its production `sock_send_static` and current-main `bp_idle` run in the same
Bun child/event loop as the fsync burst, identical in both variants. The
independent controller uses the same `get()` to measure `/hello` and a
262,144-byte static socket response. Only its two extracted TS helpers are
transpiled as part of this Bun component test; no app or Bend build runs.

Method: two excluded warmups per variant, then 32 alternating paired rounds,
128 sequential real append+fsync completions per burst. There is no artificial
fsync delay. A verified shared Linux CLOCK_MONOTONIC domain requires both
probe issuance windows to lie inside the active burst. Every response body
and the entire ordered UTF-8 append file are checked. The request helper is
unchanged; reported RTTs use the shared-clock brackets around its completion.
Its redundant process-local `ms` result is not persisted or claimed as a
second measurement.
Report median/p95/nearest-rank p99/max, per-append modeled durable ACK and
whole-burst time, park/resume/duplicate counters, caps/OOM/swap and cleanup.
At n=32, nearest-rank p99 is the largest round observation.

These are component responses and modeled append+Disk.sync ACKs, **not actual
Hub RPC ACKs or SQLite transactions**. Real filesystem latency can vary. No startup, private
fixture, provider, whole-hub, packaged memory/CPU/p99 or installed acceptance
is covered. Lane A independently reviews and reruns the frozen measurement.

First attempt `j20261104102045-1651727` / work `1584003` failed at source
identity admission: the controller incorrectly equated Boat's new staged
snapshot commit with the submitted source commit. No tool installation,
boundary/checkpoint test or measurement ran. Original exit 1, raw receipts
and verified VM deletion are preserved in
`extra/hub-disk-sync-qualification/fb25895d-identity-fail/`. The correction
checks the runner's source-SHA commit subject, exact staged file set and
every pinned file hash, and records the distinct snapshot OID/tree.
Production and measurement code are unchanged by that transport correction.

Second attempt `j20261104102459-1678325` / work `1584384` passed corrected
snapshot admission and failed controlled boundary case 3: done=1, result=ok,
resumes=1, but the park counter remained 0. Only the two duplicate-negative
controls passed; no checkpoint/A/B or real-fsync boundary case ran. The
host-assigned VM property wrapper was bypassed by the effect's lexical call.
The prepared instrumentation correction defines the wrapper and original
byte-identical park helper in the same VM lexical environment, with named
counter assertions. Production behavior and measurement method are unchanged.
Raw failure/resource/tool/source receipts are retained in
`extra/hub-disk-sync-qualification/962c7255-counter-fail/`. Worker/controller
peaks were 22,941,696/159,805,440 bytes; cap/swap/OOM checks passed and the
enclosing VM was deleted. Corrected execution awaits lane A's static review.

## Writer component result

`j20261104103008-1718048` / managed work `1585116` passed on exact source
`0139fd6d12910082e7be7f930cb5a12a0eec3ead`, tree
`c285018e43b807e2535f70fb382e3aeefd37cfbc`: **19 boundary + 25 checkpoint
cases and all 32 A/B pairs**, strict exit 0/counts/sentinels/no FAIL.
Raw samples and source/tool/kernel receipts are in
[`extra/hub-disk-sync-qualification/0139fd6d/`](../../extra/hub-disk-sync-qualification/0139fd6d/).

| Component metric, ms | Before median | After median | Before p99 | After p99 | n per variant |
|---|---:|---:|---:|---:|---:|
| `/hello` during burst | 2454.074 | 0.294 | 4400.849 | 1.104 | 32 |
| 256 KiB static socket response during burst | 2458.741 | 4.932 | 4405.255 | 9.700 | 32 |
| Modeled append + Disk.sync ACK | 16.373 | 16.356 | 60.596 | 62.009 | 4096 |
| Entire 128-commit burst | 2452.433 | 2473.958 | 4399.592 | 4246.868 | 32 |

The durability wait/ACK did **not** materially improve: its p99 was slightly
worse and median burst time was also slightly worse. The result demonstrates
removal of the component's loop freeze. Both response types completed inside
the burst in 0/32 before rounds and 32/32 after rounds. Every probe's issuance
bracket was inside the real burst; no synthetic fsync delay was inserted.
The 4096 ACK observations are pooled from 128 sequential operations in each
of 32 rounds, not 4096 independent process trials. At n=32 the response p99
is the maximum observed round. This is one paired run on one Boat sandbox,
not a whole-hub before/after or an explanation of storage-device latency.

All full-file UTF-8 append bytes/order and response bytes matched. Including
excluded warmups, the adapter recorded 4352 parks, 4352 resumes, 8704 total
sync/async completions, zero duplicates and zero pending continuations.
The child exited cleanly, all component sockets closed, owned temporary
files/descriptors were cleaned, and the runner verified enclosing VM deletion.
Actual kernel memory.max was 3,699,998,720 bytes for worker/child and
536,870,912 bytes for the controller. Shared worker cgroup high-water across
all three serial stages was 202,133,504 bytes, set during checkpoint testing;
controller peak was 159,784,960 bytes. These are not isolated benchmark-child
RSS. Swap current/peak/max and OOM counters were zero.
Tool: pinned Bun 1.3.10, executable SHA256
`795b37d736a88d746b0e8bb5fcde92bf29bd0f9ac6a1dd1a950fd37440cf5822`.
The Windows/macOS handle-policy cases are mocks; native execution here is
Linux only. Neither full Bend marshalling nor packaged service execution is
qualified by this component.

The unchanged sender SHA is
`a4ff856f2c97f5af58741403eca2b36bf20ad55d56f7dd80dc073ac09ec2d83f`;
both variants use the unchanged main `bp_idle` SHA
`cd8a5d6c5d31df1503227983f5d17813d355c4b968358cf735c29a911579bf32`.
PR925's scheduler candidate is not part of either variant.
Input manifest SHA:
`466aa1599802d011765900777d87fb85a3b24e82f7ba90edd9a9bb02d9c72bbd`.
Raw results SHA:
`9f898ae72cfffd307aab0a59e7e24549f6128132464911d67f2db904273eb94a`.

## Independent lane A result and cost

Lane A **APPROVES the narrow Disk.sync effect/component slice** at tested
`0139fd6d`, verifying `74440d28` preserves every tested `src` and `test/tools`
byte. Its separate detached-checkout replay passed all 19 boundary cases,
25 checkpoint cases and 32 A/B pairs (same two excluded warmups/variant) in
normal/default Boat `j20261104103713-1761208`, work `1585651`.
The readable final verdict and independent raw receipts are preserved in
[`extra/hub-disk-sync-qualification/lane-a-0139fd6d/`](../../extra/hub-disk-sync-qualification/lane-a-0139fd6d/).

| Independent component metric, ms | Before median | After median | Before p99 | After p99 | n per variant |
|---|---:|---:|---:|---:|---:|
| `/hello` during burst | 76.590 | 0.566 | 122.182 | 2.027 | 32 |
| 256 KiB static socket response during burst | 81.221 | 4.657 | 126.513 | 7.787 | 32 |
| Modeled append + Disk.sync ACK | 0.487 | 0.568 | 1.847 | 2.183 | 4096 |
| Entire 128-commit burst | 74.543 | 81.960 | 120.601 | 130.848 | 32 |

This second run exposes overhead on much shorter observed storage waits:
modeled ACK median **+0.081 ms / 16.6%**, burst median **+7.417 ms / 9.95%**;
28/32 paired bursts were slower after, with median paired difference
10.196 ms. ACK maximum rose from 4.632 to 20.277 ms. Preserve these costs
alongside the first run: this is improved socket responsiveness, not faster
durability or storage throughput. **Do not pool the two runs or publish a
universal speedup ratio.** Underlying storage/device/host-load cause remains
unestablished. The writer VM was already deleted before this replay.

Both response types again completed inside the burst in 0/32 before and
32/32 after rounds. Lane A independently verified raw quantiles, issuance
windows, input/tool hashes, exact bytes/order and strict terminal counters;
the writer's receipt-only integration audit also recomputed the saved rows.
Parks=resumes=4352, completions=8704 including warmups, duplicates/pending=0;
child exit, temporary-file/socket cleanup and enclosing VM deletion verified.
Worker cap 3,699,998,720 bytes, controller 536,870,912 bytes, swap/OOM zero;
shared worker cgroup high-water 195,698,688 bytes (checkpoint stage), controller
159,698,944 bytes. These are not benchmark-child RSS. Same pinned Bun 1.3.10,
public manifest, template, production sender, main scheduler and park helper.
Independent raw RESULTS SHA:
`ca9a516794fe01f6d424c8b5af8082504b07d8d5ef02e9c5c14baf87110dd456`.

Approval establishes this scoped effect/component result only. It does not
authorize merge or qualify Hub actor/RPC/SQLite ACKs, startup, private-store,
native Windows/macOS, whole-hub or packaged acceptance. Hub.loop's current
handler and its queued messages remain serialized. No new executable edit or
run followed this final review.

Reproduction remains Boat-only through the normal approved daily runner,
default sandbox, `hub-latency` lock, no reuse/setup-Bend; controller 512 MiB,
worker/child at most 3.7e9 bytes, swap 0. `disk_sync_gate.py --inputs PATH
--sha256 SHA` verifies the staged manifest, tool identities, strict case
counts, exit/no-FAIL/sentinels and resource receipts. Stage exactly the public
dependencies in the original manifest plus its own externally pinned bytes;
the unchanged PR925 template is a public input, not a fork or edited file.
Lane A independently reran the frozen source as recorded above. No local tests, product
build/install, new writer packet, rank-2 work or merge is authorized here.
