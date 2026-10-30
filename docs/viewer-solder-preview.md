# Illustrative solder preview

`src/web/solid-solder.js` is an optional, reversible toe-fillet illustration. It does not recover manufacturing joints from CAD. It leaves saved files, original GLB objects, authored materials and normals unchanged. The group starts hidden; the UI must say **Illustrative solder** and must retain the explicit estimated-shape limitation.

## Renderer contract

```js
const preview = buildSolderPreview({THREE, model, components, asset, maxJoints:128});
scene.add(preview.group);             // sibling of model, identity transform
preview.group.visible = enabled;      // default OFF; caller refreshes renderer
preview.dispose();                   // on revision/load change and teardown
```

`model` is the original loaded scene after coordinate normalization and `updateMatrixWorld(true)`. World coordinates are Z-up metres: saved board `[x,y,z]` mm maps to `[x,-y,z]/1000`. Mesh-local triangles are transformed through their actual `matrixWorld`. The returned group does not modify source-model bounds or ownership. Path tracing must rebuild its scene when visibility changes; an already accumulated image cannot gain or lose geometry without restarting accumulation. Renderer hooks and the UI toggle belong to the integration owners.

Return values are `{group, coverage, skipped, dispose}`. `coverage` contains the generated count, explicit limits and the actual terminal/pad anchor evidence. Meshes carry only an illustrative flag and an illustrative name; they are not saved-CAD item identities. Disposal removes only the generated group, geometries and material and is idempotent.

The original-model renderer controller exposes asynchronous
`setSolderPreview(boolean) -> {enabled,source,revision,coverage,skipped}`.
`coverage` always includes `joints`, `components` and `limits`, also when OFF.
Status includes `illustrativeSolder` and `solderJoints`. It builds the optional
group only when explicitly enabled, using current validated component metadata;
replacing metadata while enabled rebuilds it. A new source load disposes the
group and resets OFF. The group stays a scene sibling, so original model bounds,
triangle/normal counts and body picking remain based solely on source meshes.
Toggling invalidates acceleration and accumulation. Separate trace generations
prevent an older in-flight BVH build from restoring superseded geometry.

## Geometry qualification

* Require schema1 saved metadata, exact nonempty `components.source/sourceRevision == asset.source/revision`, unique saved UUIDs/references and exactly one original component ancestor matching its reference. Names identify the component parent, never a terminal by themselves.
* Support only R/C references with two uniquely numbered SMD rect/roundrect/oval pads, supported dimensions and front/back side. Through-hole/custom pads and other component families are skipped.
* Find actual horizontal source faces whose triangulated boundary follows the saved pad outline in both directions, within0.025mm. A board/mask plane, nearby pad or component box cannot stand in for that face. Bounds are used only to reject distant triangles cheaply.
* Find actual coplanar vertical faces, a connected bottom boundary edge and a real triangle intersection at the illustrated contact height. Both end caps must share a source material identity and have a differently partitioned central body. This is an additional conservative topology guard; material names and RGB values never establish conductivity. Monolithic boxes are unsupported.
* The toe extension stays inside the matched real pad boundary. The cubic meniscus, maximum0.25mm rise, material and volume are estimated display choices. No paste deposit, contact angle, wetting state, alloy, thermal history, void, flux transport or intermetallic layer is calculated. No assembly quality or manufacturing acceptance follows from this illustration.

Inputs are limited to2000 components,300000 source triangles,12000 meshes,20000 triangles per supported component and at most256 generated joints. Unsupported/missing/ambiguous geometry produces a reason rather than an approximate replacement. Fused pad faces may be unsupported. Dependencies on referenced vendor-model files remain outside the main-board revision stamp; fresh export provenance must be established separately.

## Actual saved-board evidence, 2026-10-27

The user-rejected128-sample image used cached `board-130960143.glb`:3718256bytes,95 meshes,19 materials, no images/textures. Its cache-path metadata does not prove a current board revision. It contains vendor component surfaces, thin copper/pad solids, flat silkscreen/mask faces and substrate; no solder meniscus is present at C5. Local KiCad source inspection corroborates that `AddPadShape` extrudes pad polygons and the exporter assembles vendor bodies/copper/pads/silk/mask/body; it has no joint-shape builder. The inspected source checkout is distinct from a claim about the exact installed binary source revision.

