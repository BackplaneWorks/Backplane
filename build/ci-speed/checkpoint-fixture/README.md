# Durable checkpoint fixtures, one setup transaction

The only required-test change is `db(file,marker,codec)` in
`test/tools/store_checkpoint_io_test.cjs`: create the same schema, metadata and
marker rows in one real SQLite transaction; close the handle in `finally`.
The fixture is on disk with SQLite's default durability. Every original test
case, assertion and failure injection stays unchanged, as do production store
validation, prefix hashing, codec/telemetry checks, fsync, promotion, rollback
copies, WAL/SHM handling and Windows link behavior. No PRAGMA is changed.

This is a measured fixture setup improvement. It does **not** establish the
cause or elimination of an earlier 90-second timeout. It does not qualify a
full small/check/proof/build job, the release, or the two-minute PR target.

## Before and after

Five alternating before/after pairs on one normal Boat VM, pinned Bun 1.3.10:

| Real SQLite fixture setup | Before | After |
|---|---:|---:|
| Minimum | 3.939507s | 0.065813s |
| Median | 5.024011s | 0.115317s |
| Maximum | 6.783881s | 0.152323s |
| Samples, in trial order | 6.783881, 5.460604, 3.939507, 5.024011, 5.001420s | 0.065813, 0.097374, 0.152323, 0.125289, 0.115317s |

The median setup cost fell about 44-fold in this VM. This is five setup pairs,
not an end-to-end CI speedup estimate. Creation/DDL/inserts/close are timed;
state snapshots are outside those measurements. Ordering alternates to expose
some warm/cold effects; shared-host journal latency and the small sample remain
limitations. Do not compare the earlier timeout to this median as a speedup.

Complete original candidate checkpoint suite: **25/25 passed**, 5.79s wall,
198,720KiB peak command RSS. This is one complete candidate-suite observation;
the original complete suite timed out, so there is no matched full-suite median.

## Scope-bound validation

Tested source commit `e6b558e1627f27878f028f149613b160bc85b117`, tree
`791446bf839f30f2fb4db9bdc698a874c834fe7a`; baseline source
`22d5e2202204371d3ad58f052f87cd4f89501b1f`. Candidate test SHA256
`70675f2307ae9c11308d36082e25876e5a9d8e92bb41b6be753a9b30ce0e2c6e`,
baseline test `0d3e4b73172214b0c992d4eb41fe36f0fb5a287cd36b3738347f9b99a7d136f9`.
Production `storeio.js` is unchanged:
`fc27b7c1ef83da3ad3b4ed4eec12237f7e776bf2582ffd20dc2d39b7acbb5f93`.
The test prefix/suffix outside the helper are compared byte-for-byte.

Validation deliberately reuses completed receipts for these exact source/tool
bytes; later jobs do not replay the complete suite or the mutation tests, and
no JSON-declared pass is imported into a gate or cache:

- `qualification-v1/`, **j20261104063951-69873**, work1545478: all 25 candidate
  cases pass, both original and candidate helpers reject a deliberately broken
  prefix validator at the same case with the same `ERR_ASSERTION`, operator and
  message (computed DB hashes normalized). Only the prefix validator is mutated;
  hash/codec rejection assertions remain in the unchanged complete suite.
  The old/new negative controls took 37.49s/0.86s, but those are functionality
  controls, not the matched timing benchmark. This job is **failed overall**:
  its naive schema check incorrectly treated a legitimate trigger's END as a
  transaction boundary and never reached fixture parity. No wrong-state or
  file-size mismatch was observed by that failed check.
- `parity-v2/`, **j20261104064434-119474**, work1547735: actual SQLite engine
  transaction-state checks accept the schema's CREATE TRIGGER BEGIN/END. All
  five pairs have identical normalized sqlite_master definitions, every table's
  rows (including FTS shadow tables), default PRAGMAs and integrity results.
  File and sidecar names match. Each observed file set is one 229,376-byte
  `fixture.db`, no WAL/SHM; sizes are recorded as information, not a state proof.
  The actual candidate helper rolls back a mid-schema failure with zero partial
  tables and closes its real handle in `finally`. No complete suite or mutation
  test runs in this job.
- `sql-errors/`, **j20261104064752-157500**, work1549193: only the remaining
  reason-specific SQL negative controls run. An injected premature COMMIT must
  produce `AssertionError` / `ERR_ASSERTION` containing "schema committed the
  surrounding transaction"; an injected nested BEGIN must produce SQLite's
  "cannot start a transaction within a transaction". The clean schema passes.
  Actual errors are retained. This closes the earlier assert-throws-any-error
  weakness without replaying successful measurements or tests.

