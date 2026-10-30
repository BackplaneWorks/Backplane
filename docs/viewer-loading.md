Viewer asset loading
====================

`src/web/solid-assets.js` owns transport revisions and decoded CPU geometry.
`solid.js` owns WebGL resources, input, and presentation; its integration is a
separate change. Asset identity is the source key plus the exact triangle count
and mesh bytes. Two revisions of one filename cannot overwrite each other after
an asynchronous decode: every preview, success, and failure checks its owning
generation. Reset and eviction abort outstanding work.

The decoder yields before allocation and between at most 2,048 triangles or
roughly 6 ms of work (clock checked every 64 triangles). Progress describes
triangles actually decoded. Before bytes arrive, the renderer can only report
preparation/elapsed time. A partial mesh is never a successful cached revision.
Malformed/truncated/oversized wire payloads fail explicitly and can be retried.
While a new revision is decoded, a previous successful model may remain visible
with the renderer's stale label; a failed revision restores that successful model.

The CPU cache keeps at most eight source entries and targets 128 MiB. The current
source is pinned; it may temporarily exceed the cache budget while its previous
model and new model coexist. A single decoded model is limited to 128 MiB of
positions, normals and colours. Unpinned entries are evicted in least-recently
used order. Camera/selection/presentation snapshots are separately bounded to 64
keys and copied on read/write. The renderer qualifies state keys by thread scope;
geometry remains shared by source. `reuse(key)` records warm reuse explicitly.
This is a page-memory cache, not persistent browser storage.

`src/server/plot.bend` now uses the existing `FS.revision` effect for every live
source, including boards, STEP and GLB. This includes device, inode, size, and
nanosecond mtime/ctime. Disk export filenames include that exact revision and the
bare/with-parts mode; their revision is no longer reduced to a 32-bit hash.
STEP conversion staging files are promoted only after success, and rename
failure is an export failure. This does not watch a board's referenced component
model files separately: changing one without saving the board remains a cache
invalidation limitation. Disk export retention is unchanged.

Validation
----------

`bun test/tools/solid_assets_test.mjs` exercises progressive actual geometry,
event-loop yields, the reviewed board wire fixture, malformed/oversized payloads,
exact revision reuse, delayed stale successes/failures, reset, eviction, pinning,
retry, and independent cloned camera/selection snapshots.

`test/native/solid_revision_probe.bend` calls the real server source-stamp,
export-path and staging-promotion functions without a hub or CAD conversion.
`bun test/tools/solid_revision_test.mjs BUILD/solid_revision_probe` writes only
temporary files and verifies same-size saves within the same second for
`.kicad_pcb`, `.step`, and `.glb`, atomic file replacement, stable unchanged cache
identities, distinct bare/full paths, failed export preservation, and rename
errors. It passed against the compiled probe on 2026-10-27. Focused Bend checks
reported only the repository's expected foreign-effect proof exclusions.

The generic `scripts/build-js.sh` emitted the tiny probe in 1.856 s with 851 MiB
peak RSS, but its server-specific native-map optimizer then refused the small
program because it has no map lookup. Compiling that successful emit directly
with `bun build --compile` took 77 ms and the executable regression passed. No
global proof ran in this child; the parent owns the single combined proof gate.

Browser evidence, 2026-10-27
----------------------------

`test/tools/solid_loading_browser_test.mjs` serves only isolated production
renderer modules, stub localized strings, and read-only model bytes on loopback.
It runs headless Chromium 156 with SwiftShader, at 1000×700 and touch 390×844 with
reduced motion. No installed app, hub, live data or CAD document is changed.

The real saved board GLB contains 95 component meshes: 77,216 triangles produce
833,452 wire bytes. A real C5 ceramic capacitor selected from that scene has 284
triangles and 3,124 wire bytes. GLB SHA-256:
`702b15010a2622d580f5d6c139ceb74465a7a5d1b268f3fdacc2bec9e2c91d40`.
The rendering child's test-only `solid_glb_fixture.mjs` adapter takes 105 ms for
the full board and 5.3 ms for C5. Those times are fixture preparation, not a
measurement of the hub's Bend GLB parser or CAD export.

| Model/mode | First submitted frame after bytes | First GPU-visible pixels after bytes | Full progressive decode | Warm reopen to next frame |
|---|---:|---:|---:|---:|
| Board / desktop | 9.5 ms | 42.5 ms | 212.2 ms | 35.1 ms |
| Board / touch + reduced motion | 10.6 ms | 39.4 ms | 204.3 ms | 29.1 ms |
| C5 / desktop | 1.7 ms | 9.0 ms | 1.1 ms | 16.9 ms |
| C5 / touch + reduced motion | 7.8 ms | 11.2 ms | 1.6 ms | 16.9 ms |

The first board frame is verified nonblank: 851 visible pixels on desktop and
330 on touch. Forced GPU readback synchronizes the first frame and adds test
overhead to decode/input timing. Maximum scheduled input delay with that readback
was 35.8/23.5 ms; the earlier run without GPU synchronization measured 1.7/2.8 ms.
An unsliced decode of the same full wire took 23.5/22.6 ms in one blocking call.
Slicing trades total completion time for earlier geometry and responsive input.

Cold desktop WebGL context/shader setup took 165.9 ms; the later touch context
took 5.4 ms after GPU shader caches had warmed. Context setup plus local fixture
fetch plus GPU-visible board geometry was 213.4/49.1 ms. These are browser-stage
measurements, not cold file-to-screen promises. Remote transfer, cold CAD export,
and the hub's parse/wire generation are not measured here.

The actual browser checks also verify keyboard orbit, two-pointer pan/pinch,
camera/projection/exposure restoration across canvas teardown, identical cached
model reuse, and zero repaints at rest in both motion modes. The renderer's own
material/DOF tests remain separate. Runtime receipts and screenshots are written
to `build/solid-loading/`; generated evidence is not committed.
