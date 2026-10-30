# Canvas surface and camera presentation

`src/web/solid.js` draws the actual Solid triangles; `solid-materials.js` supplies
studio shading; `solid-dof.js` supplies optional depth-dependent lens filtering.
The web and Electron use this canvas. This does not change native phone shaders,
CAD documents, model exports, model caches, or saved design history.

## Color and provenance

The GLB loader quantizes `baseColorFactor` directly into RGB bytes. Those factors
are linear RGB ([glTF material contract](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#materials)).
The canvas now lights those values in linear space and applies the sRGB transfer
at output, with an exposure control and a highlight shoulder. It preserves the
received vertex colors, positions, winding, normals and front/back axis mapping.
This fixes the old display-space diffuse multiply; dark model colors may become
more visible without any source recoloring.

The compact wire does **not** retain material IDs, metallic/roughness factors,
opacity, textures, original vertex normals, or per-component model identity.
Surface response is consequently a **color estimate**. Warm neutral terminations
and saturated copper/gold receive a reflective response; dark neutral polymer,
brown ceramic/substrate, colored mask and white silk receive distinct roughness
and coat estimates. Ambiguous gray plastic/metal and white plastic/silk cannot be
identified exactly. `cameraState().materialSource` always says `color-estimate`.
No missing geometry is replaced with invented bodies. The thermal hook applies
an illustrative display tint; it predicts neither temperatures nor solder flow.

Lighting uses two camera-relative studio lights, an analytic reflection studio,
GGX highlights and a restrained dielectric ambient reflection. It needs no remote
textures or assets. Face normals remain authored triangle normals: no smoothing
across true package corners or artificial bevel geometry.

## Controls and state

`cameraAction(action,value)` supports `fit`, `iso`, `top`, `bottom`, `front`,
`back`, `left`, `right`, `projection` (`perspective`/`orthographic`), `focal`
(15–250 mm), and `exposure` (-3–3 EV). Focal changes preserve framing at the
pivot. Orthographic views retain the same frame scale.

When supported it also accepts `aperture` (`off` or f/1.4–32), `focus` (mm), and
`focus-lock` (toggle). Focus follows pivot distance until locked. `cameraState()`
returns projection, focal length, exposure, material provenance, thermal state
and selection; supported lens fields are aperture, focusDistance and focusLocked.
`setThermal({enabled,temperature,liquid})` updates one frame.

Canvas keyboard controls: F/Home fits; 0–6 selects iso/top/bottom/front/right/back/
left; arrows orbit, Shift+arrows pan, +/− zoom. Existing mouse, touchpad and two
finger gestures are preserved. `solid-camera-change` bubbles after controller or
pointer/keyboard updates. There is no idle repaint loop.

Camera, exposure, lens, thermal and plain selection snapshots are keyed by
`canvas.dataset.scope` plus source key, so two threads can show the same asset
without sharing their camera. The bounded `SolidAssets` store owns immutable
source/revision decoding and reuse. Same-source progressive updates retain the
pose; optional full wire bounds stabilize the first frame. Failed refreshes
explicitly label any retained previous revision. Remounts release listeners,
observers, GPU buffers/programs and optional lens targets.

`projectPoint([x,y,z])` accepts board millimetres and returns canvas-relative CSS
`{x,y,depth,visible}` (depth in mm). `rayToBoard(x,y,z=0)` returns board millimetres
or null for an edge-on/behind-eye intersection. Both use the exact perspective or
orthographic camera. `setSelection(plain)` retains saved-PCB selection metadata;
these APIs never invent a component-to-triangle association. The Solid wire uses
10 micrometre units, so board millimetres map to `(x,-y,z)*100` in WebGL.

## Depth of field and limits

The optional pass uses `WEBGL_depth_texture` and fragment high precision. Missing
capabilities omit aperture controls and log a specific reason; the normal canvas
continues to work. Perspective depth is reconstructed from the actual depth
texture. Focal length, aperture and focus distance determine each pixel's circle
of confusion. Two depth-aware separable 16-tap passes approximate the lens blur
in linear color; orthographic views bypass it. It is a screen-space lens
approximation, not path tracing, measured optics or a simulation of hidden
surfaces. Radius is capped at 8 pixels; the filter avoids background color spilling
across a foreground silhouette. Extreme closeups at distances below the virtual
focal length use a bounded circle approximation.

Lens rendering is off by default. Its targets are capped at 1536 pixels on the
long side: two RGBA8 textures plus depth16 cost at most 23.6 MB (22.5 MiB), in
addition to browser/default framebuffer and geometry memory. At 1000×700 the
extra targets cost 7 MB. Targets allocate on first use and are released at canvas
disposal. Depth16 and the non-multisampled offscreen pass can soften or alias very
fine silk; the normal sharp renderer retains browser antialiasing. This is not a
claim of identical mobile GPU performance.

## Reproducible evidence

Run bounded source tests:

```sh
bun test/tools/solid_materials_test.mjs
bun test/tools/solid_mesh_test.ts
bun test/tools/orbit_test.ts
bun test/tools/solid_mesh_render_test.mjs
bun test/tools/solid_materials_render_test.mjs
```

Browser tests require `tools/browser/node_modules/playwright-core` (or
`PLAYWRIGHT_CORE_PATH`) and Chromium (`CHROME_BIN`). They start isolated loopback
servers/headless browsers and never drive or reload a live app. The mesh/silk
regression compares against the previously reviewed asymmetric front/back board.
The material test checks the exact production shader/decoder/draw path,
reversible exposure/thermal tint, orthographic/focal controls, focused-vs-blurred
depth planes, unsupported-depth fallback, board projection/ray inversion,
keyboard interaction after remount, per-thread pose/selection and idle behavior.

For real populated-board and actual component before/after images, set
`SOLID_GLB` to an existing read-only GLB, `SOLID_COMPONENT` to a node name
(default C5), and `SOLID_BEFORE_SOURCE` to the original renderer source.
`SOLID_RENDER_OUT` chooses an evidence directory. `solid_glb_fixture.mjs` is a
read-only test adapter for the existing GLB scene's TRS/matrices, indices, linear
base colors and Solid axis/coordinate quantization. It does not run CAD exports
or modify the source. Receipt JSON records source/material/GLB/wire hashes,
browser identity, triangle counts/bounds, images and measured checks. Private
cached models/screens are not committed as test fixtures.

The reviewed real model had 77,216 triangles; its actual C5 capacitor had 284.
Before/after wire hashes, triangle counts and bounds are identical. Captures
show the board and component at the same poses, plus a separate optional lens
view. Top/bottom white-silk coverage retains IoU 1.0 against the orientation
fixture. In the isolated 1000×700 SwiftShader run, three readback-inclusive sharp
frames took 24.0–25.2 ms and lens frames 20.0–20.6 ms; those short wall-clock
samples include rasterization/readback and do not isolate GPU time. The lens
path also omits MSAA, so its relative timing is not a quality-equivalent speedup.
See `viewer-loading.md` for progressive geometry and input-latency evidence.
