# Combined copied-store and active-load preparation

Status: **one-character correction released for one no-hub packet**. The
`bdd09fe794` packet failed its producer assertion; the `27e9056464` packet
failed parsing the producer diagnostic before any producer/scope controls.
The extra closing parenthesis is corrected; no other executable source is changed. There is no whole-harness, actual-hub cleanup,
memory, CPU, latency, startup or package PASS. The memory owner `t1793648028-1245355`
retains sole packaged before/after evaluation. Send the frozen source and check
scope to that owner before executing even the self-tests. Existing recovery,
load, policy and fence files are unchanged; the owner's pending fence fixes
are an explicit prerequisite.

`test/tools/real_store_active_load.ts` combines the existing private RC3 store
with 20 stand-in agents. It accepts the **existing**
`private-RC3-real-store.tar.gz`, its SHA256 and its embedded receipt's SHA256.
It fetches nothing and makes no new backup. The receipt must identify the
1,448,789-event fixture, and its copied SQLite index must hold 10,841 original
threads. The streaming parser supports Python tarfile's bounded per-member
PAX timestamp/owner metadata. It rejects path/link/size overrides, global or
sparse extensions, links, traversal, duplicates, bad checksum/padding and
truncation. The supplied account profile metadata is preserved byte-for-byte
before startup; only the known noncredential registry schema is accepted.
No credential directories are copied. Counts/identity hashes are checked
again after shutdown. The full archive is hashed before and after the run.

The harness creates its own private temporary home, token, project, lock
directory, fake HOME and loopback child. It appends one uniquely named project,
20 synthetic threads and only `telemetry.share=off` to the copied log. That
single setting prevents copied consent from enabling telemetry. It does not
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

The owner's explicitly handed-off corrected preload is pinned at SHA256
`74de3b99ba21abff2650de474fd6459d47f32778ef120d8940da8be3ed13f2d5`.
`--fence-prerequisite` supplies its original saved receipt. Both the source
and its companion test must match that receipt and its original uploaded
manifest. The harness reads the handed-off paths, or exact byte files staged
beside that receipt, and copies them into its own temporary preload directory.
Missing/mismatched files or receipts fail **before fixture extraction**.
The original job `j20261104003249-591142` passed two focused tests and owned
preload preflight, while its overall startup/memory gate **failed**; that
failure is preserved, never relabelled as acceptance. No shared file is edited.
The actual supplied package must also pass a bounded `--version` preload
receipt check before extraction, then write a fresh installation receipt
at owned hub startup. None of this claims OS-level network isolation.

`BACKPLANE_COPY=1`, a minimal environment and blocked provider/network CLIs
isolate work from the live hub and real credentials. Copied account registry
metadata and the actual verification/recovery logic remain present; auth
checks reach the stand-in and receive credential-unavailable. It only accepts this
run's synthetic thread IDs. No inspector or forced GC is enabled. Recovery,
resume, bot verification and tick settings are **not overridden**. Neither
restart.continue, fallback.on, onboarding flags nor sidebar.view are changed.
No catalog/cache/performance budgets or JSC stack override are applied.
A raw observer resumes from the current SQLite checkpoint and actual selected
view's wire epoch. Every log page must match that exact cursor/epoch, then
advance by its item count. It refuses a reset/replay rather than holding the
original log.

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
notices fail. Original-store background work is retained and reported as
aggregate event families, alongside synthetic liveness. Attribution requires
an actual canonical change referring to a known original ID/key, or an
observed info update of an initial protocol key/explicit account/helper key.
It never inspects or reports an info/reply body's account data. Only observed
metadata earns the existing -100ms/+1250ms notice attribution interval.
Unknown changes/info fail in pure mode. At least 25% of each pure window must
remain outside these intervals; a broad grace cannot hide all invalidations.
CPU, delivery and memory retain all background work and all such intervals.

Each thread needs >=32 actual produced and delivered deltas/s in every
five-second bucket, <=48/s, >=99% delivery relative to actual production,
correct byte counts, fresh receipts, correct epochs and <=2s initial, interior
and terminal gaps. Receive-time continuity and emission-owned delivery are
separate. Closing emission cohorts stay live through the real end+2000ms
deadline while the next contiguous phase already runs. Valid late tail
deliveries and view replies count toward their original cohort/request. A
missing first/last sequence, late tail or lost >1% fails. Missing cohort
deliveries also enter the timeout overflow count rather than disappearing
from latency. Observed-only latency and missing counts are reported separately.
Per-thread latency uses bounded integer-millisecond buckets
and nearest-rank `ceil(99*n/100)`, with an overflow bucket above 2000ms. Empty
latency is unavailable. Fractional latencies round upward conservatively.
Mixed views must show fresh completed own content >=1/s with <=2s gaps/p99;
pure views need a valid starting pending picture and accepted phase replies.
View requests remain serialized and have their own p99/count checks. Pacing
limits remain 20 own notices/s, two sidebar notices/s and two quiet notices/s.

