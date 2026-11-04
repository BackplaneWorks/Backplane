# Boat-only pre-merge shape validation

`pre-merge.sh` dispatches all merge-tree, guard controls and Python LAWS/PROOF
shape validation to a fresh normal Boat executor, under an actual512MiB
memory.max and swap.max0. It starts no Bend/compiler/proof/build and has no
local fallback. The local controller transports a pushed HEAD and verifies
only static byte/ref/owned-job identities; it never computes a merge or runs
a test. No account credentials are transferred.

The remote uses the public canonical repository and a blobless bare fetch,
without a shallow clone or full checkout. It checks exact pushed worker bytes,
loads the shape verifier and block-wise law merge driver from the fetched trusted base (never the candidate head), computes the
actual HEAD+base merged tree, rejects conflicts anywhere, and applies the
unchanged shape script to that tree's LAWS/PROOF bytes. Head, base, tree and
script/document hashes are recorded. It refuses a base advance during or
after validation, a changed local HEAD/worker, missing/bad owned-job receipts,
wrong scope or OOM. No generic receipt-import, cached-green or local bypass.

Usage: push HEAD to origin, then `scripts/pre-merge.sh [origin/BRANCH|COMMIT]`.
The old local-only/unpushed guard is intentionally unsupported under the hard
Boat-only rule. A missing source commit, network failure, budget/capacity
refusal or conflict stays nonzero; inspect the real result and dependency,
never fall back locally or widen the machine/cap. This source is specifically
for BackplaneWorks/Backplane. No heavy-guard floor/reserve/profile changed.

[Initial transport experiment](MEASURE.json): work1507079/job
j20261104025727-178482, head2f080ca6d;8 guard controls0.049s and the real merged
shape passed. Entire worker6.20s, process peak24,952KiB, actual cgroup peak
33,906,688B (limit536,870,912B), swap0; max/OOM0. Machine wait1s, push2s,
setup0s; one16KiB worker file transported.12credit-s/$0.00012 total, sandbox
deleted verified. Tests, merge-tree and shape ran on Boat; no local test after
the new rule. The later test entry also explicitly refuses local/unbounded
unittest invocations; the final exact-head pre-merge must pass before merge.

Before: Git merge-tree and Python ran locally, disallowed by the new rule;
no isolated old-path timing is claimed. After: same actual structural gate on
Boat, with additional identity/resource controls and network cost. This is
policy enforcement, not a CI speedup or proof/compiler/full-suite qualification.
Original canonical laws/proofs, release files, compiler source and live state
are untouched. Runtime receipts are public hashes/results; raw job logs stay
in the local runner's evidence store.

Final implementation uses an outer systemd scope before starting Python; the
worker reads actual cap/swap back and never raises or self-sets its limit.
The fetched base's shape verifier and merge driver judge the candidate, so a
candidate cannot select an always-pass checker or take-ours merge driver.
Schema3 records the verification base and both verifier/driver digests; the
local side fetches only immutable source bytes to compare those identities.
New actual Git/shape controls cover both candidate-tampering cases. The old
schema1/8-control receipt is preserved as an initial experiment, not final
trusted-base qualification. The final exact-head Boat guard must pass.

The closed guard-source set is `scripts/pre-merge.sh`,
`scripts/pre-merge-boat.py` and `scripts/boat-job.sh`. The receipt records each
base/head blob hash and mode, recomputed independently by the dispatcher.
Any difference refuses by default. A reviewed, intentional guard update uses
`--allow-reviewed-guard-change FULL_HEAD_SHA`; prefixes and other heads refuse.
This exception is printed loudly and yields `passed-reviewed-guard-change`,
never plain `passed`. It applies only to that exact commit. The controls come
from the candidate, can only block, and have their source hash, output hash
and exit recorded. BASE shape validation remains mandatory. External Git,
Python, systemd and Boat installations remain environment trust dependencies;
this procedural guard is not an independent execution attestation.

The final suite has 14 controls, including checker/driver replacement,
conflicting ordinary files, altered guard sources, and mislabelled or absent
acknowledgements. Schema3 qualification is recorded below.

