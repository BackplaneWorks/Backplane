# Original-model quality rendering

The compact Solid preview appears progressively after geometry arrives. The optional quality renderer uses
an authenticated, self-contained original GLB, preserving source geometry,
vertex normals, embedded textures, colors and node hierarchy. The default `cad-studio` policy preserves explicit PBR and alpha and fills
only roughness/metalness fields proven absent in the original JSON using documented
color estimates. Status then says `original-glb-with-estimated-missing-factors`,
with `missingFactorCount` and `estimatedFactorCount`. The optional `source-defaults`
fidelity policy retains glTF's default metalness=1/roughness=1 instead. Neither
policy claims measured physical properties; colors, alpha, normals and textures
remain authored. `setMaterialPolicy(policy)` switches reversibly without changing
the source asset bytes.

## Local pinned dependencies

`tools/viewer/package.json` and `bun.lock` pin Three 0.181.1,
three-gpu-pathtracer 0.0.24, three-mesh-bvh 0.9.5 and xatlas-web 0.1.0.
The [0.0.24 upstream API](https://github.com/gkjohnson/three-gpu-pathtracer/tree/v0.0.24)
supports WebGL2. The explicit compatible pin and local bundles avoid silently
changing renderer requirements or fetching dependencies from a CDN.

```sh
(cd tools/viewer && bun install --frozen-lockfile --ignore-scripts)
bun tools/viewer/build.mjs dist/web/viewer/vendor
```

Build emits `viewer.js`, `render-worker.js`, `bvh-worker.js`, `manifest.json`
(version/byte/SHA receipts) and `LICENSES.txt`. The generated bundles are build
outputs, not checked-in source. All URLs default to `/viewer/vendor/`; each can
be supplied to the factory when the authenticated hub uses another route.

## Integration contract

`createPathTraceView({canvas,vendorUrl,renderWorkerUrl,workerUrl,onStatus,onSelect,
maxSamples=32,maxMs=30000,maxPixels=2000000,materialPolicy='cad-studio'})` returns a promise for a controller.
The canvas must be new: it transfers to OffscreenCanvas and cannot already own
the immediate renderer's WebGL context. An overlay canvas can keep pointer input
on the immediate canvas; the caller can use `pick` explicitly.

Controller methods:

- `load({url,key,source,revision,sha256,orientation,signal})`: original same-origin
  GLB. When `sha256` is supplied its actual bytes must match before parsing. `gltf-y-up` means metres and rotates `(x,y,z)` to `(x,-z,y)`;
  `step-z-up-mm` means Z-up millimetres, scaled by 0.001. Both match the Solid
  wire's final world. Raster mode loads and presents without allocating a path
  tracer or building ray acceleration. Fetch/parse/build errors give explicit
  status; preview stays usable.
- `setCamera(snapshot)`: full `Solid.cameraSnapshot()` shape (`cam` in 10um,
  `projection`, `exposure`, optional aperture/focusDistance in mm). This helper
  never independently fits or orbits after receiving the shared snapshot.
  Identical physical camera snapshots do not restart accumulation.
- `setInteractive(true/false)`: raster while moving, restart finite convergence
  after settling. `setMode('raster'|'pathtrace',{samples})`: 1–256 samples.
- `status()`, `diagnostics()` (async), `dispose()`.
- `present(denoised=true)` (async): display the current accumulation with or
  without guided denoising, without taking more samples. Useful for honest
  matched-camera raw/filtered comparisons.
- `setComponents(data,onSelect)` (async), `selectComponent(uuid)`,
  `pick(xCSS,yCSS)` (async), `componentBounds(uuid)` (async).

Component data must have matching canonical `source` and `sourceRevision`
(`revision` alias accepted) plus `components:[{id,reference,...}]`. Only unique
exact reference names in both sidecar and original scene map to UUIDs. A mesh
inherits its validated reference ancestor; `=>...` exporter names, mesh indices,
RGB and spatial proximity are never treated as identities. Picking intersects
all actual scene meshes first, so foreground board/package geometry occludes
components behind it. Selection adds a presentation box without changing CAD or
source materials. `componentBounds` returns actual transformed board-mm
`{id,min:[x,y,z],max:[x,y,z],source,revision}` or null. Camera framing remains the
shared controller's responsibility. Unmatched/stale/duplicate metadata cannot
identify a body.

Status phases are `loading`, `building`, `raster`, `pathtrace`, `converged`,
`paused`, `unsupported`, `error`; status includes actual samples, source identity,
triangle/material/normal counts, and `materialSource:'original-glb'`.
`visible:false` remains set through initialization/loading until actual raster GPU readback differs from a separate background-only render under the same camera and color management. Positive source triangle count is required; selection overlays do not count. A subsequent worker frame forces `visible:true,firstFrame:true` with the current key/revision; a superseded generation cannot reveal a replacement canvas. `denoised:true` specifically identifies the final guided display. Reports are
throttled to 100 ms while a phase is unchanged; final/phase changes report at once.

## Threading, budgets and quality

The main-thread facade owns only messages, input state, resize/visibility and
callbacks. GLB parsing, raster shader compilation, geometry merging, acceleration
build, ray samples and denoising run in a dedicated OffscreenCanvas worker.
The first explicit path-tracing request starts one generation-bound build;
repeated requests share that promise. Raster remains visible during the build.
Moving or switching to raster prevents automatic ray samples. Picking and bounds
already work on original meshes without ray acceleration. Diagnostics expose
`traceAllocated`, `bvhBuilding` and `bvhReady`.
The acceleration build itself uses a second BVH worker. A missing threaded/WebGL2
capability fails explicitly to the existing preview; there is no blocking
main-thread quality fallback.

The default path pass is five bounces, four transmissive bounces, adaptive 3–6 tiles per axis,
32 samples and 30 seconds. Samples and timeout are bounded, convergence stops
requesting frames, and hiding/disposal stops work. A time-limited partial image
is labelled `paused`, never converged. Resolution is bounded by maxPixels and a
DPR ceiling of 2. The default two-million-pixel ceiling preserves native CSS
resolution at normal desktop sizes; an explicit smaller budget remains honored.
Status exposes CSS/render dimensions, actual pixel ratio and `upscaled` so a
low-resolution target cannot masquerade as native pixels. Original bytes are
limited to 96 MiB and geometry to one million triangles. Texture arrays use
1024px slots. These are practical memory limits, not
an exact GPU-memory guarantee; source models with many textures can still exhaust
a browser GPU process. Disposal terminates workers and releases renderer targets.

A budget-limited partial target with at least one real sample also receives guided denoising, retains its actual fractional sample count and stays labelled paused.

The final filter uses three bounded 7×7 HDR passes at pixel strides 1/2/4, guided
by separate raster depth, view-space normals and source albedo. Positions are
reconstructed at exact guide texel centers using inverse projection; point-to-plane
distance rejects distinct surfaces without rejecting a sloping flat board.
Normal and authored color edges protect surfaces and lettering. The final pass
alone applies display tone/color transforms.
The accumulation buffer stays intact. This is guided denoising, not extra ray
samples or a path-tracing substitute. At low samples it can retain visible grain;
only absent roughness/metalness fields may receive the explicitly reported CAD studio estimates; authored factors are never retuned.

External GLB dependencies are rejected rather than fetched from relative/CDN
locations or silently dropped. Embedded data/blob images are allowed. Unsupported
compression codecs surface loader errors; they are not displayed as a gray
successful model. The studio environment and area lights are local presentation
lighting, separate from the authored model. The Z-up environment has a broad
neutral key, weaker fill, a narrow rim and dark regions for reflection contrast.
Default fitting projects actual bounding-box corners and fills the frame;
an external shared camera snapshot remains authoritative.

The reviewed source GLB contains flat pads and package terminals but no solder
fillets. No solder geometry is created by this source renderer. Extra samples
cannot supply missing joints or make estimated material factors measured data.

## Isolated regression and evidence

`bun test/tools/solid_pathtrace_test.mjs /absolute/read-only-board.glb` starts a
loopback HTTP server and separate headless Chromium. Build the vendor outputs
first into `build/viewer/vendor`. It verifies real accumulated ray samples,
actual raster/path-traced pixel differences, worker input latency, finite idle,
normal preservation, real body picking/bounds, duplicate/revision rejection and
disposal. It changes no model, cache, CAD document or live app.

Environment options: `PATH_SAMPLES`, `PATH_PIXELS`, `PATH_WIDTH`, `PATH_HEIGHT`,
`PATH_VENDOR_DIR`, `PATH_COMPONENT` (the reviewed C5 closeup), `PATH_TRACE_OUT`,
`PLAYWRIGHT_CORE_PATH`. Captures include raw and denoised whole frames and crops
from the same accumulation and camera. JSON receipts record original
GLB/source/vendor hashes, browser identity, samples, denoise state and timings.

Initial main-thread implementation reached eight real samples but caused a
424 ms input gap during first load. The worker implementation on the same
77,216-triangle, 3.7 MB board retained all 90,064 source normals and 19 materials;
the focused test measured only 0.2 ms maximum main-thread timer delay during the
approximately one-second load/build. The software GPU still needs time to
converge. A prior full-board 64-sample capture completed; the large C5 closeup
hit its 90-second budget at 44.2 samples and was honestly labelled partial.
Higher-sample guided captures are separate evidence, not assumptions about
hardware GPU performance or native phone parity.

`solid_pathtrace_visibility_test.mjs` additionally uses actual GPU frames to
check empty GLB, initially off-camera original geometry, camera reveal, empty
replacement revision, native DPR 2 resolution and no tracer allocation in Studio.
`solid_pathtrace_state_test.mjs` checks deferred generation-bound first-frame
reporting and budget-paused denoising with deterministic readback stand-ins.

## Reversible physical-board presentation and safe tracing

`realistic` joins `cad-studio` and `source-defaults` in `materialPolicies`.
Only original GLTF mesh definitions and parser associations identify
`*_soldermask`, `*_silkscreen`, `*_pad`, `*_copper`, `*_via` and `*_PCB` roles;
RGB does not establish them, and ambiguous shared roles are skipped. Mask uses
an estimated opaque dielectric surface, roughness .24, clearcoat 1, coat roughness
.10 and IOR 1.5, retaining its exact source color. Pad/copper finish, diffuse silk
and rough substrate are also presentation estimates. Source/cad policies restore
original material objects and explicit values. Unclassified vendor materials
retain explicit factors and only fill absent factors. The fresh SMU has 13
unclassified, one pad, two silk, two mask and one substrate material; both studio
policies estimate 26 missing fields. Realistic additionally overrides ten
authored properties. These numbers do not prove measured physical finishes.

Status reports `materialRoleCounts`, `presentationOverrideCount`,
`explicitPresentationOverrideCount`, `materialWarnings` and `materialPolicy`.
Policy changes rebuild acceleration, release all material-dependent guides and
dispose presentation clones. Scene teardown restores originals before collecting
their GPU resources. CAD studio remains the default; realistic quality is subject
to actual visual review, not implied by its name.

Studio uses a source-scaled directional shadow key in place of the raster area
key, with a 1024 PCF map and 5um normal bias. Actual component meshes cast onto
actual board surfaces; there are no shadow planes or new CAD surfaces. Component
self-shadowing is omitted to avoid small-scale map acne. Path tracing retains the
area key and physical ray shadows; the two lighting paths are disclosed separately.

Tracing uses up to 1.5× bounded internal supersampling within the pixel budget.
Diagnostics expose both planned trace dimensions and actual `traceTargetWidth`,
`traceTargetHeight`, `guideWidth`, `guideHeight`. At most one tile is outstanding:
WebGL2 fences are checked with timeout zero after event-loop yields, with no
`gl.finish` or busy wait. Completion includes a final presentation fence.

Known software renderers are blocked BEFORE tracer/BVH allocation using the actual
OffscreenCanvas unmasked identity (`SwiftShader`, `llvmpipe`, `softpipe`, software
rasterizer), as are missing float targets. An unknown identity is not proof of
software. `pathTraceAvailable`/`pathTraceReason` advertise this; a blocked request
stays `raster` and retains original Studio. No product force override exists.
Masked/unmasked context identity appears only in local diagnostics, with no new
network or telemetry. The historical software path failed with a 35.7s compositor
stall despite fencing; the new safety fallback does not reclassify that as a pass.

`test/tools/solid_quality_test.mjs` supports explicit test-only `PATH_GPU=hardware`
with isolated headless Vulkan flags, and requires the actual render-worker context
to identify the RTX 5080 before claiming hardware evidence. It captures native
fresh-source QFN/mask raw/filtered pixels, exact source bounds, sample/time budgets,
browser RAF/worker-ACK delays and renderer identity. Fixtures with extended budgets
do not prove a 30s application budget or phone parity. The fresh source QFN really
intersects the substrate: Z .075–.796mm against board top .710mm and mask .760mm.
No renderer translation repairs that source placement.

## Pinned stratified sampling correction

The vendor build applies `stratified-axes-0.0.24` from
`tools/viewer/vendor-patches.mjs` without modifying installed dependencies.
The pinned texture stores bounce depth in columns and random dimensions in rows;
upstream's reversed shader lookup reads dimensions 14/15 out of bounds with our
5+4 bounce settings. Those dimensions select clearcoat direction and BSDF lobe.
The correction swaps only the lookup coordinates. Material policy, lights,
ray offset, model units, camera and sample budgets stay unchanged.

The build requires package name/version, upstream SHA-256, one exact literal and
one bundler interception. It verifies both dependency inputs remain unchanged.
`manifest.json` records the patch id, module, original/replacement literals,
upstream/transformed hashes, match/interception counts and artifact hashes;
an unexpected upstream change stops the build. Run
`bun test/tools/viewer_vendor_patch_test.mjs` for provenance and rejection checks.

An isolated hardware diagnostic read actual linked shader sources and float32
accumulation targets at consecutive 32/64/128 samples. Every target pixel had
the stated tile coverage, with no intervening resets; raw/guide presentation
left the target hash unchanged. Baseline dimensions 14/15 returned zero in a GPU
texture probe; corrected dimensions varied across samples. At 128 samples the
fixed mask ROI's linear HDR local residual fell from .14790 to .03500, about
76%. This identifies persistent sampling bias, not insufficient sample counts.
These extended diagnostic budgets do not establish the app's 30s performance or
overall visual acceptance. That sampling comparison retained nearest downsampling
and single-sample guides; native HDR coverage is addressed separately below.


## Native HDR pixel coverage

Raw presentation and denoising first resolve the actual trace target into native
output pixels using exact source-texel area overlap. RGB is weighted with alpha
in linear HDR, then unpremultiplied; transparent texel colors cannot create edge
halos. Float32 resolve targets retain bright values. Identity, fractional 1.5,
odd and anisotropic dimensions use the same physical pixel footprint.

Each output footprint is limited to 3×3 texels. A retained source target more
than twice the current output size on either axis is explicitly deferred before
target allocation or drawing, with `resolvePending` and `resolvePendingReason`. The
current visible source frame remains while a current-size trace target arrives;
no oversized fragment loop or silent footprint truncation is allowed. Normal
product trace scale remains at most 1.5. Diagnostics also expose `hdrResolve`,
actual resolve input/output dimensions, `resolveMaxTexelsPerPixel`, `resolveBytes`
and `mixedGuideProtection`; guide memory totals include resolve resources.

Footprints crossing source normal/depth/albedo discontinuities keep their resolved
color and are excluded from interior filtering. Three native-size ping-pong
passes work on premultiplied linear HDR; one final canvas pass applies tone
mapping and output color conversion. This preserves traced subpixel coverage
instead of snapping the filtered image to a single guide texel. Guides are
raster estimates, especially for transparent source materials, not new geometry.

Intermediate tracing presents completed full sample passes through the same
resolve. Partial tile passes retain the previous frame; final/budget-paused
filtering uses the current accumulator. Policy/model/size changes and teardown
release the corresponding resolve and guide resources. Focused checks are
`bun test/tools/solid_hdr_resolve_test.mjs` and the serialized optional `--gpu`
gate against an independent CPU area integral. No material, lighting, ray-offset,
source placement or default-policy change is part of this resolve.


## Bounded original solder candidates

`solid-solder-source.js` selects a read-only traversal view over actual source
nodes: the uniquely identified original GLTF board-pad definition outside
component ancestry, and original saved R/C ancestors and their meshes. Runtime
object names resembling pads, copper colors and bounding boxes do not establish
pad provenance. All duplicate reference containers remain visible to the existing
ambiguity checks. No source nodes are cloned or reparented for qualification.

The existing builder still enforces 12,000 candidate meshes, 300,000 triangles,
exact source/revision, saved pad outlines and actual terminal boundary geometry.
Selection additionally bounds header traversal and ancestry depth. Full outer
copper and via-cut faces do not consume the pad/terminal scan budget. The actual
SMU pads/full/cut sources share 4,027 candidate meshes and 43,500 triangles;
the unchanged builder produces the same 116 illustrative toes on 58 R/C parts,
with identical anchors and R48 still skipped. These remain estimated shapes,
not source CAD or measured solder joints, and default OFF on every load.

Missing/ambiguous pad associations, budget failure or zero qualified joints
reject the toggle with an explicit reason and leave it OFF. Rebuilding disposes
prior illustration/trace/guide state before qualification; failure reports OFF
immediately. Successful coverage includes `sourceCandidates` provenance/counts.
Focused CPU checks: `bun test/tools/solid_solder_source_test.mjs`; its documented
optional saved-source inputs exercise exact anchor equality across export recipes.

## Compatible opaque raster batches

The Studio renderer builds a separate `solid-raster-batches.js`
representation as it loads a source of 128 or more meshes, before the first
draw, and the positive framebuffer/background proof qualifies that batched draw;
a build that falls back proves the original meshes as before. It requires the actual context's
`WEBGL_multi_draw` extension before allocating derived objects. Sources with
fewer than 128 meshes keep their ordinary raster representation. This does not
change the authenticated asset recipe, source geometry, PBR factors, lighting,
ray offset or default presentation policy.

Compatible opaque meshes share a Three `BatchedMesh` only when material identity,
complete attribute/index layout, layer, order, shadow and culling state agree.
Original standard attributes, including authored normals and UVs, are copied
exactly; instances use their original world matrices in metre units. Transparent
source meshes remain ordinary meshes with borrowed original geometry/materials.
Unknown hierarchy, shaders/hooks, attribute storage, skin/morph/instancing,
alpha-test/hash/transmission, shear/reflection or budget excess retains the
**whole original raster scene**, with an explicit diagnostic reason.

Header traversal yields every 256 nodes and geometry/instance copying yields
in bounded chunks through worker tasks. Hard limits are 60,000 nodes, depth 128,
32,768 meshes, 64 compatible groups, one million copied vertices, three million
indices and 64 MiB of derived geometry/instance buffers. Individual geometry is
limited to 65,535 vertices and 200,000 indices. Shared original geometry is
copied once per compatible group. The derived scene owns its buffers, instance
textures and cloned light shadow maps. Original geometry, materials, texture
maps and environment remain borrowed. The fixed 1024² stage shadow maps are
separate from the reported geometry/instance byte count.

Original nodes and parser associations remain authoritative for picking,
component UUIDs/bounds, material roles, exact solder qualification, ray
acceleration and denoise guides. The raster representation never enters that
scene. Selection remains a separate overlay. Source-root visibility suppresses
all derived source geometry, so the proof's background pass stays empty. The
first-frame flag comes from the batched draw when one was built (exact copies of
the source attributes and indices), else from the original meshes. On the SMU
card (22,357 meshes, RTX 5080) this took bytes-to-first-frame from 1.08-1.17 s
to 0.55-0.68 s: one upload and draw per mesh was most of the first frame. A changed source/material/illustration/size epoch, hidden
viewer, teardown or entry into path tracing cancels pending optimization.
Owned resources are released synchronously before borrowed policy, solder or
source resources are disposed; a superseded shader compilation cannot publish.

Status fields describe the current epoch only: `sourceMeshes` counts actual
original meshes; `rasterBatchState` is `idle`, `building`, `ready`, `active` or
`fallback`; `rasterBatchReason` explains fallback. `rasterBatchSourceMeshes`,
`rasterBatchGroups`, `rasterBatchTransparentMeshes` and `rasterBatchBytes` describe
a completed representation. `rasterBatchBuildMs` is null until ready, then
measures request-to-ready elapsed time **including task waits and shader
compilation**. `rasterBatchEpoch` is monotonic and `rasterBatchMultiDraw` reports
the actual extension. New epochs reset derived counts/timing. `active` means an
actual derived raster draw, not geometry visibility or a build-only claim.
Production `renderCalls` retains Three's existing meaning; it is not an
instrumented count of GL API submissions.

Read-only RTX 5080 prototype evidence used the actual full-copper/via-cut GLB
`fd09119bf2df94ac7a5c36899c0fcec13d945ceb857852389140710776530513`
(22,357 source meshes, 311,551 triangles). Warm GL submissions changed from
22,359 to 22 (18 multi-draw plus four transparent passes); the first shadowed
optimized frame used 35 submissions. Five warm orbit calls averaged 72.44 to
7.10 ms CPU and 78.09 to 13.88 ms RPC. Task-yielded construction was 127.9 ms,
shader compilation 13 ms and promotion about 149 ms total. This is not a faster
cold-load claim: the first original source frame was retained before promotion.
Derived geometry/instance storage was 12,086,838 bytes; GPU geometry/texture
counts returned exactly to baseline after disposal.

Matched native 1000×700 prototype board/component/top/bottom images retained
camera/source geometry. Board mean absolute code-value difference was
0.001038/255; component 0.000633/255. Sparse antialiased edge outliers reached
45 code values; float instance matrices differ from original double matrices
by at most 12.1 nm. The bottom pair was identical and remained nearly black
under the unchanged studio. These results establish bounded performance and
source fidelity, **not overall realistic appearance or underside acceptance**.
89 actual component bounds were mapped, not all 96 metadata rows. The bounded
solder candidate path retained the same default-OFF 116 estimated toes on 58
R/C parts. Final normal-UI qualification of the production lifecycle is separate
from these prototype measurements.

Focused checks: `bun test/tools/solid_raster_batches_test.mjs` and
`bun test/tools/solid_pathtrace_state_test.mjs`. Optional `RASTER_BATCH_GLBS`
(JSON array of the two full-source paths) and `RASTER_BATCH_RECEIPT` exercise
exact real-source attribute/index equality, graph/material invariance, policy
reversal, derived bounds and borrowed-resource cleanup without a GPU job.