Hub CPU is the actual captured PID's utime+stime, with a stable process birth
and executable identity. Every loaded phase and the whole loaded interval
must consume nonzero CPU <=80% of one core, without normalization, subtraction
or idle padding. Phase emission windows are contiguous; the closing cohort's
two-second deadline overlaps the next phase, with at most two cohorts retained.
After the last emission window, producers and clients remain active for its
two-second deadline and whole-interval CPU/RSS/delivery still count. Scheduling
overshoot is bounded to 250ms and is not a substitute for loaded duration.
No idle soak follows a short active test.

RSS, HWM, swap, ticks and process birth are sampled every five seconds and
streamed to a numeric JSONL sidecar. Missing samples, holes >15s, zero memory,
PID replacement or a dead child fail. Loaded RSS **and HWM must be strictly
below decimal 1,000,000,000 bytes** and swap zero. After five loaded minutes,
at least 15 minutes/60 steady samples are required. OLS RSS slope must be
<=64MiB/hour and steady endpoint growth <=64MiB, preserving the recovery
policy's growth guard. Runs stop on failed loaded windows or a runaway 3GB
RSS boundary; such runs are failures and cannot establish a 30-minute pass.

Controller memory is bounded: at most two emission cohorts (8192 unique
sequences/thread/cohort), two fixed producer receipt epochs, fixed 2002-bucket latency histograms,
charged original ID/key metadata <=128MiB, bounded five-second receive buckets,
at most 900 process samples and 60 aggregate phase summaries. Logs are drained
with a 16KiB private tail and 64MiB total limit; neither tail nor raw replies
enter artifacts. The iterative single-flight refresh pump retains one pending
invalidation flag and one picture/call, rather than recursive promise frames.
The revised cleanup launches the payload in a fresh capped systemd user scope.
The manager's invocation ID, cgroup inode/device and actual hub executable,
argv and birth are checked independently. CPU sampling uses that actual hub
PID, rather than the manager or a wrapper. The fresh child environment is
passed through `env -i`; only the manager receives the user bus environment.
A short shell admission barrier holds the payload until the controller has read and verified the actual kernel cap and swap limit; only then is its private gate created. The controller stays outside the kill scope. The 15-second startup clock includes this admission.

