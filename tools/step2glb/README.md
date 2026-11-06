# step2glb

Turns a STEP file (`.step`/`.stp`) into binary glTF (`.glb`) for STEP
viewer. Tessellating B-reps is out of scope for , so this is a host tool,
like `curl` or `git`: it meshes and packs, and the viewer does everything after.

It wraps [occt-import-js](https://github.com/kovacsv/occt-import-js)
(OpenCascade compiled to WASM) and is built with `bun build --compile` into one
executable, `dist/backplane-step2glb`, with the WASM embedded.

## Usage

```sh
backplane-step2glb IN.step OUT.glb [--linear 0.1] [--angular 0.5]
backplane-step2glb --tree IN.step
```

- `--linear`: linear deflection in mm (absolute). Default 0.1.
- `--angular`: angular deflection in radians. Default 0.5.
- `--tree`: print the parts tree as one line of JSON on stdout, nothing written.
- Exit 0 on success; 1 when the file is missing or not readable STEP; 2 on bad usage.
  Errors and the one-line summary go to stderr.

### GLB output

- One node per STEP node, hierarchy kept (root, assemblies, parts). Node `name` is
  the product name (`C_0805_2012Metric`). When the STEP gives the instance a name
  (a KiCad board export uses the reference designator), it is in
  `node.extras.instance` (`"C14"`). A node with several solids gets one child node per solid.
- Meshes: `POSITION` (f32, with min/max), `NORMAL` (f32, unit), `indices` (u32),
  triangles. Faces are grouped into one primitive per colour.
- Materials: `baseColorFactor` from the STEP face or solid colour, else light grey
  (0.8). Colours are linear RGB, as glTF expects. Metallic 0, roughness 0.7,
  double-sided.
- Units: millimetres, the STEP axes as authored (Z up). glTF's metres/Y-up is not
  applied; `asset.extras` says `{ "unit": "mm", "up": "Z", "source": "<file>" }`.

### `--tree` output

```json
{"name":"","triangles":36766,"bbox":{"min":[x,y,z],"max":[x,y,z]},"children":[
  {"name":"StickHub 1", ..., "children":[
    {"name":"C_0805_2012Metric","instance":"C18","triangles":380,"bbox":{...},
     "meshes":[{"name":"C_0805_2012Metric","triangles":380,"color":null,"bbox":{...}}]}]}]}
```

Numbers are rounded to 1e-4 mm. `color` is the solid's colour or `null`.

## How the app calls it

The hub runs it as a subprocess, like git: `backplane-step2glb IN.step CACHE/x.glb`,
checks the exit code, then reads the GLB. The binary sits next to `backplane` in a
release (`dist/backplane-step2glb`). Cache the GLB by the STEP file's hash and the
deflection options; conversion takes from 0.1 s (a resistor) to a few seconds (a
populated board). Use `--tree` for a quick parts list without reading the GLB.

## Build and check

```sh
scripts/build-step2glb.sh               # bun install + bun build --compile -> dist/backplane-step2glb
BUN_TARGET=bun-linux-arm64 scripts/build-step2glb.sh    # cross-compile
cd tools/step2glb && bun step2glb.ts IN.step OUT.glb    # run from source
bun tools/step2glb/check-glb.ts OUT.glb                 # structural check of a GLB
bunx @gltf-transform/cli validate OUT.glb               # Khronos validator
```

The binary is about 89 MB: the Bun runtime (~81 MB) plus the 7.6 MB WASM.
It runs from any directory; nothing is read from `node_modules` at run time.

## License

step2glb itself is MIT, like the rest of Backplane. It embeds `occt-import-js`
(LGPL-2.1) and the OpenCascade Technology build inside it (LGPL-2.1). That is fine for a separately invoked helper: Backplane only runs
the program and reads its output, and never links it. Obligations when shipping it:

- ship the two license texts with the binary (`scripts/build-step2glb.sh` copies
  them to `dist/licenses/step2glb/`);
- say where the source is: occt-import-js at https://github.com/kovacsv/occt-import-js
  (v0.0.23 on npm), OCCT at https://dev.opencascade.org, and this tool's source here.
  Anyone can rebuild the binary against a modified occt-import-js with the script above.

## Known limits

- Colours in assembly exports: occt-import-js looks up colours on located
  sub-shapes and misses them inside instanced assemblies, so a KiCad board
  export comes out as a green board plus grey parts. Single-part files (the
  KiCad 3D library) keep their colours. To show a coloured board, convert each footprint
  model on its own and place it with the board's footprint transforms (which
  already parses), or fix colour lookup upstream.
- Instance names come from a small scan of the STEP text
  (`NEXT_ASSEMBLY_USAGE_OCCURRENCE`), used only where it matches OCCT's tree
  level by level.
- Memory: a 9 MB STEP took about 0.9 GB RSS and 5 s.
- Instances are flattened: a part used N times becomes N meshes (no sharing).
