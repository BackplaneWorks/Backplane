# Proof sharding and declaration slicing: negative result (lane 3, 2026-11-03)

Status: EXPERIMENTAL, UNQUALIFIED, UNWIRED to CI or the release gate. Does not claim full proof coverage.
Effect companions (`  import "./effects/x.js"` inside def bodies, plus quoted `#include` headers) are now copied byte-identically for kept declarations and verified; the set equals `scripts/bend-closure.py` on the sliced tree (test on `src/server/mailstore.bend`). Not yet independently reviewed.
Result: slicing alone did not reach 3.7 GB (221 -> 531 shards, 299 pass, 104 OOM-killed, > 300 Boat jobs). Laws, proofs,
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
  closure parse (test). (Superseded: effect companions are now handled, see above.)
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

## Exact-text binding (added after review of 0d04)
`verify_output` now compares, per declaration, a sha256 of the exact original text (law statements
and proof bodies included, trailing blank/comment lines excluded) from a separate plain scanner;
detects duplicates in original and output; requires every output import line to exist verbatim in the
original and every alias a kept declaration uses to still be imported; and requires the effect
companions (and their quoted headers) of kept declarations to be present, byte-identical and the only
non-`.bend` files. Same-name tamper of a law statement or proof body now fails (tests).
Original/output/reachability/effects coverage is still pending independent review: do not use for the
gate or cache.

## server.bend closure analysis (bounded: scope 3699998720 B, swap 0, peak 67 MB, no OOM)
359 modules, 33,331 declarations, 112,287 edges in the `server.bend` closure; largest strongly connected
component is 7 declarations and no `server.bend` declaration is in a non-trivial one, so mutual
recursion is not what keeps it whole. The cost is call depth: `Main.boot` reaches 25,399 declarations /
335 modules / 7.2 MB, `Hub.handle` 23,310; the median `server.bend` declaration reaches only 135
declarations / 52 KB and the 90th percentile 10,812 / 2.9 MB. Roots near the top of the call tree cannot
be separated from their callees by name-reachability slicing. 32 effect companions are in the closure.

## Exact limits of the merged prototype (after independent review of #871) and the controls added
Limits of `verify_output` as merged: it checks the emitted shard against a caller-supplied kept set, not
against an independently established full root/law set; `reach()` is name based, so a reference it cannot
resolve silently contributed no dependency; `effect_closure` followed only the named effect file and quoted
C/H includes, not JS-local requires or dynamic imports.

Controls now in `proof-slice.py` (all light, no Bend run; 22 tests):
- `verify_plan(root, shards)` rebuilds the canonical declaration set and the law set independently
  (`scripts/bend-closure.py` on `LAWS.bend`/`PROOF.bend` plus the separate plain scanner) and requires every
  canonical declaration kept by some shard, nothing kept that is not canonical, and every `law` named by
  exactly one shard (no missing, unknown or duplicate). On current sources: 43,544 declarations, 3,165 laws,
  221 shards pass. The first attempt, built from an out-of-date manifest, was REJECTED for
  `src/core/message-context.bend Context.units`, which shows the control works.
- `reach()` now raises on an alias-qualified reference it cannot resolve (no real source triggers it).
  Unqualified unresolved names (builtins, locals) are still not errors: that part stays name-based and is
  not proven complete.
- `effect_closure` follows relative specifiers of `.js` effect files through a comment/string-aware scanner
  (`js_dependencies`): each `require`/`import` must be a static import/`from` with a literal, or a call whose
  ENTIRE argument is one string literal. Concatenation (`require("./a.js" + s)`), template substitution, a comma,
  a conditional, `import.meta`, a variable or any computed form raises, so a literal prefix is never mistaken for the
  whole target (found by independent audit of #872). The scanner is light, not a JavaScript parser: it is conservative
  by rejection and its handling of regex literals and unusual syntax is untested beyond its unit tests. All current
  `src/**/effects/*.js` files pass.
Still open: this is name-based slicing, not a proof that the sliced program checks the same statements as
the whole; gate/cache use requires independent review. Deep roots (`Main.boot` reaches 25,399
declarations) are NOT addressed by slicing. Decision: investigate compiler-level bounded memory or
structural sharing with lanes 1/2/5; do not replace callee implementations by signatures or axioms inside a
proof unless the compiler supports a sound interface with an equivalence proof.
