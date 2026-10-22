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

`scripts/test.sh` runs each standalone IO test and its matching
`test/tools/NAME.mjs` driver when present. JavaScript fixtures are emitted
and their drivers execute assertions; successful emission alone cannot pass.
Files with an IO `main` and a driver exercise both.

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
