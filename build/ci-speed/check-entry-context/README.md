# Standalone check cache context correction

The earlier check cache ran one aggregate manifest but recorded its result as
independent module passes. That is unsound when an earlier imported module
fills a law or affects generated/template state used by a later module.
Saved actual L/P/U loader traces demonstrate different verdicts from changing
the MAIN import order while those module bytes stay fixed. An earlier claim
about an unimported sibling was retracted; this change does not add arbitrary
directory-neighbour dependencies to keys.

Each missing entry now runs as exactly `bend FILE --check-only` in a fresh
process from the repository root. The check-entry-v2 key binds that entry,
its full transitive import/effect/header closure, compiler/Base/env/verdict
logic, Python executable, stack limits and JSC settings. Old aggregate check
entries are rejected even with an intact local MAC. Source, tool identity,
complete module inventory and all omitted hit MACs are revalidated before
recording genuine scoped passes. A failed entry is never recorded.

Standalone passes still do not imply the original aggregate verdict. CLI
success is advisory exit3, standalone failure/inconclusive exit4; shell maps
even a legacy helper's zero to3. No workflow, nightly or full proof gate uses
this pre-check. The full original aggregate check and proof remain required.
The existing 14GiB cold-check admission floor is unchanged: this PR does not
make the cold execution route fit a normal capped Boat VM, nor weaken its
guard to obtain a passing benchmark. Future bounded cold execution needs its
own sound worker contract. Shared/signed reuse is not implemented.

## Preserved measurements

The source helper is byte-identical to the measured original candidate
1fbd8327ecc08c3f33b636cb86e74653e7cd5eac (SHA
faa23dd9217158ab560c48c9559a508e50b32646e51a1de1f31f9ee4c88e4424).
Rebasing does not turn an old measurement into a new one. The frozen real
corpus is e77e560175612d3637ac0559f2c1923541af0aa8: 1,185 modules /1,222
inputs, closure86306a03a8e8424d1c0b587b7ef69a1216235cf3c79f55d9680e2d916a336d08.

| Metadata-only observation | Before | After |
|---|---|---|
| Median key-plan time, three samples | 0.195583s | 0.194275s |
| Process VmHWM | 58,576KiB | 64,976KiB |
| Actual cgroup peak | 426,598,400B | 334,839,808B |

Both runs used actual512MiB/swap0/oneCPU, with zero limit/OOM events,
zero Bend/proof executions, zero pass seeds and zero paid cost. These measure
metadata only, not cold/warm checks or PR timing; no speedup/memory-reduction
claim follows from these small single-run differences. The old inherited
rusage high-water observation is archived separately and not used here.

The historical94 Python controls passed in2.812882s before the subsequent
Boat-only execution rule. They use mock compiler/guard/identity fixtures to
test routing, invalidation, stale plans, local MACs and advisory exit codes.
They are not mathematical or actual-Bend gate qualification. Routing-model
measurements likewise describe a shell fixture only. No test was rerun
locally after that rule. Fresh final-head Boat validation is still required
before this correction lands.

Fresh current-code controls ran on normal Boat as work1526433 /
job`j20261104035728-695281`, exact head
`5b3f524240022cc1b38ce4f5a6d994244514eb48`. All94 Python controls passed
in4.830s. Actual512MiB/swap0 cgroup peak109,281,280B, all limit/OOM events0;
process VmHWM40,720KiB. Whole worker12.96s,19credit-s/$0.00019; sandbox
deleted verified. The approved normal daily runner transferred one bootstrap
file; source came from immutable public blobless Git. Raw BOAT receipts and
`boat-controls.py` are preserved. Mock fixtures still mean no native Bend
or proof/gate qualification; actual-repository pass seeds remain0. The later
receipt-only commit changes none of those tested helper/source bytes.

The premerge transport dependency landed as PR906, merge
`6b24fd8773c52f0e3d20d2b1fe685da15fe5b6d8`. This branch was rebased onto it;
all four tested cache source/test files and their helper dependencies remain
byte-identical to the fresh Boat-tested5b3f52424 source. Both RESULTS tables
were retained when resolving their documentation-only rebase conflict.
Fresh normal-Boat merged-shape validation still remains required, using the
explicitly pinned approved daily runner; no runner/budget/guard exception.
