# MailProof.keeps empty-step proof repair

`Step.cat` checks empty steps before its general append case. The old proof
destructured only `Step{cs, os}` and applied `append_nil(cs)`. With both lists
still unknown, the earlier empty-left pattern prevented conversion of the
goal to that append equality. The repair splits empty changes/outputs and
nonempty changes first. It changes only the proof body: the existing law,
every quantifier, `append_nil`, `Step.cat` and all runtime code are unchanged.

## Actual Boat receipts

Before source `345a9f39546cd6eb0f7ea4544467628a054ea867`, tree
`bd44cf4e7b13e665fc4c262c329d0b6a77761aaf`: work1519147 /
`j20261104033726-517908`. After source
`957a50435fe3e018e4397746f6e0b10dd42828d4`, tree
`bcabcceded6d7d00079f5552ff0d054202816e90`: work1521004 /
`j20261104034300-564417`. Each fresh normal Boat VM regenerated the complete
128-way canonical plan, selected the first shard containing
`[PROOF.bend, MailProof.keeps]`, compared every emitted declaration block to
the original, and ran only that shard. In both plans it is037: 25 primary
laws and85 unchanged project source dependencies. The helper is also present
in044/045; neither was executed. This is the original canonical partition
with its whole module closure, not declaration slicing within those modules.

| Observation | Before | After |
|---|---|---|
| Actual checker/verdict | Exit1, typed mismatch at `MailProof.keeps` | Exit0, `ALL PROOFS CHECK`, repository verdict accepted |
| Checker wall | 59.0196s | 42.9686s |
| GNU peak RSS | 2,956,136KiB | 2,787,664KiB |
| Worker cgroup peak | 3,329,044,480B | 3,157,594,112B |
| Whole job / paid cost | 83.40s /92credit-s ($0.00092) | 67.84s /77credit-s ($0.00077) |

Both workers enforced memory.max3,699,998,720B and swap.max0, held the actual
native bend flock and passed the existing bounded-shard resource admission.
Controllers separately enforced536,870,912B/swap0, so combined hard limits
were4,236,869,632B, below4GiB. Both worker receipts record zero limit/OOM
events and unchanged source/tool identities. After-controller peak was
148,643,840B, also with zero events. Both sandboxes were deleted and verified.
These are separate single observations with different outcomes; the faster
after time is not a matched throughput/speedup claim.

The exact frozen #877/#856 loader is identified by checker commit
`fd932a32c3e2b1a305de7141c4beaec219443893`, patch SHA
`4405d272062f81d713743aab104af0c5943888b65dfd33d124b66492f7340d12`
and the source hashes in AFTER-STARTED.json. Bun1.3.10 baseline executable
SHA is `795b37d736a88d746b0e8bb5fcde92bf29bd0f9ac6a1dd1a950fd37440cf5822`.
The checker kernel `bend.ts` is unchanged. The fresh typed `{0n == 1n : Nat}`
control failed at its proof body in0.115s; its exact log/receipt is preserved.
The positive trace audit requires paired validated/completed batches,
contiguous event ranges, root closure completion, no whole-book fallback and
all expected filled proof keys, including `MailProof.keeps`. It found11,931
checked events and no missing expected keys. Existing10 saved-trace controls
were reused, not rerun. No pass cache entry was seeded.

Raw BEFORE/AFTER checker outputs, GNU times, executor provenance, selection,
inner execution and negative-control receipts are here. `profile.jsonl.gz`
preserves the raw positive trace; PROFILE-IDENTITY.json records its original
hash. `MANIFEST.json.gz` is the raw generated manifest; original SHA
`c3ca66064f463d0075b50142420761fe51ec2d4249e3190fbb1e91dd9dbbd359`.
`runner.py` is the exact Boat harness. Its small frozen compiler transport
bundle is retained in the local execution store, not copied into the repo;
its hash and every patched source hash are recorded. The existing public
checker metadata/patch provides the pinned upstream revision and postimages.

## Limits and failed preparation

This is an experimental source-loader verdict for one canonical shard. No
stock-binary, full proof join, workflow, release, install or app build is
qualified. The final evidence-only commit reuses the tested proof/source
bytes; it still requires a fresh Boat merged-law shape guard before merge.
No tests, compiler, proofs or merge-tree ran on the user's machine.

Preparation work1518687 / jobj20261104033551-503919 failed before any worker
or checker launch because the harness wrongly required a shared helper to
appear in only one shard. It generated the plan on Boat (18.21s,32credit-s /
$0.00032), then refused; the selector was corrected to the handoff's first
matching shard. It is not a proof failure or pass. Total MailProof experiment
cost including this preparation failure was201credit-s/$0.00201. No failed
proof execution was repeated without a source change.