A fresh read-only export used selected `/home/h/.backplane-bend/kicad/10.99.0-bp.4/bin/kicad-cli` (`--version`:10.99.0), `pcb export glb --force --subst-models --include-pads --include-silkscreen --include-soldermask`, writing only `/tmp/backplane-solder-evidence/saved-smu.glb`. Canonical input was `/home/h/Documents/i2cjak/SMU_Business_Card/boards/smu_card/smu_card.kicad_pcb`; `.backplane.json` names its project. Input SHA256 before/after was `d2f84cd9ce0661d2835a64339d9b1b6ec992111ee778b2c4564bc30ee037733a`, exact stamp `66306:62012461:1276025:1793041181896551999:1793041181896551999`. Export completed in1.258s, exit0. Fresh GLB:3707284bytes, SHA256 `753f2b7dcb70c6e5553dc90b1668bfc98dffc7abf84429c11be7ac66cc5eb86c`.

The production emitted saved-component parser found96 components. C5 is UUID `0e849ec0-8b9e-4255-abfd-d2e22ca90ba0`, `Capacitor_SMD:C_0805_2012Metric`, `10uF`, MPN `CL21A106KAYNNNE`, at119,53mm. Two actual roundrect pads are centred118.05/119.95,53mm, size1×1.45mm, radius ratio0.25. General thickness is0.8mm; stackup core/exported substrate top is0.710mm, actual copper-pad top0.750mm, mask0.760mm and vendor terminal bottom0.795mm. Nominal board thickness alone therefore cannot supply the true pad-top anchor.

Both C5 end faces have22 real triangles each; both pad faces have30. Derived trimmed bottom-edge anchors lie at x117.99999995/120.00000005mm, y52.43000003–53.56999997mm, z0.795mm. Estimated contact height is1.000mm, on actual face triangles. The full board qualifies116 toe illustrations across58 R/C components in about520ms in the Node test; R48 is skipped because matching pad/terminal geometry is unavailable. These counts are specific to this fresh export and saved revision.

`test/tools/solid_solder_test.mjs` checks real geometry, revision/UUID/ancestor guards, identical-color materials, monolithic-box rejection, absent/unsupported geometry, rotation, both sides, bounded counts and disposal. With explicit `SOLDER_GLB` and `SOLDER_COMPONENTS` it also checks the real saved-board gate and unchanged GLB hash. `test/tools/solid_solder_pixels.mjs` renders that same input through an isolated Chromium/Three scene at native900×650, scale1, with zero page errors. OFF/ON pixels show C5's added toes. Its neutral test lighting has no environment map, so black/specular metal in that rig is not a renderer-material quality result.

Local artifacts: `/tmp/backplane-solder-evidence/{saved-export-receipt.json,saved-export.log,saved-components.json,preview-receipt.json,pixels/off.png,pixels/on.png,pixels/receipt.json}`. This qualifies the isolated module and actual anchor derivation. It does **not** qualify installed-app UI, renderer hooks, path-traced inclusion, cross-platform parity or user visual acceptance. No full app build, global proof, push, merge, installation or native CAD edit was performed for this module.

## Closed, outward illustrative geometry

The toe's existing six-by-twelve curved top grid and actual attachment coordinates
are retained. Top, rear, base and both end faces face their exterior in world
`[x,-y,z]`, for front and back sides. Rear and base use the same span subdivisions
as the curved top, so there are no unmatched long edges. Each flat patch has its
own normal vertices; the curved top is smoothed within itself. The hidden rear
and end faces cannot turn its contact-corner shading normals inward.

Each toe has192 triangles rather than170. The actual116-toe source fixture has
22,272 illustrative triangles rather than19,720. Original source triangle and
normal counts, saved anchors, coverage, selector budgets and default OFF behavior
are unchanged. This repairs construction correctness, not measured wetting or
appearance acceptance. In particular, the existing estimated base remains2um
above its qualified pad face and rear contact is coplanar with the terminal.
No ray offset or original component surface is changed.

`bun test/tools/solid_solder_topology_test.mjs` checks the public builder using
front/back, rotated rect/roundrect source fixtures. It independently verifies
position-welded closed/manifold edges, consistent/outward faces, origin-independent
positive signed volume, top-only smoothing, unchanged source geometry and
idempotent disposal. Deliberately missing/reversed faces and smoothing across
hard joins must fail. With `SOLDER_TOPOLOGY_GLB`, `SOLDER_TOPOLOGY_COMPONENTS` and
optional `SOLDER_TOPOLOGY_RECEIPT`, it checks all116 actual toes and the original
anchor digest and all91 old Float32 top-grid vertices per toe. These CPU checks
do not establish that a photographed-looking finish or a visible contact seam
has been resolved; matched renderer evidence remains a separate review.
