# The JS service

The hub (`src/server/main.bend`) also builds for Bun's JS runtime. bend emits it as one JS file, and `bun build --compile` turns that into a single-file `backplane-serve` for any target. This is the fast-hub program's path to a server with no C: C stays until the two gates in the program's plan pass.

```sh
scripts/build-js.sh src/server/main.bend dist/backplane-serve          # this machine
BACKPLANE_JS_TARGET=darwin-arm64 scripts/build-js.sh src/server/main.bend out/backplane-serve
scripts/js-smoke.sh dist/backplane-serve [dist/web]                     # --version, exit 2, /hello, the page
BACKPLANE_DEV_SERVICE_ONLY=1 BACKPLANE_DEV_SERVICE_JS=1 scripts/dev.sh stage DIR
```

## Why `bun build --compile`, not Electron's Node

Bend's JS runtime makes every syscall through `bun:ffi`: select, sockets, posix_spawn, the pty ioctls. Electron's Node has no FFI, so the hub stops at its lock with "Cannot find module 'bun:ffi'".

A compiled binary carries Bun itself, about 104 MB, plus the 10 MB program:

| Target | Size |
|---|---|
| linux-x64 | 115 MB (27.5 MB xz) |
| darwin-arm64 | 72 MB |
| windows-x64 | 125 MB |

`bun:sqlite`, with FTS5, is built in on every target, so there's no native module to ship.

Every target cross-builds from one Linux machine, each in under a second once the emit exists. Measured on 2026-10-21:

- `--version`: 115 ms (36 ms with `--bytecode`)
- `/hello` on an empty home: 0.6 s on linux-x64, 0.2 s on an M-series Mac

## The build

- **Emit.** About 45-50 s, 8 GB peak at bend's 4 GiB heap hint. The emit is kept in `~/.cache/backplane-bend/emit-js` under `scripts/build-js.sh --key SRC`, the hash of only what bend reads for that root (`BP_KEY_ROOTS`). A change that leaves the server alone reuses the last emit.
- **Stamp.** The version and commit go into a copy of the emit (`scripts/stamp-version.py`, which also handles `.js` files).
- **Native strings.** The emit is minified with names kept, and `scripts/js-native.py` then swaps Base's `String.cmp` and `Map.bit.go` for native ones, as for the web client. Base's `String.cmp` is not a tail call: on long strings it overflowed the hub's stack. The build stops if either swap isn't found exactly once.
- **Bake.** A release's Google client and telemetry credentials go into a private prelude: `scripts/google-header.py` and `scripts/telemetry-header.py` write `.js` when asked for it. The prelude is compiled in and deleted afterwards, so the emitted and kept JS never hold credentials. The JS twins `effects/google.js` and `effects/telemetry.js` read `globalThis.BACKPLANE_BAKED`. Test: `bun test/tools/js_bake_test.ts`.
- **Compile.** The compile minifies whitespace and syntax but keeps names (`--keep-names`), so a field crash's stack still names Bend's defs and the effects. It also turns off Bun's `.env` and `bunfig.toml` autoload, which is on by default, so the hub never reads a project's `.env`.

## Licenses

A compiled binary contains Bun (MIT). Bun statically links JavaScriptCore and WebKit (LGPL-2, with some BSD parts) and tinycc (LGPL-2.1). The emitted JS also contains Bend's runtime and Base (Apache-2.0).

`scripts/js-licenses.sh DIR` writes both sets of notices into `DIR/bun` and `DIR/bend`. The texts live in `deploy/licenses`. Bun's is its own LICENSE.md for the pinned version, and both texts are checked by hash. `NOTICE.md` says where the LGPL source is (Bun's WebKit commit) and how to run the program on a modified Bun (`BACKPLANE_JS_BUN_RUNTIME`). Strict mode (`BACKPLANE_LICENSES_STRICT=1`, as in CI) stops when the Bun in use isn't the version the notices name. `dev.sh stage` and CI stage the notices.

## Releases and the desktop app

`scripts/service-release.py js` is the release producer's JS route. The request has kind `backplane-service-js-inputs`, and its toolchain pins:

- `bend`
- `bun`
- `bunRuntime`: the target's own Bun executable, pinned by sha256 and handed to `--compile-executable-path`

Because of that pin, a target can be built from any host. `--version` runs only when the host is the target; runtime acceptance runs the binary on its target as before.

BUILD.json gains `serviceRuntime: "bun"`. Everything downstream is unchanged, and the Electron app launches it as before as `resources/backplane-service/backplane-serve`. That covers `component`, `desktop/produce-release.cjs` (an ELF or Mach-O of the target's architecture), the runtime closure and `desktop/service.cjs`.

The JS targets are linux-x64, linux-arm64, darwin-x64 and darwin-arm64. Windows is missing because Bend's JS runtime has no Win32 syscalls yet. A Windows binary builds but can't serve, so the Windows producer stays on C.

## CI

The `js` job (`.github/workflows/ci.yml`) does the following:

- emits the hub once, with the emit cached by its key;
- builds linux-x64, linux-arm64, darwin-x64, darwin-arm64 and windows-x64;
- smoke-tests the Linux binary;
- runs the full `test/smoke.ts`;
- runs the bake and producer tests;
- uploads `backplane-serve-js`.

`js-native` runs the smoke test on arm64 Linux and on both kinds of Mac. Windows only reports.
