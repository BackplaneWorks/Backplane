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
