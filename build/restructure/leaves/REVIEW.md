# Server leaf boundaries

Base: `e77e560175612d3637ac0559f2c1923541af0aa8` (merged Watch boundary #890).
Nine real server definitions move to pairing-info, desktop-info and adopt-plan.
The production call sites now use these small modules. Signatures and bodies
are identical except the pairing object's update-helper module qualification.
That helper's four bodies are copied byte-identically from thin-view into
core/json-object, with the same JSON types and field ordering. The original
view module stays byte-identical. Its legacy helper copy remains for a later
view-module split; no forwarding layer or UI behavior changes in this step.

No persisted types, effect sequencing, private pairing policy, commands, output
bytes or canonical desktop identity rules change. The existing desktop identity
test imports the narrow real function, and its test body remains unchanged.
The scoped proof experiment driver now accepts an explicit identity/output so
later feature steps can reuse the same capped native execution protocol.

## Guarantee preservation

All original laws remain. Exactly seven statements change, only in the module
qualification naming their actual moved function. Quantifiers, inputs, result
types and equality targets are unchanged. `PRESERVATION.json` records the body
hashes, original statements checked and the unchanged view module.

| Retained law | Why its original guarantee follows |
|---|---|
| desktop_connection_frame_bytes_unchanged | The moved function's body is identical, so for every text/version/hub, the frame branch returns exactly text. The retained universal law still checks that function. |
| desktop_connection_close_bytes_unchanged | The identical closed branch preserves every original text for every version/hub; the same universal property is retained. |
| adopt_copy_none | The identical copy=True branch returns Nil for every record/handoff pair. Its original universal property is retained with the same result type. |
| pair_msg_kept | Expanding the moved from-function on the original fixture selects the real cache-hit branch. Instantiating the new universal pair_cache_hit law with the original kept value/link yields the original message. The checked composition proof makes this substitution explicitly. |
| pair_msg_other_link | The original fixture selects the real cache-miss branch. The new universal pair_cache_miss law, instantiated with those same inputs, yields the original fresh message. Its composition proof is explicit. |
| pair_msg_no_link_qr | The identical no-link predicate and QR dispatcher return the same empty string. The original concrete law is retained. |
| thin_pair_told | The parser, defaults, ordered puts and original JSON types are identical. The new helper has exactly the old helper bodies, so the retained original equality still proves the same pair/QR values, other fields and field ordering. |

The two cache branch laws quantify over every kept string and link. They prove
the small functions actually used by the production cache dispatcher. Four
more laws strengthen coverage without replacing an existing law: object puts
on arbitrary fields, puts on null, connected desktop snapshots for arbitrary
client text/version/canonical hub, and live adoption for arbitrary records.
All six new proofs check against the actual implementation; none substitutes
an unchecked interface or assumes a callback implements production behavior.

## Dependencies and actual proof measurements

Static reverse declaration reachability from the new modules selects all 13
affected laws, recorded in `AFFECTED.json`. This is a conservative lexical
analysis, not a compiler dependency trace or a gate. The original laws' import
closures shrink from the server's 359 modules to 5 for pairing, 2 for desktop,
and 15 for adoption (Base excluded). The global largest law remains 5,500
declarations; Main.boot still genuinely depends on the server as a whole.
`METRICS.json` contains the individual measured closures.

Normal Boat job `j20261104014902-1246854` / work `1497360` used stock Bend 2.0.36
SHA256 `02089dc0eed0fd5fd6d73c74cc9cffcb2c6638dd3b02ae83f7b8cc57fbe381ba`.
It generated the entire canonical 3,173-law / 128-shard partition, identity
`25f46c83c2d64d8ebcb88078006b54f0a5862e22c059609210721b2f84ca59b8`, and ran
the three original native shards containing all affected laws. No slicer,
template proof book, cached pass or alternative implementation was used.

| Shard | All laws in shard | Worker time | Peak RSS | Cgroup peak |
|---|---:|---:|---:|---:|
| 077 | 25 | 3.608 s | 431,604 KiB | 425,324,544 B |
| 105 | 25 | 0.780 s | 215,852 KiB | 203,083,776 B |
| 121 | 22 | 0.196 s | 168,868 KiB | 155,475,968 B |

All three returned exit 0 and printed ALL PROOFS CHECK. Each held the actual
native Bend flock through the merged #854 guard, with an effective worker cap
3,699,998,720 bytes and no swap activity. The controller cap was 536,870,912
bytes, swap 0, peak 68,694,016 bytes, zero OOM/max events. Configured caps sum
to 4,236,870,912 bytes (<4 GiB). All 456 canonical source hashes still match the
tested worktree. The sandbox was deleted and verified gone; cost $0.00029.

Three mapper tests, 14 canonical shard tests, law shape and diff checks pass.
This is scoped affected-law evidence (72 laws total in those shards), not a
full proof join, runtime build, desktop integration test execution, or global
memory-budget qualification. The failed initial stock shard000 remains a
failed baseline; historical local sliced results remain diagnostic-only.
No gate, checker, foreign-code verdict or residual-source requirement changes.
