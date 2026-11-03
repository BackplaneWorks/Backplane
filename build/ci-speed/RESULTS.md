# CI speed measurements

Measured 2026-11-02, source `7b0e95833`, before CI changes. Target: approximately
120 s PR feedback and minute-scale builds, without removing any law, proof or
required assertion. This target is **not achieved**.

## Before / after

| Change / workload | Before | After | Evidence / limits |
|---|---:|---:|---|
| Artifact compression, installed 118,042,489-byte service, DEFLATE 6 → 1 | 2.543 s; 43,834,514 B | 0.934 s; 47,954,641 B | One local pair using Python zipfile/zlib; 63% less compression CPU, 9.4% larger archive. Not an Actions wall-time measurement. |
| Native CI artifact selection, all targets → own target (merged PR737) | 840,655,549 B per native download; latest Mac x64 102 s | Not yet measured on Actions | Workflow changed; native smoke still requires its target's binary and every helper. |
| Lean source transfer for saved two-slice pilot | 146 s push +37 s setup; 2,748 files /77 MiB | **3 s push +1 s setup; 3 files /272 KiB** | One pair; exact saved pilot inputs and actual Bend/library identities verified; default4CPU/8GB Boat. |
| Local combined gate (existing owner receipt, PR630) | 932.760 s | Not yet measured | Check/tests cached; proof 417.870 s, build/smoke 505.442 s. Not a cold complete gate. |
| Lock regression E2E (existing build-times owner) | 151.956 s | 123.941 s | One controlled pair, original assertions retained; already on main. Not a whole-gate result. |

## Actions audit

150 most recent runs, 1,154 jobs, 2026-10-22 06:25 UTC through 2026-11-02
12:22 UTC. Includes release/bootstrap/desktop/mobile workflows and failed,
cancelled and skipped jobs: 631 success, 96 failure, 341 cancelled, 86 skipped.
[Aggregates](AUDIT.json); full raw API replies remain in `audit/raw/` locally.
Reproduce: `python3 scripts/ci-audit.py --limit 150` (authenticated `gh`).

The API's job `created_at` → `started_at` measures queue time. Run creation →
job creation is reported separately as dependency/scheduling time; counting
that as machine wait would misdiagnose this pipeline. Step setup classification
is conservative: install commands embedded in build/test steps remain running.
Artifact transfers are a separate category. Unreported timestamps stay unknown.
Skipped/cancelled jobs are retained in totals, not used as successful benchmarks.

Across all jobs: queue p50 **3 s**, p95 **5 s**; setup p50 **5 s**, p95 **10 s**.
The observed queue already beats the approximately 8 s target; execution is the
bottleneck. Successful nightly jobs have these wall-time medians:

| Job | Samples | Median |
|---|---:|---:|
| Tests 1/4 | 6 | 2,015.5 s |
| Tests 2/4 | 6 | 2,049 s |
| Tests 3/4 | 6 | 2,217 s |
| Tests 4/4 | 5 | 2,270 s |
| Prove laws | 9 | 379 s |
| Type-check | 10 | 145 s |
| JS service | 8 | 295.5 s |

