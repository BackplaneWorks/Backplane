# Proof sharding and declaration slicing: negative result (lane 3, 2026-11-03)

Status: prototype, NOT wired into CI or the release gate, NOT a qualification. Laws, proofs,
the checker and shared files are unchanged.

Exact cap for every measurement below: systemd scope `MemoryMax=3700000000` (kernel value
3699998720), `MemorySwapMax=0`, verdict through `scripts/bendok.sh`. Earlier measurements at
3,879,731,200 B are looser and do not qualify.

## Findings
- 128-way import-closure plan (`proof-shards.py`, 3157 laws): shards 000/018/034/037 and the
  single laws `watch_copy_intent_none`, `held_empty_passes_release`, `adopt_kept_never_carried`
  and others are OOM-killed. 1367 of 3157 laws import a shared core (far, notice, codec, ...)
  that is killed even as a one-law shard. Memory does not follow source bytes.
- Declaration slicing (`scripts/proof-slice.py`): shard 000 went from 10 MB/396 files to
  312 KB/52 modules, 165 MB, ALL PROOFS CHECK, false-statement and dropped-declaration controls fail
  as they must. 124/128 law shards passed; 4 OOM. Residual (unreached) declarations are 18,510 of
  43,469 and need their own shards.
- Two compiler facts found by experiment: a callee must precede its caller in a module (callee-first
  order, mutually recursive groups keep original order); `Laws.x` is also a local PROOF.bend name.
- Iterative bisection (halve any OOM/>2.5 GB shard, 4 rounds): 221 -> 531 shards (> 300 Boat jobs),
  299 PASS, 104 OOM-killed, only 6 single-root. Highest passing peak 3.55 GB. Does not converge.
- `server.bend` imported whole is OOM-killed; `far.bend` (442 MB) and `msgs.bend` (274 MB) are fine.

## Structural controls added after independent audit (this commit)
- Strict reader: any unsupported top-level line, import after a declaration, indented stray text,
  duplicate import alias or duplicate declaration raises. All modules in the canonical `.bend`
  closure parse (test). `.js` effect companions are not Bend modules and are not sliced or covered.
- Entry reachability: one import per source component of the kept import graph (a cycle is one
  component), then `verify_output` re-reads the written files with an independent scanner and
  requires every emitted module reachable from the entry, every import to resolve inside the shard,
  no unaccounted top-level line, and the declaration inventory to equal the kept set.
- Tests: `test/tools/proof_slice_test.py` (cycle, Base-only entry, unknown syntax, tampered output,
  union coverage, callee order). No Bend runs.

## Not done
No frozen manifest, planner/verify/join integration, CI or gate wiring, or full-gate claim. Residual
shards and the heavy closures (server.bend, shared core) remain unsolved by sharding alone; they need
compiler load/check memory work or a better closure model.

`proof-shards-budget-mode.patch` archives an unmerged byte-budget planner experiment against
`proof-shards.py` (superseded; does not converge either).
