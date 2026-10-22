# Bun in backplane-serve

`backplane-serve` is one executable made by `bun build --compile` (Bun 1.4.3,
https://bun.sh, source https://github.com/oven-sh/bun/tree/bun-v1.4.3). It holds
two things: the Bun runtime, and Backplane's program (JavaScript emitted by Bend
from this repository's `src/server/main.bend`).

Bun is MIT-licensed. It statically links JavaScriptCore and parts of WebKit,
which are under LGPL-2 (some files under BSD licenses), and tinycc, which is
under LGPL-2.1. The other libraries it links, and their licenses, are listed in
`LICENSE.md` beside this file. That is Bun's own LICENSE.md at bun-v1.4.3.

## Source and relinking (LGPL)

- **WebKit / JavaScriptCore:** Bun 1.4.3 uses the WebKit at
  https://github.com/oven-sh/WebKit, commit
  `0c06faadf65bf8e8c8ad3a5a8aca83e1e9ed653f` (`WEBKIT_VERSION` in Bun's
  `scripts/build/deps/webkit.ts`). Bun's LICENSE.md, beside this file, says
  how to build Bun against a modified copy.
- **tinycc:** https://github.com/tinycc/tinycc, the version Bun 1.4.3 pins.
- **Backplane's program:** the program is not linked into Bun's object code. It
  is a separate JavaScript file that Bun runs, and its source is this
  repository at the commit `backplane-serve --version` names. To run it on a
  modified Bun, rebuild it with that Bun:

  ```sh
  BACKPLANE_JS_BUN_RUNTIME=/path/to/your/bun scripts/build-js.sh src/server/main.bend backplane-serve
  ```

  This uses `bun build --compile --compile-executable-path`.