Cleanup uses descriptor-bound `cgroup.kill` and whole-subtree `populated`,
independently of whether the hub leader has already exited. These kernel
interfaces include other task children and concurrent forks; see the
[cgroup v2 interface](https://docs.kernel.org/admin-guide/cgroup-v2.html#core-interface-files)
and [systemd scope lifetime](https://www.freedesktop.org/software/systemd/man/latest/systemd.scope.html).
Every signal target is the verified owned scope, never a copied/recycled PID.
An unknown invocation, missing identity, denied operation or ambiguous
retirement permanently refuses cleanup PASS and preserves the private home.
Every nonempty observed invocation ID is compared before accepting retirement,
including an inactive unit without a control group and the missing-events
fallback. Resource uncertainty is sticky independently of ownership: verified
owned kill can still stop children, while missing/denied/malformed resource
reads, absent observations or prior resource uncertainty deny home deletion
even after the whole subtree is empty. Main and synthetic cleanup require both
the deletion verdict and healthy resource evidence before removing the home.
The home is removed only after proven whole-scope emptiness, launcher exit
and closed log streams, with each result included in `report.checks`.
Delegate/kill permissions and the revised implementation remain unqualified
until the owner releases and reviews the synthetic Boat controls.

The evaluator released one corrected Bun self-test packet; no run on the new
freeze has occurred yet. Reuse the unchanged fence PASS only after exact
pinned input/tool audit; rerun it only if a relevant input/tool changes:

```sh
REAL_ACTIVE_FENCE_PREREQUISITE=/private/staged/FENCE-PREREQUISITE.json node test/tools/real_store_active_fence_test.cjs
bun test/tools/real_store_active_load.ts --self-test
```

Run only through the normal reviewed daily Boat runner, with hard memory
<=3,700,000,000 bytes and swap zero; never locally or on xlarge. These checks
use pure negative controls, mocked fence transports, synthetic in-memory
streamed tar/gzip (including PAX, splits and truncation), a 20,000-refresh
logical retention/queue control and codec checks. The revised producer control
requires observed completed text, tool use, tool result and matching receipt
counters before switching phases, with a bounded deadline and aggregate
emitted phase spans. A separate real stand-in with only its cycle gate
suppressed must fail readiness despite fresh deltas. Sixteen explicit
producer assertions are expected. It does not substitute a longer fixed sleep
for observing the missing cycle.

Forty-nine execution-scope controls include a synthetic Node worker spawning
a child absent from the main task's children list, a late orphan after leader
exit, concurrent spawning during stop, missing-task reads, replaced invocation
and inode/device, and unknown-owner refusal. They use the same production
harness scope implementation, start two small synthetic scopes and preserve
their homes on uncertainty. Each has the same <=3.7GB/no-swap cap; no actual
hub or real store is opened. The packet must verify those scope identities,
caps, admission/pre-kill resource observations and cleanup as well as the outer workers. Resource observations are explicitly pre-kill; missing counters fail rather than becoming invented zeroes after cgroup retirement.
Twenty-one of the 49 controls mock only the manager/kernel boundary and call
the actual `LinuxScope.stop` implementation: direct inactive/not-found
replacement IDs, ENOENT/ENODEV fallback replacement IDs, valid original/erased
retirement IDs, before-kill missing/denied/malformed resource reads and absent
observations. They assert no deletion after emptiness and no later recovery
from prior uncertainty; they run no systemd command or process.
The forwarding control
intentionally rejects the old fence that loses Node HTTP option/callback
overloads; do not weaken it to accept that version. There is no full/root
proof, production compile or load run in this check scope.

The original released packet is preserved in
[`extra/hub-real-active-qualification/bdd09`](../../extra/hub-real-active-qualification/bdd09/FAILURE.json):
work `1567247`, job `j20261104073057-626225`, overall exit 1. Node receipt-pinned
fence mocks passed; Bun reported 54 policy cases, 29 streamed archive cases
and 20,000 iterative refreshes before the producer assertion at line 45.
It observed no mixed tool-use row after its fixed 650ms delay; the cause is
unknown. Producer receipt assertions and the terminal sentinel were not
reached. This partial evidence cannot qualify the producer or full harness.
All 26 staged inputs and tool hashes were unchanged. Worker peaks were
23,425,024 and 85,204,992 bytes; controller peak was 349,569,024 bytes, with
zero OOM/swap events, no remaining worker children, clean synthetic temp and
verified VM deletion. The original failed fence provenance remains failed.
Raw receipt bytes and log hashes are retained; raw logs and private overlay
receipt bodies are not published. This failure remains a failure.

The second released packet is preserved in
[`extra/hub-real-active-qualification/27e905`](../../extra/hub-real-active-qualification/27e905/FAILURE.json):
work `1576980`, job `j20261104084549-1097971`, overall exit 1. Node fence mocks
passed again, along with policy54, tar29 and pump20000/max1. Bun rejected the
extra closing parenthesis at producer diagnostic line 94. No producer16,
scope49/21 mocks or synthetic execution-scope trees ran; exit, exact groups
and terminal sentinel failed. All 30 staged inputs and tools were unchanged;
worker peaks were 23,203,840 and 72,019,968 bytes, controller peak 349,749,248
bytes, swap/OOM zero, remaining worker children zero and synthetic temp clean.
Boat verified deletion of the enclosing sandbox. Raw receipts are preserved
byte-for-byte with raw log hashes; no logs/private overlay bodies are published.
The sole permitted executable correction removes that one parenthesis.
Neither this correction release nor either partial result qualifies runtime
cleanup, packaged performance or merge. A new failure requires terminal
evaluation; it does not authorize automatic retry.

The evaluator's later acceptance command supplies `--bin`, `--artifact-receipt`,
`--sha256`, `--fixture`, `--fixture-sha256`, `--fixture-receipt-sha256`,
`--fence-sha256`, `--fence-prerequisite` and a fresh `--out` under an owned build/temp directory.
`--seconds` defaults to 1800 and permits only complete mixed/pure pairs through
3600 seconds. These arguments are deliberately absent from CI/public logs
because fixture and artifact locations are private. Actual acceptance remains
a separate owner-released, normal-size Boat job.