Residual trust root: this version still executes the dispatcher and worker
from the candidate checkout. Its matrix cannot authenticate itself against a
candidate that rewrites the matrix logic. Reviewers must independently diff
all three guard files against the base before trusting a result, for ordinary
PRs too. A future trusted-base dispatcher accepting a separate PR head as data
would remove this procedural dependency; that mode is not implemented here.
An acknowledgement when the files match is refused, so it cannot be ambient.
On a changed guard, failure prints the matrix and exact-head flag. This first
guard PR can qualify only as `passed-reviewed-guard-change`.

History remains blobless and non-shallow as required by repository policy.

## Reviewed schema3 qualification

[Raw result](SCHEMA3-RESULT.json), [controls](SCHEMA3-controls.txt) and
[owned executor provenance](SCHEMA3-provenance.json) cover exactly
`0ea8b82ba88e55220813ea1f6a19a0572a824d12` against main
`c499e7f5477508410dcbb9f014affb9c85cc08a2`, merged tree
`ffab6166f521b998d42ab4bdf68006deb0e770e3`. Work1513291 / Boat
`j20261104032138-375442` ended0, including the dispatcher's returned-source
and final-base identity audit. Expected distinct outcome was
`passed-reviewed-guard-change`, with the acknowledgement equal to that exact
head; controls_exit0, 14 tests0.116s, actual merged shape_exit0.

Worker5.60s /25,600KiB peak RSS; cgroup peak39,907,328B under536,870,912B,
swap0 and all memory events0. Default sandbox wait1s, push3s, setup0;
12credit-s/$0.00012, sandbox deleted verified. One20KiB worker source file
was uploaded; Git blobs were fetched directly on Boat. End-to-end controller
time was not isolated, so worker time is not a PR latency claim.

This qualification is only for the recorded source/base pair. The subsequent
receipt-documentation commit requires its own fresh exact-head guard before
merge. The candidate-dispatcher residual trust root described above remains;
trusted-base `--head` mode is still unimplemented. No proof or full CI/build
qualification follows from this structural gate.

## Explicit runner compatibility

The caller can select an approved external runner with `--runner ABSOLUTE_PATH
--runner-sha256 SHA256 --runner-transport exclude`. A complete digest is
required; no environment-selected runner or automatic fallback is used.
The digest must come from an independently reviewed runner record, not from
hashing an unknown file immediately before executing it. The dispatcher
prints the selected path and digest; caller review is still required.
`exclude` supports older runners that lack `--include-file`, by excluding
every other top-level path and script child from the current inventory.
Glob characters are escaped as literals. It still transfers only the exact
pushed worker, without full-repository upload. The default repository runner
keeps its existing include-file transport. A session requiring a different
runner must explicitly select that approved source; the default is no
exception to such a policy.

The dispatcher reads the reviewed executable source, checks its digest, and
executes a private immutable byte snapshot. It lets that owned execution and
cleanup finish even if output contains ambiguous executor identities, then
refuses the receipt. The original selected source is rechecked after the job;
drift refuses merge permission. `premerge-controller.json` in the owned local
job directory records the selected path, digest, transport, snapshot digest
and exact worker-result digest after all existing source/base/verdict checks.
For both transports it also requires the owned job's actual manifest to
contain exactly one regular-file entry, `./scripts/pre-merge-boat.py`, with
the expected worker digest. Extra/missing/changed entries refuse merge
permission, including paths introduced between the two inventories.
The distinct reviewed-guard-change state is preserved. The runner is a
caller-reviewed trust dependency, not an independent execution attestation.

No account, budget, concurrency, start, size or memory guard is changed or
overridden. Command arguments remain explicit default-size, no-reuse, wait0,
idle0, no Bend setup and outer512MiB/swap0. Trusted BASE shape/merge driver,
three-file guard matrix, exact-head acknowledgement, worker/source/control
identities and base-drift fences remain mandatory. The suite now adds12
runner/transport controls to the existing14. This correction still needs
independent review and normal-Boat qualification before use for merge gates.
