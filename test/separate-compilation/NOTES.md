# Separate-compilation semantic fixtures (experimental, unwired, no soundness claim)

Small fixtures of what the stock checker does at module boundaries. They check nothing about the real tree and
qualify nothing. Run (inside a cgroup with own memory.max <= 3,700,000,000 and swap.max 0, else exit 75):

    systemd-run --user --scope -p MemoryMax=3700000000 -p MemorySwapMax=0 python3 scripts/sepcomp-fixtures.py run
    python3 scripts/sepcomp-fixtures.py list        # no compiler
    python3 -m unittest discover -s test/tools -p sepcomp_fixtures_test.py   # light, no compiler

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
