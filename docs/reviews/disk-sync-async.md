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

Plan: two excluded warmups per variant, then 32 alternating paired rounds,
128 sequential real append+fsync completions per burst. There is no artificial
fsync delay. A verified shared Linux CLOCK_MONOTONIC domain requires both
probe issuance windows to lie inside the active burst. Every response body
and the entire ordered UTF-8 append file are checked. The original helper's
process-local RTT is retained alongside shared-clock observation brackets.
Report median/p95/nearest-rank p99/max, per-append modeled durable ACK and
whole-burst time, park/resume/duplicate counters, caps/OOM/swap and cleanup.
At n=32, nearest-rank p99 is the largest round observation.

These are component responses and modeled append+Disk.sync ACKs, **not actual
Hub RPC ACKs or SQLite transactions**. Real filesystem latency can vary; no
speedup or threshold is asserted before receipts exist. No startup, private
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

Execution/measurement receipts: pending. Run only through the normal approved daily Boat
runner, default sandbox, `hub-latency` lock, no reuse/setup-Bend; controller
512 MiB, worker at most 3.7e9 bytes, swap 0. `disk_sync_gate.py` verifies source
manifest/tool identities, strict case counts, exit/no-FAIL/sentinels and
resource receipts. Its public manifest must stage the unchanged template and
only the source/test dependencies. No local tests or product build/install.
