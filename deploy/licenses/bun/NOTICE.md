# Bun in backplane-browser and backplane-step2glb

`backplane-browser` (the Browser tab's helper, tools/browser) and
`backplane-step2glb` (the STEP to GLB mesher, tools/step2glb) are each one
executable made by `bun build --compile` with Bun 1.4.4 (https://bun.sh, source
https://github.com/oven-sh/bun/tree/bun-v1.4.4; the version is pinned in
`deploy/bun-version`, and a release built with any other Bun is refused). Each holds two things:
the Bun runtime, and Backplane's program (JavaScript built from this
repository's `tools/browser` or `tools/step2glb`).

Bun is MIT-licensed. It statically links JavaScriptCore and parts of WebKit,
which are under LGPL-2 (some files under BSD licenses), and tinycc, which is
under LGPL-2.1. The other libraries it links, and their licenses, are listed in
`LICENSE.md` beside this file. That is Bun's own LICENSE.md at bun-v1.4.4.

## Source and relinking (LGPL)

- **WebKit / JavaScriptCore:** Bun 1.4.4 uses the WebKit at
  https://github.com/oven-sh/WebKit, commit
  `d6375c72affe0f70222cdcb45012b10f0bf3a9d5` (`WEBKIT_VERSION` in Bun's
  `scripts/build/deps/webkit.ts`). Bun's LICENSE.md, beside this file, says
  how to build Bun against a modified copy.
- **tinycc:** https://github.com/tinycc/tinycc, commit
  `05f0fafaa3be31e31d7b4b5c17dc60f62c991171` (`TINYCC_COMMIT` in Bun's
  `scripts/build/deps/tinycc.ts`).
- **Backplane's programs:** they are not linked into Bun's object code. Each is
  a separate JavaScript program that Bun runs, and its source is this
  repository at the commit in the archive's `COMMIT` file. To run one on a
  modified Bun, build it with that Bun:

  ```sh
  BUN=/path/to/your/bun scripts/build-step2glb.sh   # dist/backplane-step2glb
  cd tools/browser && /path/to/your/bun build.ts backplane-browser
  ```

`backplane-step2glb` also embeds OpenCascade through occt-import-js
(LGPL-2.1): their notices are in `licenses/step2glb/`.
