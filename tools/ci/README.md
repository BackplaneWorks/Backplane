# Fast JavaScript and TypeScript tools

Pinned development commands for CI/pre-push integration. Install from the lock:

```sh
BUN_CONFIG_NO_ANALYTICS=1 DO_NOT_TRACK=1 bun install --ignore-scripts --frozen-lockfile --cwd tools/ci
bun run --cwd tools/ci typecheck:step2glb
bun run --cwd tools/ci lint ../../src/web/FILE.js
bun run --cwd tools/ci format:check ../../src/web/FILE.js
bun run --cwd tools/ci test
```

`lint` enables Oxlint's correctness and suspicious categories as errors and
rejects warnings. `format:check` is read-only. `typecheck:step2glb` checks the
existing strict project and its transitive imports with native TypeScript7;
`strict`, `noUncheckedIndexedAccess` and `noEmit` remain enabled. The project's
`allowImportingTsExtensions` setting accepts its existing Bun `.ts` imports.
Bun types are pinned to1.4.3, matching the CI runtime pin.

The full strict lint profile currently reports existing diagnostics in the
application. Review those before enabling a global gate. Other TypeScript
sources still need project configurations and coverage. These commands are
opt-in; pre-push integration waits for the measured UI validator and its
accepted snapshot/budget contract.

## Duplicate parameters

Oxlint1.90.0 does not implement ESLint's `no-dupe-args` for sloppy CommonJS
functions. A real negative fixture exposed that omission. The local plugin
preserves the check, walking parameter bindings and keeping nested function
scopes separate; unsupported binding shapes fail the run. The tool test checks
all eight comparison rules, positive binding patterns, a real strict type error
and formatting controls. Upgrades must keep those controls passing.

[Oxlint's JS plugin API](https://oxc.rs/docs/guide/usage/linter/js-plugins.html)
is currently alpha, so Oxlint and the plugin source are pinned inputs. There is
no blanket claim of equivalence to all ESLint rules.

## Reproduce measurements

```sh
uv run --no-project python scripts/ci-tool-bench.py --prepare
```

`--prepare` installs exact ESLint/TypeScript6 comparison versions into `build/`
with install scripts disabled. Five fresh processes per tool measure the same
JavaScript files/eight-rule profile and the same strict TypeScript project.
The native lint profile includes the duplicate-parameter plugin. The complete
strict lint/format results remain failures when existing diagnostics exist.

Receipts include source/config hashes, pinned locks and actual runtime/native
implementation hashes in `build/ci-speed/fast-tools/`. Peak RSS is the largest
descendant process, not aggregate concurrent memory. All deliberate errors and
format writes live in disposable fixtures. No application is built or run.
