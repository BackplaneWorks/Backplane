# CI speed measurements

Measured 2026-11-02, source `7b0e95833`, before CI changes. Target: approximately
120 s PR feedback and minute-scale builds, without removing any law, proof or
required assertion. This target is **not achieved**.

## Before / after

| Change / workload | Before | After | Evidence / limits |
|---|---:|---:|---|
| Artifact compression, installed 118,042,489-byte service, DEFLATE 6 → 1 | 2.543 s; 43,834,514 B | 0.934 s; 47,954,641 B | One local pair using Python zipfile/zlib; 63% less compression CPU, 9.4% larger archive. Not an Actions wall-time measurement. |
| Native CI artifact selection, all targets → own target | 840,655,549 B per native download; latest Mac x64 102 s | Not yet measured on Actions | Workflow changed; native smoke still requires its target's binary and every helper. |
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
