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