Default PRAGMAs match before/after: journal_mode `delete`, synchronous `2`
(FULL), page_size4096, user_version0, wal_autocheckpoint1000 and integrity_check
`ok`. No memory database, tmpfs, synchronous-off or changed pragma is used.
Physical DB byte identity is not asserted: transaction count can legitimately
change SQLite header/page bookkeeping. Logical contents and tested rejection,
promotion and rollback behavior are retained.

All these jobs use a hash- and mode-pinned six-file closure exported from the
real Git commits. Original files are immutable; deliberately broken runtime
and diagnostic copies live separately. Native `/tmp/bp-bend.lock` is held and
probed. Worker cap **3,699,998,720 B**, controller **536,870,912 B**, both swap0;
combined hard caps **4,236,870,912 B <4GiB**, zero recorded max/OOM events.
Worker peaks are 473.6–473.8MB including pinned Bun/Node setup. The suite's
198,720KiB and parity benchmark's61,244KiB are command RSS, not cgroup peaks.
Controller peak during parity was10,280,960B. Final source/reference/helper/tool
identity fences pass. No Bend binary is installed or executed.

Bun1.3.10 baseline executable SHA256:
`795b37d736a88d746b0e8bb5fcde92bf29bd0f9ac6a1dd1a950fd37440cf5822`;
Node24.13.1:
`d95de52ccb76fb2c5775bf176f29f3025e4b19352c092aad51ce5d930717359f`.
The pinned Bun source exposes the actual transaction-state accessor used by the
control: https://github.com/oven-sh/bun/blob/bun-v1.3.10/src/js/bun/sqlite.ts .
Source/tool hashes, environment, filesystem type, transport manifests and
resource receipts are retained. The approved normal daily runner digest is
`7e3f2d4ffad2d86f0a1bea580ae52e95fde9706016a236b291be98cfba98b3df`.
No size, budget, concurrency, admission or timeout guard is relaxed.

## Earlier failure and diagnostic, not a reproduced runtime cause

Baseline job **j20261104045545-1148789**, work1540307 (receipts in
`../small-guards-profile/baseline/`), profiled the literal twelve leading
non-Bend commands from frozen `local-small.py`. Commands0–4 passed in a total
1.33s. Sidebar guard5 failed3.03s on stale source-review pins. Checkpoint guard6
printed the first three cases then exceeded90s; remaining five commands did
not run because the old profiler's TimeoutExpired aborted the loop. This is an
incomplete profile, not a full CI failure diagnosis or complete twelve-command
measurement. Source/tools stayed unchanged; actual peak473,755,648B, swap0,
zero cap/OOM events. A prelaunch digest typo before this job started no VM and
cost$0.

The next unprinted case was prefix rejection, but the old output did not tell
whether the stall was fixture construction, runtime validation, cleanup or
something else. It was **not** the later picked-lines case.

Read-only trace **j20261104050613-1227814**, work1542630, completed those first
four original cases, including prefix rejection, in0.9147s. Actual SQLite/FS
calls and original store functions were forwarded with progress markers. An
owned-process sample was in D state at `jbd2_log_wait_commit` with low CPU;
schema creation took about80–103ms. This establishes journal waiting in the
traced run, not the cause of the earlier90s failure. Tracing can change timing.
The original runtime is unchanged, and no hang fix is claimed.

The stale sidebar guard is deliberately still held. At frozen22d5, twelve files
changed after review1d5f879b87: constructor renames (#902/faf11dfbbc), bounded
catalog projection changes (cae46833e1), and a private window setter extraction
(ae612f2450). The guard correctly refuses stale review reuse. It requires draft
reader review and full-output requalification before its pins may change; the
older release source's output receipt does not cover these changed inputs.
No pins, guard rules or release files change in this PR.

## Cost and cleanup

| Job | Outcome / scope | Remote wall | Credit-s / cost |
|---|---|---:|---:|
| j1148789 | incomplete baseline, preserved90s timeout | 126.66s | 134 / $0.00134 |
| j1227814 | first-four trace completed, no hang | 5.37s | 11 / $0.00011 |
| j69873 | full25 and mutation checks pass; schema-control harness fails | 49.16s | 55 / $0.00055 |
| j119474 | remaining parity/rollback/setup passes | 31.26s | 38 / $0.00038 |
| j157500 | specific SQL error controls pass | 5.37s | 13 / $0.00013 |
| Total before merge guard | | | **251 / $0.00251** |

All five owned normal sandboxes were deleted, verified. No job was duplicated,
no original90s batch replayed, and completed native checks were reused. No local
tests/compiler, live home, hub, package, installer, emitter, telemetry send or
release action was used. Full checks/proofs/builds and whole-PR timing remain
unqualified; the broader CI/build goal stays open.
