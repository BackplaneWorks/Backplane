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
Schema2 records the verification base and both verifier/driver digests; the
local side fetches only immutable source bytes to compare those identities.
New actual Git/shape controls cover both candidate-tampering cases. The old
schema1/8-control receipt is preserved as an initial experiment, not final
trusted-base qualification. The final exact-head Boat guard must pass.
