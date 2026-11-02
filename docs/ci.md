# CI validation

CI runs the installed, checksum-pinned Bend 2.0.36 for type checking, proofs
and JavaScript tests. Tests use a bounded 1 GiB OS stack and a 512 MiB JSC
stack, matching `scripts/check.sh`. The test shards and the hub's build pin
Bun 1.4.3. Each of the four test shards uses two workers, leaving headroom
for the larger render tests on the 16 GiB runners.

The jobs are `check`, `proof` (joined as `laws`), `test`, `js` and
`js-native`. The hub has no C build. `js` emits it as JS once, cross-builds
`backplane-serve` for every target with `bun build --compile`, and runs the
Linux one through `scripts/js-smoke.sh` and the whole of `test/smoke.ts`.
`js-native` smoke-tests the other targets on their own runners: arm64
Linux and both kinds of Mac gate; Windows only reports until Bend's JS
runtime has Win32 syscalls (fast-hub J4). See
[the JS service](js-service.md).

## Nightly

`nightly.yml` calls `ci.yml` on main every day at 04:17 UTC (and by hand
with workflow_dispatch). The merge gate (`scripts/local-ci.py`) runs only
`scripts/fast-tests.txt`, so the nightly is where the whole suite
(`TEST_FULL=1`), the full type-check, the proof and the native smoke
runs are held green. It tags and publishes nothing; each run keeps the
hub for every target as the `backplane-serve-js-TARGET` artifacts for 7 days.
A red nightly means main broke something the fast set does not cover:
fix it on main, do not skip the test.

`scripts/test.sh` runs each standalone IO test and its matching
`test/tools/NAME.mjs` driver when present. JavaScript fixtures are emitted
and their drivers execute assertions; successful emission alone cannot pass.
Files with an IO `main` and a driver exercise both. The intentionally failing
`cache_identity_before_test.bend` is reserved for
`test/tools/mobile_cache_identity_gate.py`, which checks its exact negative
result. It is excluded from the positive suite.

## No C

Nothing in Backplane builds C any more: the hub, the benches and the frame
tools under `test/native` build with `scripts/build-js.sh` (any IO root
works). The patched native emitter (`scripts/native-compiler.py`, the
20-bit function-ID patch) and the split C build (`scripts/build-app.sh`)
are gone with the native window's C effects.

Run the focused guards with:

```sh
python3 -m unittest discover -s test/tools -p test_runner_test.py
scripts/check.sh
scripts/test.sh
scripts/build.sh
scripts/smoke.sh
```

## Lean Boat packets

For a focused Bend program, `scripts/ci-packet.py` uploads an explicit source
closure as one compressed, verified archive through `boat-job.sh`:

```sh
python3 scripts/ci-packet.py create \
  --root test/NAME_test.bend --extra scripts/bendok.sh \
  --manifest extra/NAME-packet.json \
  --env BEND_NO_TELEMETRY=1 \
  --env BUN_JSC_forceRAMSize=4294967296 \
  --env BUN_JSC_maxPerThreadStackUsage=536870912
# Use the manifest SHA printed above, never a partial/prefix restore key.
python3 scripts/ci-packet.py run --manifest extra/NAME-packet.json \
  --expect SHA256 --timeout 180 -- bend test/NAME_test.bend
```

Run it under Backplane `work_run` and end the turn. It uses the existing Boat
budget/admission/deletion controls, waits0 seconds for capacity and retains no
paid idle sandbox. A local transport cache under `.ci-packet/` is keyed by the
full manifest digest and verified before reuse. It is generated transport data;
do not commit it. It stays visible to Git so the existing Boat source-staging
interface can transfer it; all other files are explicitly excluded.

The packet records exact source bytes and modes plus the actual Bend binary,
bundled files and named-library hashes. Remote verification runs before and
after execution; mutations, unexpected source and incomplete uploads fail.
The command's exit status is preserved. Results in `build/ci-packet/` are
copied back by the existing runner. This does not cache or certify a gate pass.

Bend imports/effects and local C headers are discovered by bend-closure.py.
Dynamic JS imports, drivers and data dependencies need explicit `--extra`
entries. Only Bend/tool resources are bound automatically; external runtimes
used by drivers need their own pinned identity checks. A packet claiming
unrelated/missing coverage must not be used as a whole-project pass. Check
`build/ci-speed/RESULTS.md` for measured scope and limitations.

## Inspect an affected-test plan

```sh
python3 scripts/ci-select.py --base origin/main --out build/ci-speed/affected.json
```

This opt-in planner is advisory: existing local/nightly gates do not consume it.
It includes the required fast set plus changed Bend dependencies and opaque
roots. Unknown source/import/tool identities use full selection. Plans bound to
Git are revalidated against the current diff and test inventory; a failed run
removes its old output. Its model supports the pinned official Bend2.0.36
compiler/resources; upgrades/custom libraries select all until reviewed.

`python3 -m unittest discover -s test/tools -p ci_select_test.py` exercises the
skip logic with dependency mutations, a deliberately dropped affected test,
new/later edits, opaque IO and compiler/library changes. This plans only Bend
tests; it cannot replace check/proof/build or other-language gates. Required
integration and execution measurements are tracked in build/ci-speed/RESULTS.md.
