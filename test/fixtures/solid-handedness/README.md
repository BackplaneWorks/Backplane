The WebGL viewer omitted the board-wire to right-handed-world conversion
that both phone renderers already apply. `Solid.of` intentionally sends
board axes (x right, y down, z up). Using those positions unchanged in a
right-handed WebGL orbit reflects the whole model: front and back text and
graphics, pads, and the outline. Direct native GLB renders bypass this wire
and therefore did not expose the bug. PR430's keep-upright fix is separate.

`src/web/solid.js::mesh` now converts each received vertex to (x, -y, z),
then computes its normals and bounds from those positions. All layers and
parts receive the same conversion exactly once. No text mirror setting,
footprint rotation, wire encoding, GLB export/cache, UV, shader or camera
rule changes. The mobile board overlays and picking retain their wire
coordinates; their existing MVP conversion remains unchanged.

`wire.json` freezes the real GLB→Solid.thin→Solid.of output for
`test/fixtures/pcb-bottom-silk/asymmetric.kicad_pcb` (source and wire hashes
included). The board has different front/back labels and graphics, offset
rotated footprints, pads and holes, a chamfered corner, and intentional
unmirrored and 180-degree text. It is the existing isolated reference board,
not a modified user board. KiCad 10.99.0-bp.4 exported it with
`pcb export glb --no-components --include-pads --include-soldermask --include-silkscreen`.

`before-top.png`/`before-bottom.png` and `top.png`/`bottom.png` are actual
headless WebGL output from the production viewer body before/after the fix.
The camera is set to the same top/bottom poses. The corrected orientation was
visually compared with KiCad's SVG and direct GLB output. `UNMIRRORED` remains
backwards and `BACK180` retains its authored upside-down angle from below.
These controls prevent a blanket per-layer text mirror or keep-upright fix.

Run the bounded checks from the repository root:

```sh
bun test/tools/solid_mesh_test.ts
bun test/tools/orbit_test.ts
bun test/tools/solid_mesh_render_test.mjs
# After the coordinator rebuilds/installs the frontend:
bun test/tools/solid_mesh_bundle_test.mjs PATH/TO/chunk-HASH.js
```

The render test needs playwright-core installed in tools/browser/node_modules
(or PLAYWRIGHT_CORE_PATH pointing to its index.mjs) and Chromium (CHROME_BIN
if it is not at a standard Linux path). It starts an isolated loopback server
and headless browser, uses the exact production decoder/shaders/camera/draw
body with only a test export to set the view, and stubs progress strings.
It exercises `mount`/`got`, compares top/bottom white silk coverage against
the reviewed output (IoU >= 0.95), and saves actual PNGs plus source/wire/browser
identity to build/solid-handedness. At verification, both scores were 1.0.
The unchanged pre-fix source fails this regression (top silk IoU 0).

Issue #45 was rechecked on `origin/main` at `a8339af0f` on 2026-11-02.
The production WebGL renderer still shows readable `TOP F12` and
`BOTTOM R37`, correctly placed asymmetric pads and the chamfer, and the
authored reversed/rotated back text. Both faces match these reviewed silk
masks exactly (IoU 1.0). The test now serves the renderer's mesh-worker
dependency and pins the original reference camera distance: production Fit
has since changed its framing, independently of handedness.

PR #57's encoder negation was read as a reference and was not applied.
The wire remains in board axes; WebGL's decoders and the phones' MVPs
already negate Y. As a negative control, negating the frozen wire's signed
Y deltas (the encoder change proposed in #57) makes the top render fail
with silk IoU 0. Adding that negation would restore the reflection.

The numerical check covers signed coordinate deltas, correct normal winding,
colours, bounds, front/back orientation, and camera panning. The existing
orbit test covers drag, zoom and pan behavior. No live UI or device is driven.

A read-only snapshot of the user's reported board also reproduced the
reflection through this same GLB→wire→WebGL path. Corrected images were
inspected against its KiCad SVG reference; source GLB and wire bytes were
unchanged. Private board artifacts and exact deployed-asset identity were
supplied separately to the integration coordinator. An extracted decoder
from the actually installed frontend reproduces the missing conversion;
source-only passing images do not imply live adoption.