Latest nightly [41444167520](https://github.com/BackplaneWorks/Backplane/actions/runs/41444167520):
proof 382 s, type-check 256 s, test execution 1,887–3,010 s. JS service builds
203 s, web build 103 s, upload 58 s. Native artifact downloads: Linux arm64
18 s, Windows 28 s, Mac arm64 67 s, Mac x64 102 s. Every native runner downloads
the same 802 MiB all-target artifact; its own smoke takes only 2–3 s.

The GitHub workflow is nightly/manual/release validation, **not currently the
PR merge gate**. The PR loop uses `scripts/local-ci.py`, which runs small guards,
check, proof, build/smoke and required tests serially. Both `check.sh` and
`test.sh` force one compiler worker, overriding workflow worker requests. This
is part of the memory policy; changing it requires measured per-worker budgets.

## Local and Boat audit

22 retained local CI receipts from five worktrees: 15 passed, 5 failed, 2
incomplete. Successful non-reused phase medians: small guards 17.589 s (11),
check 92.512 s (10), proof 391.820 s (8), build/smoke 461.983 s (7), required
eight tests 81.924 s (8). Historical receipts are not controlled comparisons.
Detailed snapshot: `audit/local.json`.

150 most recent completed Boat provenance records, 2026-10-24 through
2026-11-02: 56 success, 74 failure, 12 timeout, 6 refusal, 2 exit132. These are
all workloads, not 150 whole CI runs. For 104 new sandboxes, creation p50 **2 s**,
p95 **4 s**; transfer/setup p50 **36.5 s**, p95 **151 s**. For 138 timing-bearing
runs, execution p50 **50.565 s**, p95 **689.12 s**. Peak process RSS p95
15,226,692 KiB, close to/exceeding a large sandbox's usable RAM. Existing full
proof examples reach 13.6–15.6 GiB. Detailed snapshot: `audit/boat-summary.json`.
The completed-job ledger charges total **$1.75325** for this sample; idle warm
charges, unresolved jobs, subscriptions and other threads are not included.

## Cache and runner findings

The repository had 15 Actions caches (126,775,411 B): only 3 on main, 9 on
`backplane/nightly-ci`, 2 on a packaging branch, 1 on a probe branch; none
listed on PR refs. Existing proof cache keys already include the entire
transitive PROOF input closure and compiler, but are monolithic. There is no
per-file proof cache. Emit caching also exists. Do not bypass proof validation
using a broad restore key. [GitHub's cache scope rules](https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching)
allow PRs to restore base/default-branch caches; PR-created caches do not seed
main or unrelated PRs. Main needs to produce caches for current inputs.

Repo runner API: zero self-hosted runners. Organization runners API: HTTP403
(requires org runner permission). Hosted-runners API: HTTP404 (not supported
for this organization). Existing workflows all use GitHub default runners.

[Boat](https://boat.dev/) has Linux only: current account offers up to large,
8 vCPU / nominal16 GB, at $0.00002/s ($0.0012/min); default4 vCPU/8 GB at
$0.00001/s. Its startup meets the queue target and warm sandboxes avoid repeated
setup. A large machine has insufficient safe headroom for today's largest
monolithic proof; the current guard requires18GiB free. Current account hourly
start guard can refuse at48/60 starts. Reusing warm machines or a pool matters
more than adding disposable shards. A full Actions runner also needs lifecycle,
registration and workflow-dispatch plumbing; Boat is not registered as one yet.

[Blacksmith's current list prices](https://www.blacksmith.sh/pricing): base Linux
x64 $0.004/min, Linux ARM $0.0025/min, Windows $0.008/min, Mac M4 $0.08/min;
3,000 free min/month advertised. These are base-size prices, not quotes for
our larger machines. No Backplane benchmark or installation yet. Need actual
large-memory sizes, Intel Mac coverage and organization app authorization before
routing jobs there. Do not replace working runner labels with unregistered ones.

## Ownership / verification

Build-times owner `t1793604413-1188217`: memory guard and Boat concurrency.
This thread: audit, CI workflows/caches/affected selection/pre-push. Installer
`t1792701529-415584` receives app-affecting changes. One heavy local job under
`bend`; remote work must meet its host memory/headroom checks. No Railway.

Audit tests verify queue/dependency separation and missing timestamps. Further
speedups need paired runs with identical inputs, compiler/environment hashes,
required assertions, invalidation/mutation tests and actual billable usage.

## Resumed tooling pass (2026-11-02)

The third-pass instruction omits broad proof/build/installer runs for these
CI-only corrections. Existing pilot receipts are retained; no new whole-gate
pass is claimed. Canonical laws/proofs and application code are unchanged.

Boat job `j20261102202902-993330`, source HEAD `7b0e95833` plus the recorded
prototype manifest: usage slice (1 law, 1 proof; 15 imported files) **31.12 s,
274,608 KiB**; scene slice (1 law, 15 proof/helper blocks; 19 imported files)
**15.05 s, 198,596 KiB**. These are **0.19–0.26 GiB**, not evidence that all
3,058 canonical laws are covered. Both passed; a false-theorem mutation was
rejected by the same verdict handler. Raw receipts remain at
`~/.local/state/backplane/boat/jobs/j20261102202902-993330/artifacts/metrics/`.

That job transferred 2,748 files / 77 MiB: startup3 s, **push146 s**, setup37 s,
command52.69 s. Ledger cost was250credit-s ($0.00250), then a further1500credit-s
($0.01500) for the warm reservation until automatic stop: **$0.01750 total**.
New probes must send a minimal explicit source closure, verify bytes and pinned
tool identity, and avoid paying to retain idle sandboxes by default.

Preserved four-file `bp-fastci` follow-up from owner335450 is being reconciled
before overlapping implementation. Its old `bp-coord/fast-hub` path is absent
on this machine; no replacement gate has been written or launched.

## Lean source transport, merged tooling candidate

New `scripts/ci-packet.py` sends the explicit transitive Bend source closure,
required runner/effect/data extras and pinned setup metadata in one deterministic,
content-addressed gzip archive. It uploads just that archive, the verifier and
setup pin using the existing Boat runner's exclusion interface. It retains
Boat's budget/admission/orphan/cleanup behavior; no runner or settings changed.
Archive cache hits are byte/mode verified; a corrupt cache is rebuilt. No proof
or test pass is cached by this tool. Missing dynamic dependencies must be added
explicitly; this is not an affected-test selector.

Probe `j20261102214147-1677271` / work1283095 used source HEAD `dc4052def` with
**the exact saved7b pilot proof/statement/import bytes**, bound by snapshot
hashes. Forty declared source inputs total827,451 B. Transport manifest
`1f30d50eed61d420f37dfc04e874dcf06601ab5a5b43fc9a34f93b5e456a36a5`,
archive `f4dae6e34f4ded64a42e06b6b6b6219c69b20fd459f28ebc95c0eca919fa54c3`,
compiler/library identity `690c66727e08f7b63d2ad681e590140a6913bb5bad2e2070a2a5986abfbb885e`.
Bend2.0.36 release archive is checksum-pinned; actual executable and bundled and
named-library file hashes match before/after the command. Bootstrap payload
3 files/272KiB: startup3 s, **push3 s, setup1 s**. Command wall1.47 s, peak
308,484KiB; receipt's own inner wall1.397 s. Usage proof0.58 s/308,484KiB,
scene proof0.34 s/256,268KiB. False theorem rejected again; required inputs
unchanged. This is one before/after pair on separate default-size sandboxes,
not a controlled estimate of compiler speed or a claim of full-law coverage.
Earlier **198,596–274,608KiB (0.19–0.26GiB)** evidence remains valid for its
original pilot; the new run peaks0.29GiB. The large command-time difference has
not been causally attributed to transport alone.

The sandbox was deleted and verified absent immediately. Recorded charge:
**8credit-s = $0.00008**, including its lifetime; no idle retention. Compare
original pilot $0.00250 active lifetime +$0.01500 warm tail. Only one new lean
probe was launched; no broad proofs, app builds or installer work were repeated.

[Saved benchmark receipt](lean-transport/result.json), metrics and negative
control logs are retained beside it. Measured verifier SHA256:
`0f71ef48d23c9015e4a621238f330739ad8ead4a0f8d53f55de7d92f0c1c1c07`.
The measured source can be reproduced from the final verifier using the saved
`lean-transport/ci-packet-measured.patch`; `candidate-identity.json` binds both
hashes. The final candidate additionally preserves exact ordinary permission bits
instead of only executable/non-executable, with a separate regression; this
small hardening was not rebenchmarked remotely. Sixteen focused tests pass
(0.338 s), including source/compiler/library/manifest/mode invalidation,
corrupt archive cache repair, symlink/traversal/duplicate rejection, ignored
source detection, private-file filtering, failure exit propagation and no
stale successful receipt after input mutation.

## Historical bp-fastci reconciliation

Owner335450 recovered a transcript, not a ready patch. The original
`/home/h/Documents/i2cjak/bp-fastci/` worktree and
`/tmp/ci-helper-recovery-a498.patch` backup are absent. Historical base
`15067058f3c3496668911f8101d96a4044bad683` is not an ancestor of current main.
The four uncommitted follow-up files were `scripts/ci-fast.py` (runner-bound
cache reuse), `scripts/ci-runner-id.sh` (Bend/Bun binary hashes),
`scripts/boat-job.sh` (slot identity, modes, symlink fallback and inventory),
and `test/tools/boat_job_test.sh` (slot regressions). Recorded patch digest:
`66d64a1ce2b55463fc029db2fe400a3a713eb257e1642445b2453b0e8f68e265`;
112 insertions/46 deletions. Three recorded blob prefixes cannot be resolved.

Historical receipts: ci_fast43/43 and local-ci25/25 passed **before** the last
slot repairs; final Boat fake run was interrupted (exit143). There was no
completed cold/warm benchmark or passing receipt for the final four-file state.
The surviving transcript was independently read; its head-limited diff is
retained in [the partial recovery](bp-fastci-recovered-diff.partial.txt). The
base non-ancestry was checked against origin/main. Current main lacks ci-fast.py/ci-runner-id.sh; its boat-job pushes the full
filtered tree. Therefore this is reference material for deliberate porting,
not a change to cherry-pick or certify from those older passes. The transport
correction above reuses current bend-closure.py and budgeted Boat plumbing;
exact byte/tool/mode identity and inventory checks were independently tested.
No old pass-cache, named-slot or symlink-fallback implementation was restored.

Surviving handoff source: Codex recovery transcript
`~/.codex/sessions/2026/10/21/rollout-2026-10-21T14-43-08-01a15fc6-39d2-7413-876e-c23b00b027ce.jsonl`
lines505/512; original helper receipt in Claude subagent-a49849459aaa2f2d1.jsonl
line932 (paths supplied by owner335450). These are historical evidence paths,
not shipped artifacts. Installer415584 and design1131486 remained untouched.

## Affected-test planning (opt-in, no gate change)

`scripts/ci-select.py` plans from the PR merge-base through current tracked,
dirty and untracked inputs. It preserves the required eight tests, includes new
tests, follows every transitive Bend import, and always retains roots with
opaque drivers/custom effects or Base operations that can reach undeclared
inputs. Unknown non-Bend changes/imports, missing files, symlinked dependencies
and unreviewed compiler/resources select the full set. `.ci-packet/` transport
changes retain opaque tests without pretending the archive affects pure code.
The effect model is bound to the actual official Bend2.0.36 executable and all
66 bundled resources plus Base bytes; a custom binary, changed sidecar or named
library environment selects all. Check/proof/build/other-language gates are
outside the selector's scope and are not skipped.

Fifteen focused tests pass in0.073s. An independent 16-node dependency fixture
checks every changed node against its known dependents and deliberately removes
an affected test to require rejection. Other controls cover later import/body
edits, new-test inventory drift, dirty/deleted/renamed/untracked inputs, opaque
foreign behavior, changed compiler/sidecar bytes and stale output removal on
failure. A Git-bound plan is recomputed against the current diff before being
accepted. No pass records are made or reused by this planner.

| Simulated single-file change | Full catalog before | Planned tests after | Skipped | Plan+revalidation median |
|---|---:|---:|---:|---:|
| src/core/cli.bend | 557 | 221 | 336 | 1.066s |
| src/core/kserve.bend | 557 | 409 | 148 | 1.048s |
| src/core/qr.bend | 557 | 285 | 272 | 1.053s |

[Three-sample API planning benchmark](affected-plan-benchmark.json) retains its
measured source hash; final source revalidation produced the same counts. The
[final complete CLI run](affected-cli-metrics.json), including extra compiler
resource pinning, diff and inventory checks, took **1.17s /28,984KiB**. It
correctly selected all557 tests for the current tooling-only dirty diff. These
are planning measurements, **not test execution or CI wall-time reductions**.
No Boat credit was spent. The existing local gate already runs only eight
required tests; this planner prepares a future complete affected-test gate and
does not claim a speedup over that eight-test gate. Local/nightly behavior stays
unchanged until integration and actual full coverage/performance validation.

## Remaining original goal scope

| Requirement | Verified current state |
|---|---|
| Audit150 CI runs plus local/Boat | Done; aggregates and source receipts above. |
| Every job on fast runners, queue~8s, cost | Incomplete. Existing queue3s median; defaults still in use. Org runner authorization unavailable; no live auth/settings changed. |
| Fast lint/checkers; per-file proof profile/cache | Partial: pinned Oxlint/TypeScript7 commands and measured controls below; global lint debt/type coverage and production proof splitting/pass-cache remain. |
| Shared/reset test workers, core-aware shards | Incomplete; no unvalidated state sharing enabled. |
| Cache setup/outputs; blobless clones | Partial: verified lean closure transport/cache landed. ci.yml now uses full-history blobless checkouts (fixture below); other release/mobile workflows and broader dependency caches remain. |
| Parallel independent gates, split slow job | Incomplete; current local controller remains serial. |
| Affected PRs, full nightly, mutation guard | Partial: opt-in planner/negative controls; existing nightly is full, but no new PR integration or green-nightly claim. |
| Actual pre-push hook under10s | Incomplete; accepted UI3s/total9s contract, waiting for measured UI command; no placeholder hook/settings changed. |
| Incremental low-memory local/deploy builds, auto-offload | Existing owner1188217 retains this lane. No duplicate broad proof/build or installer work in this pass. |

The approximately120s complete PR gate and minute-scale complete build targets
remain unproven. Preserve these requirements; do not redefine success around
the tooling already delivered.

## Pinned fast JavaScript/TypeScript tooling

New isolated worktree `ci-speed-next-1261555` at source998f536bd; the original
367b checkout had unrelated uncommitted app changes and was preserved. New
`tools/ci` commands pin Oxlint1.90.0, Oxfmt0.75.0, TypeScript7.0.2 and Bun
types1.4.3 with an exact lock. Installed Bun1.3.10 has no type-check command in
its help; the measured checker is native TypeScript7. The existing step2glb
strict tsconfig needed `allowImportingTsExtensions` for its already-used Bun
`.ts` imports (both checkers initially reportedTS5097). Adding that syntax
option preserves strict/noUncheckedIndexedAccess/noEmit and makes both checkers
pass. No executable or application build was performed.

| Matched workload / largest process RSS | Before | After |
|---|---:|---:|
|337 JavaScript files, same eight correctness checks | ESLint10.14.0: 2.245s /482,936KiB | Oxlint1.90.0 + compatibility rule: **0.329s /174,136KiB** |
|Existing strict step2glb project and imported platform helper | TypeScript6.0.2:0.375s /236,604KiB | TypeScript7.0.2: **0.051s /75,876KiB** |

These are medians of five fresh processes per command on the same warm local
filesystem/dependency cache, Node24.13.1. Approximately6.8x faster matched lint,
7.4x faster configured type-check; peak RSS is the largest descendant process,
not aggregate concurrent memory. Tool/package locks, source/config hashes and
actual runtime/native implementation hashes are in
[the benchmark receipt](fast-tools/benchmark.json). The existing source has no
ESLint/Oxlint gate: the before column is a measured comparison baseline, not
prior CI time. No whole-repository type coverage or complete hook timing is
claimed. The configuration change is checker syntax support, not a weaker
strictness setting.

A negative control found a real native-linter coverage gap: CLI
`-D no-dupe-args` enabled zero native rules for a sloppy CommonJS duplicate
parameter and returned0; specifying that unsupported rule in JSON correctly
failed configuration. A pinned local JS plugin now preserves the duplicate
binding check. The **reported lint time includes that plugin**, rather than
claiming the faster seven-rule result was equivalent. Both matched profiles
reject all eight deliberately bad cases; both compilers rejectTS2322 in the
same deliberately wrong project fixture. Production tool tests additionally
check nested scopes, binding patterns, valid strict source and format failures.
[Compatibility controls](fast-tools/compatibility-controls.json) retain the
native CLI/config distinction. Oxlint's JS plugin API is alpha; its exact
version and these controls must remain pinned during upgrades.

Full Oxlint correctness+suspicious categories run as errors with warnings fatal:
**exit1, 1,513 existing diagnostics, 0.343s** across the337-file source corpus.
Oxfmt check is also **exit1**, with existing formatting differences in334 files
(0.050s). These failures are retained as debt, not waived or presented as a
passing global gate; no application source was reformatted. New tool files
pass their own strict lint, formatting and actual negative/positive tool tests.
Only the existing configured TypeScript project is covered; other TypeScript
sources require coverage before repository-wide integration. No default gate,
Git hook, live settings/auth, telemetry send, installer/design or native-transform
change was made. No Boat credit was spent on this narrow local tooling pass;
maximum observed individual tool RSS was under0.5GiB.

Reproduce with pinned `tools/ci` install and
`uv run --no-project python scripts/ci-tool-bench.py --prepare`.
Comparison install scripts are disabled; the saved baseline lock is reused
frozen when available. The benchmark exercises real tool binaries and failures;
a failed invocation produces no passing benchmark receipt.

## Accepted UI/pre-push integration contract

UI validation owns **3s inclusive** (startup/cache validation/static/simulator),
inside a **9s total pre-push deadline**. Cold/unknown/stale/timeout is exit3,
validation failure1, every required check passed0. The integration layer supplies
an exact pushed snapshot, immutable base/head IDs and NUL-delimited changed
paths including deletions and both rename ends. Report schema/head/status/
elapsed/checks/diagnostics/cache identity and explicit outcomes for all four
approaches. Static route/handler coverage always runs; simulator reuse binds
complete closure+specification+actual runtime/tool identity. No heavy job or
lock wait starts from the hook. UI owner accepted this contract and reports the
latest static inventory of 306 literal bindings, 37 computed bindings and 31 URL
expressions. Ten cheap immutable-export/cache/timeout controls passed in 1.07s.
The computed bindings and URL expressions remain unresolved: static and overall
status are incomplete/exit 3, even with a valid simulator receipt. Full
qualification is still pending. Computed/unmodelled coverage must remain explicit or
incomplete. No placeholder hook has been installed.

## Full-history blobless CI checkouts

All nine `ci.yml` checkout steps now request `filter: blob:none` and
`fetch-depth: 0`, including the native smoke job's unchanged sparse paths.
Nightly/manual/release callers of this reusable CI inherit it; separate release
and mobile workflow checkout settings are outside this change. No job, test,
proof, runner label, permission or publish step changed.

The [self-validating probe](../../scripts/ci-checkout-probe.py) creates a tiny
disposable filter-enabled Git server and transfers over `file://`, avoiding
local clone hardlinks. Git2.53.0, one sample per mode, warm local filesystem:

| Change / fixture | Before | After |
|---|---:|---:|
|Default shallow -> full-history blobless |15.45ms /263,179 packed bytes /1 commit |20.33ms /264,376 packed bytes /7 commits |
|Native sparse shallow -> sparse full-history blobless |11.63ms /977 packed bytes /1 commit |14.41ms /2,142 packed bytes /7 commits |
|Full unfiltered reference -> full-history blobless |75.62ms /1,837,753 packed bytes |20.33ms /264,376 packed bytes |

Both new modes found the exact cross-branch merge-base; both shallow controls
could not. Current head file bytes and executable modes matched exactly; the
sparse job excluded its existing nonselected asset directory. Historical
obsolete blobs remained missing in both full-history filtered clones. The
probe exits on any semantic mismatch and removes a stale receipt before work.
[Raw receipt](checkout-probe.json) binds the exact probe hash and fixture head.

This fulfills the full-history/filter requirement for the reusable CI. It is
**not an observed checkout speedup over shallow clones**: this fixture added
about 5ms (normal) /3ms (sparse), while preserving history with very little
extra object storage. The audit's real checkout median was already about3s;
no new Actions run was dispatched, and GitHub network/all-ref history cost
remains unmeasured. Packed bytes are local object storage, not billed network
bytes. No Boat credit or paid runner time was spent. No broad proof/build was
repeated, per the user's current tooling-only constraints.

## Restore dispatch guards after heavy-guard integration

The dispatch unit fixture copied `test.sh` but omitted its newly sourced
`heavy-guard.sh`. All invocations exited before fake Bend/Bun ran: four tests
failed and four negative tests accidentally passed on that setup error. The
fixture now supplies an explicit no-op guard only inside its disposable fake
repository. Real admission remains covered by `heavy_guard_test.py`; production
lock, memory floors, heap settings and worker count are unchanged. Negative
cases now require exit1 plus their actual failure/crash/empty-output diagnostic,
so missing fixture setup cannot satisfy them.

| Change | Before | After |
|---|---|---|
|Fake-tool dispatch unit suite |4 failures /8 tests;4 setup-error false positives;0.035s |8/8 pass;1.094s; intended negative diagnostics verified |

[Exact-source receipt](test-dispatch-fixture.json). This is a guard correctness
repair, not a speedup; no actual Bend/app execution, broad proof/build, Boat
credit or installer work was used. Inspection also confirmed `test.sh` forces
serial execution despite CI's `TEST_JOBS=2`. Build owner1188217 confirmed that
two tests can each reach about8.2GiB RSS, so blindly restoring parallelism on
a16GiB runner would be unsafe. Production scheduling stays serial pending
trusted capacity/admission and representative measurements; no twofold/nightly
speed claim is made.

## Refresh the slow-test scheduling profile from existing nightly logs

Reused [nightly 41444167520](https://github.com/BackplaneWorks/Backplane/actions/runs/41444167520)
at source 8ec64e3d40c9dd1fb7b72a1374a05220f1a46be9. Its four downloaded test
logs contain every one of 514 positive-suite programs exactly once: 513 timed
passes and `subagent_rows_cost_test.bend: did not run`. That failed result is
retained in the receipt and remains scheduled with an unknown duration. No
nightly, proof or build was restarted.

The old profile contained only 90 estimates. Current full-tree partitioning
keeps all 572 programs exactly once; 513 have recorded weights, 59 use the
existing unknown fallback. `import-test-timings.py` verifies checked-out source
identity, exact historical inventory, no duplicates and the original shard
assignment before writing any weights. No pass/cache/skip logic changes.

| Change | Before | After |
|---|---|---|
|Recorded historical shard work, seconds |3797.6 /3146.8 /4822.2 /4671.5 |4109.4 /4109.8 /4109.4 /4109.5 |
|Largest recorded-work shard |4822.2s |4109.8s (14.8% smaller model) |
|Profile recorder with inherited focused/sharded environment |Runs narrowed selection and overwrites full profile |Forces full/serial, clears selection; failed run preserves old profile |

[Profile receipt](nightly-profile/profile.json) binds the four job-log hashes,
source revision, importer/output hashes and failed test. These are recorded-work
sums from the old concurrent run, **not an observed nightly wall-time gain**.
They omit the failed test's unknown duration and cannot predict effects of
contention, new code/tests or the current serial worker policy. Real historical
shard job walls were 31.6–50.4 minutes. No two-minute full-suite claim is made.

The saved logs also show larger memory use than the previous 8.2GiB examples:
workspace_layout 164.9s/12.3GiB; lang 119s/11.2GiB; body_sidebar_issued 119.7s/11GiB;
thin_mirror 141.6s/10.1GiB. RSS is each test's largest measured process, not an
aggregate concurrent memory bound. Production lock/heap/floor/worker settings
are unchanged; unsafe parallelism is not enabled.

The timing recorder previously omitted `TEST_FULL=1` and inherited
`TEST_FILES`/shard variables. A real fake-runner baseline reproduced exit 0 and
profile replacement with TEST_FULL=0 and the narrowed selection. The repair
forces complete serial selection and atomically replaces only on success.
Six import/recorder controls pass in 0.043s, including missing/foreign inventory,
duplicate/stale revision, wrong shard order and failed-record preservation.
[Control receipt](nightly-profile/controls.json). YAML parsed and the current
572-test partition at the source base was checked independently.

Future CI shards retain small per-test timing artifacts for three days even
on failure; artifact transfer overhead remains unmeasured. This pass spent
**$0 Boat credit**, ran only fake-tool/metadata checks and changed no Bend app,
law, proof, installer or design source. Full proof/build intentionally omitted
under the current user instruction.

UI handoff settled: regular PR #790 normal-merged as 34c7f0d92a4ff15a2d1ca3be85e55d4a70cc96bc.
Exact pushed head b248be1c978cf236292ae71f05c75d9d0eaa2da6 Git-free export took 0.260s,
with cached simulator pass. Static/overall remain incomplete/exit 3 for 37
computed bindings and 31 URL expressions; trace remains incomplete with its
fixture separately passing. No pre-push hook is wired.

## Hub/web compiler memory: profile and rejected small-heap experiment

The current priority is a release compile below **6 GB**. This remains
unproven. Neither the hub nor web import closure includes `LAWS.bend` or
`PROOF.bend`, so separating those files out of emission cannot reduce these
builds. The law/proof gates remain required separately.

Reused build owner1188217's completed source 273b268cd1c56661ead5cc8ba0ae1b5a33ed375f
baseline instead of replaying it: backend emit 108.928s /14,217,712KiB
(13.56GiB), web 59.309s /8,146,460KiB (7.77GiB), stamp 0.662s /404,400KiB,
Bun packaging 0.672s /354,240KiB. That job used a 16GiB no-swap scope, a 4GiB
JSC RAM hint, 1GiB OS stack and 512MiB JS stack. The stack settings are limits,
not a measurement of committed stack memory.

Instrumented public Bend v2.0.36 source at
ae1101ca7d15364f9274fa6b1367d175884a7da7: main/comp diagnostics only, with
**no change to human-owned bend.ts, checker calls, laws or proofs**.
[Observation patch](../../patches/bend-memory-profile.patch) applies to the
clean tag. It is a Bun-only diagnostic patch for an isolated source checkout,
not a deployed compiler; the Node loader was not qualified. Both probes use
pinned Bun 1.3.10 executable
SHA 795b37d736a88d746b0e8bb5fcde92bf29bd0f9ac6a1dd1a950fd37440cf5822,
frozen source 273b268c, a verified 552-input packet and one 8CPU/15GiB Boat VM.

| Boundary | RSS at checkpoint | Cumulative process peak |
|---|---:|---:|
| Load/parse complete |3.91GiB |3.92GiB |
| Type-check complete |9.27GiB |9.27GiB |
| Compiler analysis complete |12.13GiB |12.17GiB |
| JS function emission complete |13.36GiB |13.37GiB |
| Output strings joined |13.16GiB |13.37GiB |

These are checkpoint values and cumulative high-water marks, **not isolated
phase allocations or net live heap sizes**. The final census reported about
216 million JSC cells at its most recent GC. That count does not establish
current reachability: both objectCount and heapSize describe the previous
collection; objectTypeCounts has different allocation semantics. This
[Bun API distinction](https://bun.com/reference/bun/jsc/HeapStats) was also
checked with a tiny allocation fixture on the pinned runtime.

The roughly 15MB emitted file and ~0.1GiB joining increase are small relative
to the checked/compiler representations. Instrumented wall time 162.116s
includes expensive census walks; it is not an uninstrumented speed baseline.
[Raw checkpoints](compiler-memory/baseline-server-profile.jsonl),
[GNU time](compiler-memory/baseline-server.time),
[summary](compiler-memory/baseline-summary.json).

| Change / trial | Before | After / decision |
|---|---|---|
| Small-heap policy, exact frozen hub | Source-compiler profile 13.37GiB /162.116s with census | `--smol` 12.64GiB /255.494s without census; **rejected**, above 6GB and slower |
| Full web bundle in experimental packet | Saved complete baseline 7.77GiB /59.309s | **Failed** after 65.672s: packet omitted desktop/connection-record.cjs. Observed 7.64GiB is a failed partial run, no bundle gain/pass claim |

The source compiler's raw output initially differed from the saved official
executable's emit: only embedded f32_round Function.toString formatting
changed. Every byte before and after that helper matched. Both exported
helpers normalize to the same 467-byte function under the pinned production
minifier; a deliberately changed helper does not. An initial unexported
normalization fixture was eliminated as unused and was discarded. The
[qualified format control](compiler-memory/format-control/result.json) records
that distinction. The original strict-output failure is preserved, not
relabelled. The later small-heap emit matched the source-compiler baseline
SHA 5f2b021aa080e2424f77db1c56e98e2f447e05aed2ecfdb19f610bf325841666
exactly. **Correction:** both jobs rejected malformed equality syntax (missing
`: Nat`) at the parser. Their negative control did not establish theorem
rejection; the raw failure logs are preserved. This changes that qualification,
not the recorded memory, time or output hashes.

First job j20261103161034-146641 cost 734credit-s (**$0.00734**); second
j20261103162034-268193 cost 1414credit-s (**$0.01414**). Total **$0.02148**.
Both original --idle0 runners deleted their VMs and verified deletion.
No Alpha release output, installed compiler, installer-owned release file,
start guard, live PID 3103745 or live/private home was changed.

`bend-memory-report.py` summarizes saved data without compiling anything.
Four cheap controls pass, covering truncated/duplicated/nonmonotonic profiles
and failed/contradictory process receipts. The small-heap policy is not enabled
in production. Next work must reduce retained compiler representations and
qualify a complete frontend closure; no 6GB release or 120s complete CI success
is claimed. Completed experiment sources were archived under ignored build/
to avoid polluting future whole-checkout source inventories.


## Typed compiler controls and module experiment (2026-11-03)

The initial PR836 module-check experiment kept the v2.0.36 checker unchanged
and called its existing prefix-check API at local module boundaries. That
revision retained all definitions, bodies and erased terms, with an optional
lower/higher roundtrip. Cycles, missing or unfamiliar imports, canonical
LAWS/PROOF and unfilled/cross-file declarations used the original full path.
The current [opt-in patch](../../patches/bend-module-check-experiment.patch)
evolves that storage strategy as documented in the later V18 measurements. This is **not installed or
selected by production builds**. Its complete hub/web memory and output
qualification is pending; it is not yet a 6GB improvement or a pass cache.

| Control / trial | Before | After / qualification |
|---|---|---|
| Memory-profile false-theorem control | Missing `: Nat`: parser failure, incorrectly described as proof rejection | Typed `{0n == 1n : Nat}` reaches proof body: expected0n/observed1n in all three modes; failing cross-file law also rejected |
| Tiny module controls on h@backplane | Original full-check mode | Six remaining cases /18 invocations in2.314s; largest child143,332KiB;2GiB cap/no swap; six valid cases from earlier attempt retained as described below. No whole-hub memory comparison |

The earlier terminal attempt passed six valid cases in all three modes
(diamond imports, template specialization, imported type error, cyclic and
missing imports, datatype emission), then stopped at a malformed law fixture.
Those18 results were reused with exact identical compiler/toolchain inputs.
The remaining18 calls cover typed false theorem, passing/failing LAWS+PROOF,
a successful filled module-local law, and two official files whose missing Base
APIs remain expected failures. Those two files are not successful functional
tests. Passing emissions must have identical raw hashes across full, per-module
and compact modes; the float fixture records the common hash explicitly.
Canonical LAWS/PROOF never reported modular completion.

[Raw receipts and source manifests](compiler-memory/module-qualification/SUMMARY.json)
bind unchanged checker SHA1d133151652b597a1a035c77c172475e6de096da37f3b9e3adc23c2f75bdfe49
and pinned Bun SHA795b37d736a88d746b0e8bb5fcde92bf29bd0f9ac6a1dd1a950fd37440cf5822.
The final batch measured cgroup peak110,202,880B; child RSS and cgroup accounting
are distinct metrics. Both batches used actual native bend flock, sole-slot
ledger admission, unchanged2GiB no-swap cap and8GiB reserve; no installer work,
application build or live hub action. One preceding transport attempt stopped
before admission because Python's data filter stripped group-write modes;
the corrected private packet restores only validated ordinary manifest modes.
All source bytes/modes and runtime hash were reverified after execution.
New Boat spend **$0**: no VM was started, controls used the existing owned host.
The protected checker and project laws/proofs were not edited. Broad proof,
release and installer builds remain omitted; no memory target or full-CI pass
is inferred from these small controls.


## Module-mode cap failure and proof parallelism (2026-11-03)

The frozen273 module/compact hub trial **failed the6GB target**. Matching
systemd scope `run-p1355330-i1355738.scope` reports kernel `oom-kill`,
MemoryMax6,000,000,000B, MemoryPeak5,999,996,928B and285.809s wall. The
process stopped after152 module boundaries, with15,985 definitions reached;
no emit or web measurement exists. Sampled process high-water5,426,972KiB
is not the final aggregate scope peak. Small module boundaries repeatedly
spent about4.8s in check/compaction/collection, so this candidate also fails
our speed goal. Its unchanged cap will not be retried. [Failure receipt and
raw checkpoints](compiler-memory/module-cap-failure/RESULT.json).

| Change / trial | Before | After / decision |
|---|---|---|
| Module-prefix checks, compact interfaces, boundary collection | Completed frozen273 hub emit13.56GiB /108.928s; instrumented source baseline13.37GiB /162.116s | **OOM**,6GB scope /285.809s, no output. Rejected as a deployable improvement; platforms/instrumentation differ |
| Exact proof partition planning | One canonical root containing3,149 laws |32 partitions planned in4.74s /51,524KiB; all declaration blocks unchanged, exact-once primary law ownership, all original imports and444-file source union covered. **Planning only**, worker wall/RSS and critical path unmeasured |

`scripts/proof-shards.py` generates conservative local helper/method/import
closures; unknown canonical syntax fails closed. Unused helpers and imports
are still checked somewhere. Every selected canonical declaration is copied
byte-for-byte, and every generated pair passes the existing shape check.
The partition and actual compiler/library/verdict/guard bytes are bound by a
manifest. Execution requires an inherited native bend flock and a finite
no-swap cgroup or isolated default-size Boat VM no larger than8GiB; a passing worker must also peak below3GiB. The join requires every distinct expected
worker receipt and rechecks the existing bendok verdict, source/tool identity,
input stability and finite measurements. Missing, failed, duplicated, corrupt
or stale results cannot make a passing gate. No proof pass cache is enabled.

Eleven fast controls pass in0.31s /27,512KiB, including dropped-law and
unused-import-change controls, compiler-byte/mode drift, unknown syntax,
symlink parents and missing/duplicated/false-green worker results. The existing
full proof gate is not replaced until all required shards and typed theorem mutations
qualify the generated programs. No full release-proof success, <3GiB actual
worker peak, new parallel critical path or2-minute complete CI is claimed.
Next execution must use available admitted workers without duplicating the
build owner's productive proof/store-recovery campaigns. Hub emit, complete
web bundle, proofs and tests can run on separate hosts; independent module
compilation/linking remains unavailable in this compiler experiment.
New Boat spend for the failed cap run and planning/tests: **$0** (owned h@ host
and cheap local planning only). No Alpha, installed app or live PID action.


Parallel shard transport attempts025/026 (jobs j20261103172959-1014215 and
j20261103173000-1014525) both stopped **before Bend execution**. An ignored
build/ manifest was incorrectly included in the non-build inventory comparison;
025 also received026's archive created between the controller's exclusion
snapshot and runner staging. The closed-inventory verifier rejected both.
Neither is proof or memory evidence. Both sandboxes were deleted and verified;
14+7credit-s cost **$0.00021**. Seventeen packet controls pass in0.37s /28,804KiB,
including build-manifest hash verification and unexpected-source rejection.
An exact bootstrap allowlist is being coordinated with runner owner1188217;
no replacement proof job starts with the racy exclusion-only transport.


## Two actual small proof partitions and parallel transport correction

The first two generated partitions now ran successfully on independent default
Boat VMs, with the canonical statement/proof bytes unchanged and the standard
pinned compiler. These are198 distinct primary laws out of3,149; the complete
32-partition gate is **not** replaced or claimed passing.

| Partition | Canonical proofs included | Bend wall | Peak child RSS | Kernel swap I/O |
|---|---:|---:|---:|---:|
|025 |99 |0.819s |189,136KiB (185MiB) |0 pages |
|026 |102 (99 primary) |2.156s |207,984KiB (203MiB) |0 pages |

026 returned0. 025 returned1 **only for the existing 16-definition unsafe/foreign
notice** (font/text effects); the unchanged `bendok.sh` accepts that notice,
as it does in the original full gate. Its raw warning remains in the receipt.
No checker error or failed theorem was accepted. A typed `{0n == 1n : Nat}`
negative control reached its `{==}` proof body and was rejected with
expected0n/observed1n. These checks do not establish external effect correctness
or a Lean-kernel verdict.

[Exact input and tool identities, raw results and provenance](proof-shards/receipts/SUMMARY.json)
are retained with the measured runner and partition manifest. Both jobs checked
source/tool bytes before and after, held the actual native bend flock, and
used default VMs (about7.76GiB physical cap). The guests provisioned2GiB swap;
**pswpin and pswpout stayed0 before/after each command**. The worker and join
reject any swap activity, so paging cannot masquerade as a small-memory pass.
Host workers still require a finite no-swap cgroup. Successful worker RSS is
capped by the verdict policy at<3GiB; larger VMs are rejected.

VM lifetimes overlapped; the very short Bend subprocesses did not, due to
startup skew. Max Bend duration2.156s; outer commands1.35s/2.49s; the observed
command window was about5.49s with1s timestamp resolution. Per-job startup
2s/4s, push2s each, setup2s each. The full-suite parallel critical path and its
largest worker are still unmeasured; the largest planned import closure must
be reduced or qualified before any all-suite speed/memory claim.

The exact3-file allowlist from runner PR841 fixes the concurrent-archive race;
`ci-packet.py` now uses it, rather than a snapshot of exclusions. Its build/
manifest inventory comparison was corrected without relaxing source/hash
verification. Eighteen packet controls pass in0.39s /28,060KiB; runner include
controls3/3 pass in1.712s. Proof partition controls12/12 pass in0.29s /26,220KiB.
Two preceding v2 resource attempts stopped before Bend and cost8credit-s each.
Successful pilots cost9+11credit-s (**$0.00020**) with deletion verified; all
six setup/transport/success attempts total57credit-s (**$0.00057**).
No source law/proof, release workflow, installed compiler or live hub changed.

### Retained emission admission and unsuccessful paging experiments (2026-11-03)

| Change | Before | After |
|---|---|---|
| Normal `service-release.py js` retained-emission handoff | Copied verified JS into the legacy size-only cache; `build-js.sh` requested whole-build16GiB before looking up that cache, refusing small consumers before compile | Pass exact verified producer path/SHA256/import-tool key; verify before separate `compile-js`3GiB admission and again during atomic copy. All existing whole-build/check/proof floors and8GiB host reserve remain unchanged. Missing/corrupt/drifted input fails; no Bend fallback. |
| Verify/copy retained33ce raw hub emission,15,919,342bytes | Old consumer did not reach reuse after admission refusal | Three actual copies:0.03/0.03/0.04s,22,244/21,836/21,336KiB peak RSS; every output SHA256 exactly `ca921838988fe844f7e1bc3f0040d5d30f4d9d139facecd76014c17c0c94185f`. Copy-only measurement, **not a Bun/producer/release qualification**. No paid compute. |
| Paged module prototype, frozen33ce hub/web/mobile | Official hub147.340s /19,189,172KiB; web72.539s /8,773,704KiB (completed existing producer reused) | All three new compiler scopes OOM-killed at3,699,998,720bytes during `core/refview.bend`. No qualified output or speedup. Hub/web/mobile scope wall times40.129/22.702/44.353s; GNU compiler RSS unavailable after scope kill. |
| Bounded groups within a module | Module-level paging failed at refview | Tiny16cases×4modes passed in11.837s /264,484KiB maximum child RSS under actual2GiB/no-swap scope; forward-call/forward-proof/late-false-proof controls reject. Focused refview still OOM after~52s. Last independent sample:anonymous3,626,995,712bytes,file35,483,648bytes,kernel27,246,592bytes. File cache was already reclaimed; anonymous allocation is the remaining problem. |

Consumer controls13/13 (0.229s), admission16/16 (0.048s), existing producer
attestation13/13 (0.010s), producer integration47/47 (0.674s) pass. These are
offline boundary checks; no app binary, protected bake, full proof suite or
release packaging was run. Streaming copy preserves raw bytes and does not
stand in for the still-required final-output identity. The3GiB admission floor
is an expected working-set allowance, **not a hard memory cap**; execution
qualification must retain the runner's actual sub-4GiB no-swap caps.

[Exact copy measurements](retained-emission-copy.json) and
[paging experiment receipts](compiler-memory/paging-experiments/refview-v3.json)
retain identities and raw-artifact hashes. Hub/web/mobile failed attempts cost
67+37+71credit-s ($0.00175); focused refview64credit-s ($0.00064);
tiny v3 controls21credit-s ($0.00021). All default Boat sandboxes were deleted
and deletion verified. No larger VM or cap increase was used. **No qualified
memory-fix trigger exists yet; RC2 remains paused.**

### Focused refview regression isolation (2026-11-03)

These use the same frozen33ce closure, unchanged2.0.36 checker and pinned
Bun1.3.10. Each compiler scope has a3.7GB hard cap and zero swap; the separate
512MiB controller keeps the sum of hard caps below4GiB. They are focused
memory experiments, not hub/web/mobile release qualifications.

| Change | Before | After |
|---|---|---|
| Matched original whole-book refview baseline | Paging variants failed without a matched focused original measurement | Original passes in22.50s at2,671,396KiB actual GNU peak RSS; sampled cgroup peak2,699,665,408bytes. Profile checkpoints alone miss the real peak (1,917,216KiB). Subsequent qualification readers now require actual GNU receipts. |
| Lazy-body paging v4, annotation interning v5, weak identities v6 | Refview fails at3.7GB cap | Still fail at the same cap; scope wall56.504/24.635/27.168s respectively. V5 isolates failure inside kernel checking `Card.of`, before spilling. Annotation disk size falls but this does not qualify memory. |
| Retain exact type-interface graphs v7 | Paging reconstructed interface graphs; `Card.of` checking exceeded cap | `Card.of` kernel check finishes at26.425s/RSS1,907,806,208bytes. Failure moves to spill storage, scope killed42.401s. No qualified output. |
| Preserve binary lower-term graph sharing v8 | JSON storage expands shared graph nodes | Tiny graph-sharing/byte-verdict controls pass, but the focused run still fails89.201s into its compiler scope. Markers isolate the remaining expansion to re-lowering the original higher-order source body, before writing the checked result. |

Later candidates retain exact parsed source bodies and interfaces, paging
only checked lower results. The measurements below establish their outcomes. No checker/law/proof
change or source rewrite is used. Original positive tiny outputs are reused
by hash; only changed paging modes are rerun. Exact source/tool input checks,
typed false-proof, forward-reference and corruption controls remain required.

[Baseline resource evidence](compiler-memory/paging-experiments/baseline-v6.json)
and [latest failed phase evidence](compiler-memory/paging-experiments/v8.json)
include raw artifact hashes. New v4 tiny controls23credit-s, focused v4/v5/v6
73/59/49credit-s, original baseline34credit-s, v7/v8 combined controls+focus
65/123credit-s:426credit-s total (**$0.00426**). Every sandbox was deleted and
deletion verified. These results do not authorize release resume.

### Checked-result paging and resident-memory diagnosis (2026-11-03)

All measurements retain the same frozen33ce source and unmodified checker.
The compiler cap remains3,699,998,720bytes, zero swap, with a separate512MiB
controller. Controller-only peak RSS near264MiB is **not compiler RSS**.

| Change | Before | After |
|---|---|---|
| V9 retain source bodies; v10b higher-interface handles | Re-lowering source bodies expands shared graphs | Both still hit compiler cap:47.038/54.565s scope wall. No usable output. |
| V11 stream checked graphs with shared higher interfaces | Spill construction exceeded cap | Module checking and spill finish, but verdict traversal reloads checked results and hits cap at87.711s. |
| V12 separate exact dependency walk | Verdict reloads results | Additional graph walk exceeds cap at66.136s. |
| V13 collect exact dependency names during the same streamed traversal | V12 exceeds cap | Focused refview passes:61.776s, actual GNU peak3,540,544KiB, zero swap. Original whole-book focused baseline22.50s/2,671,396KiB remains faster and smaller; this is a paging milestone, **not a standalone improvement or release qualification**.38 changed-mode controls and32 exact graph-reference comparisons pass. |
| V13 frozen complete hub/web/mobile phases | Official hub147.340s/19,189,172KiB; web72.539s/8,773,704KiB | All three hit the unchanged3.7GB compiler cap:254.310/186.245/73.825s scope wall. Hub fails after core/cef; web reaches JS emission; mobile fails during Card.of. No final output identity can be claimed. |
| V14 mobile check-only discards emitter IR after unchanged checker accepts it | V13 retains paged emitter IR | All41 tiny controls completed across the same-compiler v14/v14b receipts, with nine remaining invocations rerun after correcting expected PASS text. Actual mobile still hits cap at102.946s in core/msgs, with stored_bytes=0/store_reads=0. No mobile pass or new bridge emission. |
| Owned mobile prefix memory diagnostic | OOM receipts lack resident-memory attribution | Captured live process mappings and JSC counters; deliberately stopped only its own compiler scope at56.968s near3.0GB cgroup usage. Last mapping sample:anonymous2,760,568,832bytes; stack569,344bytes; file44,969,984bytes; anonymous executable2,289,664bytes. Stack limits reserve address space but stack residency is small here. **Diagnostic completion is not a compile pass.** |

The one-shot JSC census was taken before an explicit collection: its
heapSize/objectCount describe the last collection, while objectTypeCounts
include subsequent allocations. Do not interpret its4.6MB heapSize as the
live retained heap. A separate post-collection comparison is required;
[Bun documents this counter distinction](https://bun.sh/reference/bun/jsc/heapStats).

[Focused v13 receipt](compiler-memory/paging-experiments/v13.json),
[full hub](compiler-memory/paging-experiments/hub-v13.json),
[web](compiler-memory/paging-experiments/web-v13.json),
[mobile](compiler-memory/paging-experiments/mobile-v13.json),
[check-only mobile](compiler-memory/paging-experiments/mobile-v14b.json) and
[diagnostic](compiler-memory/paging-experiments/mobile-diagnostic-v14.json)
retain exact source/tool identities and hashes of raw artifacts. V9/v10b/v11/v12
cost68/74/108/107credit-s; focused v13 costs85; complete v13 phases283/203/90;
v14 harness-only14; v14b137; diagnostic76. Total1,245credit-s (**$0.01245**).
All sandboxes were deleted and deletion verified. No release resume, installed
compiler replacement, app execution or proof/law change follows these receipts.

### Bounded declaration batches and checked-result storage (2026-11-03)

The updated opt-in patch and [exact source/tool manifest](../../patches/bend-module-check-experiment.json)
reproduce the measured V18 compiler from clean upstream commit
`ae1101ca7d15364f9274fa6b1367d175884a7da7`. Applying the patch in a fresh
export reproduces all nine compiler/Base hashes; human-owned bend.ts and
safe.ts remain byte-identical. This is still an experimental Bun-only path:
normal build admission and the installed compiler are unchanged. Full
hub/web qualification is pending. No local/release trigger is implied.

| Change | Before | After |
|---|---|---|
| Remove weak decoded-IR cache, V15 | V13 web reaches emission but exceeds3.7GB cap | V15 still exceeds cap during checking at150.021s. No claimed gain. |
| Explicit collection at the same mobile prefix checkpoint | RSS2,148,741,120bytes before collection | Bun.gc(true):2,097,881,088; gcAndSweep():1,992,044,544. Post-GC live heap761,940,430bytes includes463,363,038bytes external. Diagnostic only; intentionally stopped at23.872s after capture. |
| Batch128 declarations instead of one at a time | Mobile V14b OOM at3.7GB after102.946s; repeated prefix-table construction per event | Same frozen33ce mobile check passes96.804s /2,768,116KiB (2.64GiB), raw exit0.28 compiler invocations plus one CLI rejection control pass; beyond128 forward-reference and false-proof cases reject,300-def positive emission matches full reference. |
| Apply batch128 to complete hub/web | One-event variants exceed cap | Both still fail while spilling accepted refview results:45.982s hub /80.582s web. Kernel had returned successfully; no final byte qualification. |
| V17 page higher type annotations, preserving free-variable identity | Retained higher graphs keep IR alive | First focused check passes35.873s /2,649,048KiB, but receipt shows discard_results=true and stored_bytes=0. This check does **not** exercise the new paging codec. Corrected emission gate below supersedes it. |
| V17 actual retained-storage synthetic emission over frozen refview imports | Matched unchanged whole-book emission22.003s /1,864,996KiB | Exact output bytes match, but122.724s /3,612,472KiB is a regression.1.07million resident annotation handles correspond to only46,300 distinct blobs. |
| V18 content-intern handles, bound identity cache256, free obsolete accepted e after successful hash write | V17 emission122.724s /3,612,472KiB; post-check RSS2,824,450,048bytes | Same actual retained-storage gate62.400s /2,977,024KiB (2.84GiB); post-check RSS889,204,736bytes. Final SHA256 exactly `a7401f2712f024aa9f4b7c2101b454e15c6fba0001eb9f5ec7bafb5419b7097f`, matching unchanged whole-book reference. Still slower/larger than that original focused baseline. |
| V18 same-compiler mobile check | Earlier batch128 compiler96.804s /2,768,116KiB |88.149s whole compiler scope (check:end87.860s) /2,770,720KiB; cgroup peak2,802,311,168bytes, swap0. Exact6,037,027-byte ddb9 bridge reused from its previously qualified unchanged closure; no new bridge emission, SDK build or app package. |

[The exact codec fixture and completed controls](compiler-memory/storage-controls/IDENTITY.json)
cover open dependent annotations and caller-variable identity, corruption,
cross-store/missing handles, Unicode boundaries, DAG sharing, exact dependency
sets, bounded identity cache, content interning, release of obsolete accepted
emitter IR, and typed false-proof/forward-reference rejection.22 changed-mode
compiler invocations reuse the already completed original full-mode references.
Source T/v graphs remain exact; no theorem or law is omitted or changed.
Every experiment retains3.7GB compiler plus512MiB controller hard caps
(sum4,236,870,912bytes<4GiB), zero swap and the actual native bend flock.

[Mobile batch receipt](compiler-memory/paging-experiments/mobile-batch128.json),
[corrected V17 emit](compiler-memory/paging-experiments/refview-emit-v17.json),
[V18 emit](compiler-memory/paging-experiments/refview-emit-v18.json),
[V18 mobile](compiler-memory/paging-experiments/mobile-v18.json) and
[post-GC census](compiler-memory/paging-experiments/mobile-postgc-v14.json)
include raw artifact hashes. V15 web170credit-s; post-GC diagnostic38;
mobile batch128120; hub/web batch12860/102; initial V17 check60;
corrected V17 emission including new original reference187; V18 focused90;
V18 mobile117. Total944credit-s (**$0.00944**), all sandboxes deleted and
verified. This cumulative section includes failed/scoped experiments; its
passes do not establish a complete low-memory hub/web build.

### Cold source recipes, metadata cache and full-run limits (2026-11-03)

These experiments preserve the protected2.0.36 checker and actual3.7GB
compiler +512MiB controller caps, sum4,236,870,912bytes<4GiB, swap0.
All full-run references below are frozen33ce. No low-memory hub/web final
output has qualified; PR852 remains opt-in tooling, not a release trigger.

| Change | Before | After |
|---|---|---|
| V18 complete hub/web after bounded storage | Original official hub147.340s/19,189,172KiB; web72.539s/8,773,704KiB | OOM at362.557s hub /399.23s web; same3.7GB compiler cap. Hub reached24,264definitions before workspace checking; web completed workspace checking before storage failed. No final-output identity. |
| V19 cold source bodies rebuilt by unchanged parser | V18 retains original parsed v ASTs | Exact type interfaces remain resident; ordinary cold v uses hash-bound source/offset/prefix-header recipes. Unsupported layouts and law/fill cases retain the original body. Every original kernel check remains required. |
| V19 qualification fixtures | New reference used Base's existing name Word; next same-file alias needed no cold replay | First fixture fails fresh-name parsing, then strict replay-coverage guard rejects unexercised path. Neither is a compiler pass. Imported type fixture forces replay; remaining6 calls include passing/false theorems through a paged dependency. Prior22 completed cases and codec reused by exact hashes. |
| V19 real retained-storage focused gate | V18 focused62.400s/2,977,024KiB | V19c83.596s whole scope /2,932,636KiB;2,228 bodies paged/411 replays. Exact final JS bytes match original reference. Slower; no speedup claim. |
| V19 mobile check and retained bridge reuse | V18 mobile88.149s/2,770,720KiB; post-check RSS1,997,938,688bytes |183.311s/2,715,740KiB; post-check RSS1,035,329,536bytes.20,126 bodies paged,8,411 replays/metadata reads. Exact existing ddb9 bridge preserved; no new bridge/SDK/package. Lower steady RSS, wall regression. |
| V19 complete hub/web | V18 OOM | Both stop at600s configured timeout without OOM, no final output. Hub last module botnet with5,442,452,055bytes stored; web still storing accepted workspace results. Last hub cgroup:anonymous836,059,136bytes,file cache1,831,563,264,kernel795,906,048. Memory pressure moved toward file/cache overhead; these are failed partial runs. |
| V20 bounded32MiB immutable parser-metadata cache | V19c source metadata read per replay | Focused scope171.203s/2,867,012KiB, exact JS match;407 replays need44 reads with363 hits. Wall regresses despite fewer reads. GNU CPU:106.27s user/64.19s system versus V19c77.55/7.36, with99%/101% CPU and no major faults. Do not attribute this to VM queueing or claim a speed win. |
| V21 mobile check plus exact retained bridge reuse | V19 mobile183.311s/2,715,740KiB; V18 mobile88.149s/2,770,720KiB |149.628s/2,717,832KiB (2.59GiB), cgroup peak2,775,441,408bytes, swap0 and no OOM.8,446 source replays require2,813 capsule reads/5,633 hits;182 metadata records in one file. Exact ddb9 bridge reused; no fresh bridge/SDK/package.18.4% faster than V19, still slower than V18. Full hub/web failures below. |
| V21 complete web with packed metadata | V19 web600s timeout without final output |397.645s then systemd Result=oom-kill at3,699,998,720byte compiler cgroup peak, swap0. Module checking completes331.121s; failure occurs during JS analysis, before analysis:end. Last sample anonymous3,638,951,936bytes/file10,330,112/kernel49,225,728. Final GNU RSS unavailable after kill; outer264,964KiB measures controller. No final output identity and no retry. |
| V21 complete hub with packed metadata | V19 hub600s timeout |600.277s controlled timeout: timed_out=true/raw_exit124; no OOM observed. Last completed module ring at598.826s/25,771defs; next rules group still checking.4,772,389,115bytes stored. Checkpoint peak2,950,448KiB; final GNU RSS unavailable. No modules:complete, JS analysis or server.js. No final-byte qualification or retry. |

The V21 candidate packs metadata into one append-only private file while
retaining per-segment SHA256 and bounds verification. This targets the
hundreds of thousands of small metadata files and repeated existence/open
operations observed in the earlier store. Lower-term files retain their
previous format. V21 passes25 changed-mode controls plus codec corruption,
truncation and isolation checks. The focused scope passes106.574s /2,846,192KiB
(2.71GiB), with exact reference JS bytes.43,956 metadata records occupy one
53,670,557-byte file; ending kernel memory38,268,928bytes versus V20
101,896,192bytes. It improves V20 time171.203s but remains slower than
V19c83.596s and the unchanged whole-book focused reference22.003s.
Cgroup peak reaches the3.7GB cap (file cache included); no full-program
qualification is implied. Job j20261103203918-2786061, packet
`db7531b95c94f5e58d81f605156f9c542b3e29b2a05657c2583baea786c1951d`.

Promotion hold: static inspection after the V21 mobile run confirmed that
`sourceCapsule(key)` returns a cached value before `get(key)` revalidates the
backing segment. Existing corruption controls clear the caches first, so they
do not cover tampering after a successful first read. In-memory decoded data
was previously verified, but the promised fail-loud storage-mutation contract
is not established for this path. V20/V21 receipts remain experimental timing
evidence and cannot qualify promotion. Lane5 owns the isolated correction
and negative controls; the immutable V21 hub/web jobs ended as reported above.

Lane5 isolated warm-hit correction (reviewed and applied in private checkout only):
`offload.ts` reuses the decoded capsule only after rechecking its backing
record bounds and SHA256.18 controls pass, including warm same-size edit,
truncation and missing record rejection. Live/loaded bodies explicitly remain
verified in-memory snapshots; clear() alone does not invalidate live WeakRefs.
Job j20261103205550-2996607 compiler scope82.690s /2,928,256KiB; packet86.165s.
Exact focused JS matches, enforced caps and swap0 retained.95credit-s
(**$0.00095**), reused receipt with no duplicate run. This isolated single
measurement cannot establish a causal speedup or qualify full output.
After all three V21 jobs became terminal, the exact isolated offload delta
was applied to the CI owner private compiler only. Protected checker/safe/Base
and original experiment snapshots remain unchanged; no normal build rollout.
Corrected upstream patch SHA256
`4405d272062f81d713743aab104af0c5943888b65dfd33d124b66492f7340d12`
is handed to the swarm integrator; no full candidate is repeated.

Original source, reconstructed-body identity, typed false theorems,
forward references, corruption/truncation/missing/cross-store records,
and exact output bytes remain mandatory controls. A focused constant main
imports and checks the real25-file refview closure but does not emit every
refview function; its receipt cannot qualify a complete hub/web program.

Costs: V18 full hub/web378+420credit-s; V19 fixture/corrected focused
13+9+101; V19 full hub/web/mobile617+617+200; V20 focused197; V21 focused141/mobile164/web411/hub626.
Total3,894credit-s (**$0.03894**), all terminal sandboxes deleted and deletion
verified. No installer files,
installed compiler, app execution, live auth/settings, telemetry or release
authorization changed.

### Saved V21 phase audit (2026-11-03)

[The reproducible summary](compiler-memory/phase-audit/SUMMARY.json) binds
the existing raw journals and result files by SHA256. This is offline analysis:
**zero new compiler jobs and $0 additional Boat cost**. Regenerate it with
`python3 build/ci-speed/compiler-memory/phase-audit/summarize.py --output /tmp/v21-phases.json`.
A measured regeneration takes 0.08s and 32,332KiB peak RSS and reproduces
the committed summary byte-for-byte. The script requires the three saved job directories under
`~/.local/state/backplane/boat/jobs`, or an explicit `--jobs-root`.

| Measured interval | Hub, partial 600.277s run | Mobile, completed 149.628s source check |
|---|---:|---:|
| Completed checker groups | 365 | 285 |
| Inside unchanged `Bend.book_valid` | 256.312s | 104.872s |
| Validated group through storage/cache/collection/yield completion | 298.701s | 26.023s |
| Outside those completed intervals, including load/preparation and partial work | 45.263s | 18.732s |

Intervals are wall time, not CPU samples. They are disjoint within each
single-book run; the incomplete hub group is excluded. The post-check
interval includes spill serialization, cache work, collection and scheduler
yields, so it must not be described as pure disk I/O. Mobile's check-only
mode discards accepted emitter IR; hub must retain it. This difference helps
locate the next investigation, but these different programs and incomplete
outcomes do not establish a causal performance comparison.

The slowest completed hub modules, including checking and post-check work:

| Module | Time | Added declarations |
|---|---:|---:|
| `src/web/view.bend` | 88.872s | 890 |
| `src/core/hub.bend` | 81.267s | 1,683 |
| `src/core/refview.bend` | 72.790s | 81 |
| `src/web/workspace.bend` | 57.277s | 77 |

Refview's one completed group spends 9.411s checking and 63.285s after
validation. Workspace spends 12.066s checking and 45.191s after validation.
These are better targets for retained-result storage work than source line
count alone. Hub reaches only 245 completed module checks before timeout;
these are not a ranking of the entire program.

Web starts three loader invocations at 0.007s, 0.061s and 0.101s for
timeline-plan, timeline-history and app. Their journals share unqualified
event ranges. The summary deliberately refuses per-group attribution for
web: existing trace IDs cannot distinguish overlapping books soundly.
Process RSS is shared by all invocations, not memory owned by one file.
Two small analysis passes finish in 1.981s and 10.005s. App analysis starts
at 331.144s and is unfinished at OOM 66.501s later. Anonymous memory grows
from 1,594,904,576 bytes at that start to 3,638,951,936 in the last sample;
only 10,330,112 bytes of file cache remain there. The final failure is
dominated by anonymous memory, not the metadata pack's resident file cache.
The journal does not identify the exact analysis allocation responsible.

No final GNU RSS exists for the killed/timed-out compiler processes; reported
checkpoint maxima are lower bounds. No separate per-law proof time or pure
parse time can be recovered from these marks, and no full proof suite was
run. PR #856 remains opt-in and nonqualifying for a complete build or release.
