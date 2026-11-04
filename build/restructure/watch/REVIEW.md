# Watch planner boundary

Base: `2789dddeb86f159893ab835c8071d4094eab1310` (merged #888 plus #886/#887).
Ten real production definitions move from `server.bend` to `watch-plan.bend`.
Every declaration body and signature is byte-identical (`PRESERVATION.json`).
Only the import and qualified call sites change in the remaining server.
No effect sequencing, shell bytes, argument order, event type, or persisted value
changes. The planner imports Base alone; it cannot call back into the actor.

## Original guarantees and implication arguments

All four original law names, quantified inputs, equality targets and result
types remain. Their only statement change is the module qualification naming
the moved production function. No other existing law statement changes.

The new `watch_copy_gate_none` states, for **every** command list `xs`,
`Watch.argv.k(True{}, xs) == []`. The new `watch_live_gate_kept` states, for
every command list, `Watch.argv.k(False{}, xs) == xs`. Both have checked proofs.
Each original law now has an explicit composition proof invoking the first
law with the actual list built by its production planner:

| Retained original law | Substitution for xs |
|---|---|
| watch_copy_launch_none | `["-c", Watch.launch.sh(), "sh", Watch.dir(home,id), cwd, target]` |
| watch_copy_check_none | `["-c", Watch.check.sh(), "sh", kind, target, ident, Watch.dir(home,id)]` |
| watch_copy_stop_none | `["-c", Watch.stop.sh(), "sh", kind, target, ident, Watch.dir(home,id)]` |
| watch_copy_intent_none | `["-c", "mkdir -p \"$1\" && : > \"$1/intent\"", "sh", Watch.dir(home,id)]` |

For launch, expanding `Watch.launch.argv(True{}, home, id, cwd, target)` gives
exactly the gate applied to row 1. Universal gate elimination with that list
yields the original empty-command guarantee for arbitrary home/id/cwd/target.
For check, expansion gives the gate applied to row 2; the same elimination
yields the original guarantee for arbitrary kind/target/ident/home/id.
For stop, expansion gives row 3, proving the original guarantee for those same
arbitrary inputs. For intent, expansion gives row 4, proving the original
guarantee for arbitrary home/id. These are the actual checked proof terms in
`PROOF.bend`, not assumptions about callbacks or unchecked interfaces.

The actor's launch, poll, stop, intent, path and tail call sites all use the
moved implementation. Normalizing only the new `WATCHPLAN.` qualification and
removing the new import reconstructs the original server with exactly the ten
moved definitions removed. The stronger live-gate law additionally covers the
unchanged non-copy path.

## Measurements and limits

The four original laws' implementation import closure falls from 359 modules
to **one**, excluding Base. Explicit lemma composition adds two declarations
to each static paired law/proof closure (6 -> 8; intent 5 -> 7); the generic
gate laws each reach 3 declarations. The largest law in the whole program
remains 5,500 declarations. Main.boot's semantic declaration closure remains
25,401: this first leaf extraction does not solve the whole server.

Fresh normal Boat job `j20261104013154-1092717`, work `1491719`, used stock
Bend 2.0.36, SHA256 `02089dc0eed0fd5fd6d73c74cc9cffcb2c6638dd3b02ae83f7b8cc57fbe381ba`.
It generated the **complete canonical** 3,167-law / 128-shard partition and
executed the two real shards containing these six application laws. No slicer,
template laws, substituted implementation or cached pass was used.

| Canonical shard | All laws in shard | Worker time | Peak RSS | Cgroup peak |
|---|---:|---:|---:|---:|
| 098 | 18 | 0.298 s | 180,176 KiB | 165,904,384 B |
| 124 | 25 | 0.202 s | 146,728 KiB | 131,624,960 B |

Both pass the unchanged `bendok` verdict: native exit 1 with only unrelated
foreign font/window definitions listed, no failing typed proof. The raw output
is preserved in `098.json` and `124.json`; these are not claims of a fully safe
foreign-free proof book. All 452 canonical source hashes still match the tested
worktree. `RESULT.json` and `BOAT.json` bind the partition, tools and launcher.

The merged #854 native guard held the actual Bend flock in each worker's own
3,699,998,720-byte/no-swap scope. The separate controller scope was capped at
536,870,912 bytes, swap 0, peak 69,324,800 bytes, zero OOM/max events. Sum of
configured caps is 4,236,870,912 bytes (<4 GiB). Swap-in/out stayed zero for both
workers. The sandbox was deleted and verified gone; cost 56 credit-s/$0.00056.

The fresh before run in `../BEFORE-BOAT.json` failed with SIGKILL at the cap on
canonical shard000, which included the original watcher laws. The before and
after shard memberships differ, and the before launcher had no controller cap:
do not interpret these samples as a controlled whole-shard speedup or total-job
before/after qualification. Historical #871 sliced runs were local diagnostics;
#877 was an unsliced canonical Boat run using source compiler #856. Neither is
being relabeled as this stock-compiler application-law result.

Only two of 128 canonical shards ran here. Full proof join, runtime build and
full application tests remain unqualified. No merged gate, proof verdict,
foreign-code policy or source-coverage requirement has been changed.
