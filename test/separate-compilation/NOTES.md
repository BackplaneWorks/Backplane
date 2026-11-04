# Separate-compilation semantic fixtures (experimental, unwired, no soundness claim)

Small fixtures of what the stock checker does at module boundaries. They check nothing about the real tree and
qualify nothing. Sources are stored as `*.bend.txt` so `scripts/check.sh` (type-checks every tracked `*.bend`) and the
content-cache / inputs-key inventories never ingest these deliberately failing modules; the runner materialises a
case in a temporary directory with the suffix removed.

    python3 scripts/sepcomp-fixtures.py list        # no compiler
    python3 -m unittest discover -s test/tools -p sepcomp_fixtures_test.py   # light, no compiler

`run` executes the real compiler and therefore REFUSES (exit 75) unless the process (1) inherited the actual native
bend flock (BACKPLANE_HEAVY_LOCK_FD on /tmp/bp-bend.lock), (2) has its own cgroup with memory.max <= 3,700,000,000
and swap.max 0, and (3) passes heavy-guard admission (3 GiB floor and host reserve). The supported invocation is the
merged `fixture-check` profile (PR #892), which takes the flock itself and admits exactly this argv:

    systemd-run --user --scope -p MemoryMax=3700000000 -p MemorySwapMax=0 python3 scripts/heavy-guard.py fixture-check -- python3 scripts/sepcomp-fixtures.py run

Anything else is refused and there is no fallback. The runner sets BEND_NO_TELEMETRY=1 for the compiler itself.

Two kinds of evidence, kept apart:
- **Earlier, unadmitted:** the first 15-case result and the strace traces came from a local capped scope
  (MemoryMax=3700000000, swap 0, no flock, no guard admission), compiler env not set to no-telemetry.
- **Later, admitted:** one run through `fixture-check` (native flock taken by the guard, cgroup memory.max
  3699998720, swap 0, peak 92,725,248 B, oom_kill 0), 15/15 cases matched, on a local merge of main into the #891
  head 8f8400970 with the same script/fixture bytes. Receipt: bp-coord/lowmem-swarm/lane3/fixture-exec-receipt/.
  It has not been repeated on the rebased head. Saved traces: bp-coord/lowmem-swarm/lane3/fill-order-evidence/.

Compiler identity: `bend` 2.0.36 (binary sha256 b2b183bddc3c3fa4bc45d99f6a6bdbb192ebc616e6e697777f49e74c545b1643);
verdicts use scripts/bendok.sh (sha256 prefix 3f3e17c4ba1a0d90). Each case: `bend cases/<name>/MAIN.bend --check-only`.

## Observed (all 15 cases match expected.json at cap 3699998720 B, swap 0, cgroup peak 92 MB)
- A consumer proof depends on a dependency's BODY (conversion): changing or replacing `A.two` breaks `B`.
- A law without a fill is unusable; a law is usable once some checked def fills it.
- Import cycles, mutual recursion and a call to a def written below are rejected; `@unsafe` defs may do
  these but are named in the unsafe notice and the taint reaches their callers.
- **Fill visibility depends on load order, not only imports.** `U` uses `Laws.L()` and does NOT import the filler `P`.
  Entry loads `P` first: pass. Same files, entry loads `U` first: fail. Filler on disk but never imported: fail.
  Loaded-file order (strace openat, O_NOCTTY reads): `MAIN, base.bend, P, L, U` vs `MAIN, base.bend, U, L, P`.

## Retracted observation (my earlier fixture mistake)
An earlier run seemed to show an implicit sibling `P.bend` being found. That fixture's `U.bend` itself contained
`import ./P.bend as P`; renaming or deleting `P.bend` simply broke an explicit import. No implicit sibling lookup
is established: the `filler_never_loaded_fails` case (filler beside, not imported) fails.

## Not covered
Fills in two places, a fill inside a dependency, a filler reached only through another module, a law whose
statement unfolds another law, compositional-law obligations, unsafe code beyond the flag fixtures, canonical
LAWS/PROOF behaviour. Any cache or interface design must key on the actual ordered fill context.
