# Combined copied-store and active-load preparation

Status: **unexecuted preparation**. There is no runtime, memory, CPU, latency,
startup or package PASS from this change. The memory owner `t1793648028-1245355`
retains sole packaged before/after evaluation. Send the frozen source and check
scope to that owner before executing even the self-tests. Existing recovery,
load, policy and fence files are unchanged; the owner's pending fence fixes
are an explicit prerequisite.

`test/tools/real_store_active_load.ts` combines the existing private RC3 store
with 20 stand-in agents. It accepts the **existing**
`private-RC3-real-store.tar.gz`, its SHA256 and its embedded receipt's SHA256.
It fetches nothing and makes no new backup. The receipt must identify the
1,448,789-event fixture, and its copied SQLite index must hold 10,841 original
threads. Streaming extraction rejects links, traversal, duplicates, PAX
overrides and oversized members before writing them. Account profiles in the
archive are discarded. The full archive is hashed before and after the run.

The harness creates its own private temporary home, token, project, lock
directory, fake HOME and loopback child. It appends one uniquely named project,
20 synthetic threads and six safety/UI settings to the copied log. It does not
replace the catalog or history. Original log-prefix, original indexed-entry
and catalog hashes, full before/after counts, archive identity and service
identity must match. The target, home, port and pairing cannot be supplied.
Installed binary paths are refused. Reports contain hashes, numbers and fixed
labels; no copied text, reply bodies, account records, argv or private paths.

The evaluator supplies a small artifact receipt alongside the immutable
packaged service copy:

```json
{
  "schema": 1,
  "qualification": "qualified",
  "serviceSha256": "<64 lowercase hex characters>",
  "sourceHead": "<40 lowercase hex characters>",
  "sourceTree": "<40 lowercase hex characters>"
}
```

This records build/package qualification, **not** performance qualification.
The evaluator owns that provenance assertion. Both baseline and candidate
must carry their actual source/tree/service identities. An existing build's
results cannot be relabelled for newer source.

The existing preload fence is SHA-pinned and must write its installation
receipt before the first request. `BACKPLANE_COPY=1`, a minimal environment,
blocked provider/network CLIs and an isolated profile prevent restored work
and provider credentials from being used. The stand-in only accepts this
run's synthetic thread IDs. No inspector or forced GC is enabled. A raw
observer resumes from the current SQLite checkpoint and the selected view's
epoch; it refuses historical replay rather than holding the original log.

Readiness starts immediately before spawn and includes one successful hello,
an actual real-thread timeline, the synthetic own-thread timeline and the
sidebar. It must finish within 15 seconds. Each picture must be a validated
production `ThinViews` reply with the requested thread/timeline or sidebar.

Loaded measurement alternates 60-second mixed and pure-streaming windows for
at least 30 **loaded** minutes. Every agent emits 40 deltas/s of 96 ASCII bytes;
mixed mode commits message/tool cycles every 500ms. Pure mode intentionally
keeps the proper Working/pending row static while the raw delivered stream
advances. Completed content from an earlier pure phase is valid history.
Partial markers from the current pure phase, foreign thread text and foreign
notices fail. Watchdog liveness metadata has the existing bounded attribution
window; other pure-phase commits fail rather than being hidden.

Each thread needs >=32 actual produced and delivered deltas/s in every
five-second bucket, <=48/s, >=99% delivery relative to actual production,
correct byte counts, fresh receipts, correct epochs and <=2s initial, interior
and terminal gaps. Per-thread latency uses bounded integer-millisecond buckets
and nearest-rank `ceil(99*n/100)`, with an overflow bucket above 2000ms. Empty
latency is unavailable. Fractional latencies round upward conservatively.
Mixed views must show fresh completed own content >=1/s with <=2s gaps/p99;
pure views need a valid starting pending picture and accepted phase replies.
View requests remain serialized and have their own p99/count checks. Pacing
limits remain 20 own notices/s and two sidebar notices/s.

Hub CPU is the actual captured PID's utime+stime, with a stable process birth
and executable identity. Every loaded phase and the whole loaded interval
must consume nonzero CPU <=80% of one core, without normalization, subtraction
or idle padding. The <=1.5s receipt/notice flush between windows keeps all
producers and view clients running, remains in whole-load CPU/RSS, and must
fit the <=2s window seam. No idle soak follows a short active test.

RSS, HWM, swap, ticks and process birth are sampled every five seconds and
streamed to a numeric JSONL sidecar. Missing samples, holes >15s, zero memory,
PID replacement or a dead child fail. Loaded RSS **and HWM must be strictly
below decimal 1,000,000,000 bytes** and swap zero. After five loaded minutes,
at least 15 minutes/60 steady samples are required. OLS RSS slope must be
<=64MiB/hour and steady endpoint growth <=64MiB, preserving the recovery
policy's growth guard. Runs stop on failed loaded windows or a runaway 3GB
RSS boundary; such runs are failures and cannot establish a 30-minute pass.

Controller memory is bounded: one current phase plus a bounded notice grace,
two fixed producer receipt epochs, fixed 2002-bucket latency histograms,
at most 900 process samples and 60 aggregate phase summaries. Logs are drained
with a 16KiB private tail and 64MiB total limit; neither tail nor raw replies
enter artifacts. Cleanup targets only the owned hub and known synthetic
producer PIDs with unchanged birth/argv identities, then deletes the home.

Proposed checks, **not yet executed**:

```sh
node test/tools/real_store_active_policy_test.cjs
node test/tools/real_store_active_fence_test.cjs
bun test/tools/real_store_active_load.ts --self-test
```

Run only through the normal reviewed daily Boat runner, with hard memory
<=3,700,000,000 bytes and swap zero; never locally or on xlarge. These checks
use pure negative controls, mocked fence transports and one short synthetic
producer. They start no hub and open no real store. The forwarding control
intentionally rejects the old fence that loses Node HTTP option/callback
overloads; do not weaken it to accept that version. There is no full/root
proof, production compile or load run in this check scope.

The evaluator's later acceptance command supplies `--bin`, `--artifact-receipt`,
`--sha256`, `--fixture`, `--fixture-sha256`, `--fixture-receipt-sha256`,
`--fence-sha256` and a fresh `--out` under an owned build/temp directory.
`--seconds` defaults to 1800 and permits only complete mixed/pure pairs through
3600 seconds. These arguments are deliberately absent from CI/public logs
because fixture and artifact locations are private. Actual acceptance remains
a separate owner-released, normal-size Boat job.
