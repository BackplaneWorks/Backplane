// step2glb: STEP (.step/.stp) -> binary glTF (GLB), via OpenCascade (occt-import-js, WASM).
//
//   step2glb IN.step OUT.glb [--linear 0.1] [--angular 0.5]
//   step2glb --tree IN.step
//
// A thin host adapter, like curl or git: it tessellates and packs; the Bend
// viewer owns everything after that. Coordinates stay in STEP millimetres
// (Z up, as authored); asset.extras records { unit: "mm", up: "Z" }.

import occtimportjs from "occt-import-js";
import { nativePath } from "../platform/windows-path.ts";
// Embedded into the executable by `bun build --compile`; a plain path under `bun run`.
import wasmPath from "occt-import-js/dist/occt-import-js.wasm" with { type: "file" };

const VERSION = "0.1.0";
const DEFAULT_COLOR: RGB = [0.8, 0.8, 0.8];

type RGB = [number, number, number];
type OcctFace = { first: number; last: number; color: RGB | null };
type OcctMesh = {
  name: string;
  color?: RGB;
  brep_faces: OcctFace[];
  attributes: { position: { array: number[] }; normal?: { array: number[] } };
  index: { array: number[] };
};
type OcctNode = { name: string; meshes: number[]; children: OcctNode[]; instance?: string };
type OcctResult = { success: boolean; root: OcctNode; meshes: OcctMesh[] };

type Box = { min: [number, number, number]; max: [number, number, number] };

function usage(code: number): never {
  const text = [
    "usage: step2glb IN.step OUT.glb [--linear MM] [--angular RAD]",
    "       step2glb --tree IN.step",
    "  --linear   linear deflection in mm (default 0.1)",
    "  --angular  angular deflection in radians (default 0.5)",
    "  --tree     print the parts tree as JSON (names, triangles, bbox in mm)",
  ].join("\n");
  (code === 0 ? console.log : console.error)(text);
  process.exit(code);
}

function fail(msg: string): never {
  console.error(`step2glb: ${msg}`);
  process.exit(1);
}

type Args = { tree: boolean; input: string; output?: string; linear: number; angular: number };

function parseArgs(argv: string[]): Args {
  const pos: string[] = [];
  let tree = false, linear = 0.1, angular = 0.5;
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]!;
    const num = () => {
      const v = Number(argv[++i]);
      if (!(v > 0)) fail(`${a} needs a positive number`);
      return v;
    };
    if (a === "-h" || a === "--help") usage(0);
    else if (a === "--version") { console.log(VERSION); process.exit(0); }
    else if (a === "--tree") tree = true;
    else if (a === "--linear") linear = num();
    else if (a === "--angular") angular = num();
    else if (a.startsWith("--")) fail(`unknown option ${a}`);
    else pos.push(a);
  }
  if (tree ? pos.length !== 1 : pos.length !== 2) usage(2);
  return { tree, input: pos[0]!, output: pos[1], linear, angular };
}

async function readStep(path: string, linear: number, angular: number): Promise<[OcctResult, Uint8Array]> {
  const file = Bun.file(path);
  if (!(await file.exists())) fail(`no such file: ${path}`);
  const content = new Uint8Array(await file.arrayBuffer());
  const wasmBinary = new Uint8Array(await Bun.file(wasmPath).arrayBuffer());
  const occt = await occtimportjs({ wasmBinary, print: () => {}, printErr: () => {} });
  const result: OcctResult = occt.ReadStepFile(content, {
    linearUnit: "millimeter",
    linearDeflectionType: "absolute_value",
    linearDeflection: linear,
    angularDeflection: angular,
  });
  if (!result || !result.success) fail(`could not read STEP: ${path}`);
  return [result, content];
}

// ---- instance names ----------------------------------------------------------
//
// OCCT names each node after its product (the part: "C_0805_2012Metric"). The
// instance name (NEXT_ASSEMBLY_USAGE_OCCURRENCE, e.g. the reference designator
// "C14" in a KiCad board export) is dropped, so recover it from the STEP text.
// Applied only where the file's assembly structure matches OCCT's tree name for
// name, level by level; anything unexpected leaves the node without one.

const STR = "'((?:[^']|'')*)'";
const unq = (s: string) => s.replace(/''/g, "'");

function annotateInstances(root: OcctNode, content: Uint8Array) {
  const text = new TextDecoder("latin1").decode(content);
  const re = (head: string, tail: string) => new RegExp(`#(\\d+)\\s*=\\s*${head}\\s*\\(${tail}`, "g");
  const productName = new Map<string, string>();
  for (const m of text.matchAll(re("PRODUCT", `\\s*${STR}`))) productName.set(m[1]!, unq(m[2]!));
  const formation = new Map<string, string>();
  for (const m of text.matchAll(re("PRODUCT_DEFINITION_FORMATION(?:_WITH_SPECIFIED_SOURCE)?", `[^#;]*#(\\d+)`)))
    formation.set(m[1]!, m[2]!);
  const pdName = new Map<string, string>();
  const pds: string[] = [];
  for (const m of text.matchAll(re("PRODUCT_DEFINITION(?:_WITH_ASSOCIATED_DOCUMENTS)?", `\\s*${STR}\\s*,\\s*(?:${STR}|\\$)\\s*,\\s*#(\\d+)`))) {
    const prod = formation.get(m[4]!);
    if (prod !== undefined && productName.has(prod)) { pdName.set(m[1]!, productName.get(prod)!); pds.push(m[1]!); }
  }
  type Use = { name: string; child: string };
  const uses = new Map<string, Use[]>();
  const used = new Set<string>();
  for (const m of text.matchAll(re("NEXT_ASSEMBLY_USAGE_OCCURRENCE",
    `\\s*${STR}\\s*,\\s*${STR}\\s*,\\s*(?:${STR}|\\$)\\s*,\\s*#(\\d+)\\s*,\\s*#(\\d+)`))) {
    const [parent, child] = [m[5]!, m[6]!];
    if (!uses.has(parent)) uses.set(parent, []);
    uses.get(parent)!.push({ name: unq(m[3]!), child });
    used.add(child);
  }
  const walk = (node: OcctNode, kids: Use[]) => {
    if (kids.length !== node.children.length) return;
    if (!kids.every((u, i) => pdName.get(u.child) === node.children[i]!.name)) return;
    kids.forEach((u, i) => {
      const c = node.children[i]!;
      if (u.name && !u.name.startsWith("=>") && u.name !== c.name) c.instance = u.name;
      walk(c, uses.get(u.child) ?? []);
    });
  };
  walk(root, pds.filter((p) => !used.has(p)).map((p) => ({ name: "", child: p })));
}

// ---- geometry helpers ------------------------------------------------------

const triCount = (m: OcctMesh) => (m.index.array.length / 3) | 0;

function meshBox(m: OcctMesh): Box | null {
  const p = m.attributes.position.array;
  if (p.length < 3) return null;
  const min: Box["min"] = [Infinity, Infinity, Infinity];
  const max: Box["max"] = [-Infinity, -Infinity, -Infinity];
  for (let i = 0; i < p.length; i += 3)
    for (let k = 0; k < 3; k++) {
      const v = p[i + k]!;
      if (v < min[k]!) min[k] = v;
      if (v > max[k]!) max[k] = v;
    }
  return { min, max };
}

function union(a: Box | null, b: Box | null): Box | null {
  if (!a) return b;
  if (!b) return a;
  return {
    min: [Math.min(a.min[0], b.min[0]), Math.min(a.min[1], b.min[1]), Math.min(a.min[2], b.min[2])],
    max: [Math.max(a.max[0], b.max[0]), Math.max(a.max[1], b.max[1]), Math.max(a.max[2], b.max[2])],
  };
}

// Smooth vertex normals from the triangles, for meshes OCCT gave none.
function computeNormals(pos: number[], idx: number[]): Float32Array {
  const n = new Float32Array(pos.length);
  for (let t = 0; t < idx.length; t += 3) {
    const a = idx[t]! * 3, b = idx[t + 1]! * 3, c = idx[t + 2]! * 3;
    const ux = pos[b]! - pos[a]!, uy = pos[b + 1]! - pos[a + 1]!, uz = pos[b + 2]! - pos[a + 2]!;
    const vx = pos[c]! - pos[a]!, vy = pos[c + 1]! - pos[a + 1]!, vz = pos[c + 2]! - pos[a + 2]!;
    const nx = uy * vz - uz * vy, ny = uz * vx - ux * vz, nz = ux * vy - uy * vx;
    for (const v of [a, b, c]) { n[v]! += nx; n[v + 1]! += ny; n[v + 2]! += nz; }
  }
  return normalize(n);
}

function normalize(n: Float32Array): Float32Array {
  for (let i = 0; i < n.length; i += 3) {
    const l = Math.hypot(n[i]!, n[i + 1]!, n[i + 2]!);
    if (l > 0) { n[i]! /= l; n[i + 1]! /= l; n[i + 2]! /= l; }
    else { n[i] = 0; n[i + 1] = 0; n[i + 2] = 1; }
  }
  return n;
}

// Triangle ranges grouped by colour: face colour, else mesh colour, else grey.
function colorGroups(m: OcctMesh): Map<string, { color: RGB; tris: number[] }> {
  const groups = new Map<string, { color: RGB; tris: number[] }>();
  const base = m.color ?? DEFAULT_COLOR;
  const add = (color: RGB, first: number, last: number) => {
    const key = color.map((c) => c.toFixed(4)).join(",");
    let g = groups.get(key);
    if (!g) groups.set(key, (g = { color, tris: [] }));
    for (let t = first; t <= last; t++) g.tris.push(t);
  };
  const covered = new Uint8Array(triCount(m));
  for (const f of m.brep_faces ?? []) {
    add(f.color ?? base, f.first, f.last);
    covered.fill(1, f.first, f.last + 1);
  }
  for (let t = 0; t < covered.length; t++) if (!covered[t]) add(base, t, t);
  return groups;
}

// ---- --tree ----------------------------------------------------------------

type TreeNode = {
  name: string;
  instance?: string;
  triangles: number;
  bbox: Box | null;
  meshes?: { name: string; triangles: number; color: RGB | null; bbox: Box | null }[];
  children?: TreeNode[];
};

function tree(node: OcctNode, meshes: OcctMesh[]): TreeNode {
  const ms = node.meshes.map((i) => {
    const m = meshes[i]!;
    return { name: m.name, triangles: triCount(m), color: m.color ?? null, bbox: meshBox(m) };
  });
  const kids = node.children.map((c) => tree(c, meshes));
  let triangles = 0, bbox: Box | null = null;
  for (const x of [...ms, ...kids]) { triangles += x.triangles; bbox = union(bbox, x.bbox); }
  const out: TreeNode = { name: node.name, triangles, bbox };
  if (node.instance) out.instance = node.instance;
  if (ms.length) out.meshes = ms;
  if (kids.length) out.children = kids;
  return out;
}

// ---- GLB writer ------------------------------------------------------------

type Json = Record<string, unknown>;

class Glb {
  chunks: Uint8Array[] = [];
  length = 0;
  bufferViews: Json[] = [];
  accessors: Json[] = [];
  materials: Json[] = [];
  materialIndex = new Map<string, number>();
  meshes: Json[] = [];
  nodes: Json[] = [];

  view(bytes: Uint8Array, target: number): number {
    const pad = (4 - (this.length % 4)) % 4;
    if (pad) { this.chunks.push(new Uint8Array(pad)); this.length += pad; }
    this.bufferViews.push({ buffer: 0, byteOffset: this.length, byteLength: bytes.byteLength, target });
    this.chunks.push(bytes);
    this.length += bytes.byteLength;
    return this.bufferViews.length - 1;
  }

  accessor(a: Json): number {
    this.accessors.push(a);
    return this.accessors.length - 1;
  }

  material(color: RGB): number {
    const key = color.map((c) => c.toFixed(4)).join(",");
    let i = this.materialIndex.get(key);
    if (i === undefined) {
      i = this.materials.length;
      this.materials.push({
        name: `rgb(${color.map((c) => Math.round(c * 255)).join(",")})`,
        pbrMetallicRoughness: { baseColorFactor: [...color, 1], metallicFactor: 0, roughnessFactor: 0.7 },
        doubleSided: true,
      });
      this.materialIndex.set(key, i);
    }
    return i;
  }

  mesh(m: OcctMesh): number | null {
    const pos = m.attributes.position.array;
    const idx = m.index.array;
    const count = pos.length / 3;
    if (count === 0 || idx.length === 0) return null;
    const box = meshBox(m)!;
    const posAcc = this.accessor({
      bufferView: this.view(new Uint8Array(new Float32Array(pos).buffer), 34962),
      componentType: 5126, count, type: "VEC3",
      min: box.min.map(Math.fround), max: box.max.map(Math.fround),
    });
    const nrm = m.attributes.normal?.array.length === pos.length
      ? normalize(new Float32Array(m.attributes.normal!.array))
      : computeNormals(pos, idx);
    const nrmAcc = this.accessor({
      bufferView: this.view(new Uint8Array(nrm.buffer), 34962),
      componentType: 5126, count, type: "VEC3",
    });
    const primitives: Json[] = [];
    for (const g of colorGroups(m).values()) {
      const ix = new Uint32Array(g.tris.length * 3);
      g.tris.forEach((t, k) => { ix[k * 3] = idx[t * 3]!; ix[k * 3 + 1] = idx[t * 3 + 1]!; ix[k * 3 + 2] = idx[t * 3 + 2]!; });
      primitives.push({
        attributes: { POSITION: posAcc, NORMAL: nrmAcc },
        indices: this.accessor({
          bufferView: this.view(new Uint8Array(ix.buffer), 34963),
          componentType: 5125, count: ix.length, type: "SCALAR",
        }),
        material: this.material(g.color),
        mode: 4,
      });
    }
    this.meshes.push({ name: m.name, primitives });
    return this.meshes.length - 1;
  }

  // One glTF node per STEP node; a node with several meshes gets one child per mesh.
  node(n: OcctNode, meshes: OcctMesh[], meshIds: (number | null)[]): number {
    const own = n.meshes.map((i) => ({ i, id: meshIds[i] })).filter((x) => x.id != null);
    const children = n.children.map((c) => this.node(c, meshes, meshIds));
    const out: Json = { name: n.name };
    if (n.instance) out.extras = { instance: n.instance };
    if (own.length === 1 && children.length === 0) out.mesh = own[0]!.id;
    else
      for (const { i, id } of own) {
        this.nodes.push({ name: meshes[i]!.name || n.name, mesh: id });
        children.push(this.nodes.length - 1);
      }
    if (children.length) out.children = children;
    this.nodes.push(out);
    return this.nodes.length - 1;
  }

  encode(root: number, source: string): Uint8Array {
    const json: Json = {
      asset: { version: "2.0", generator: `backplane-step2glb ${VERSION}`, extras: { unit: "mm", up: "Z", source } },
      scene: 0,
      scenes: [{ name: source, nodes: [root] }],
      nodes: this.nodes,
    };
    if (this.meshes.length) {
      Object.assign(json, {
        meshes: this.meshes, materials: this.materials, accessors: this.accessors,
        bufferViews: this.bufferViews, buffers: [{ byteLength: this.length }],
      });
    }
    let jsonBytes = new TextEncoder().encode(JSON.stringify(json));
    const jsonPad = (4 - (jsonBytes.length % 4)) % 4;
    const binPad = (4 - (this.length % 4)) % 4;
    const binLen = this.length + binPad;
    const hasBin = this.length > 0;
    const total = 12 + 8 + jsonBytes.length + jsonPad + (hasBin ? 8 + binLen : 0);
    const out = new Uint8Array(total);
    const dv = new DataView(out.buffer);
    dv.setUint32(0, 0x46546c67, true); // "glTF"
    dv.setUint32(4, 2, true);
    dv.setUint32(8, total, true);
    dv.setUint32(12, jsonBytes.length + jsonPad, true);
    dv.setUint32(16, 0x4e4f534a, true); // "JSON"
    out.set(jsonBytes, 20);
    out.fill(0x20, 20 + jsonBytes.length, 20 + jsonBytes.length + jsonPad);
    if (hasBin) {
      let o = 20 + jsonBytes.length + jsonPad;
      dv.setUint32(o, binLen, true);
      dv.setUint32(o + 4, 0x004e4942, true); // "BIN\0"
      o += 8;
      for (const c of this.chunks) { out.set(c, o); o += c.byteLength; }
    }
    return out;
  }
}

// ---- FreeCAD, for what OCCT's WASM build cannot mesh ------------------------
//
// occt-import-js reads some large STEP files (a whole KiCad board with its
// parts, a case assembly) but meshes none of their faces: every mesh comes
// back empty. FreeCAD's native OpenCascade meshes them, and quickly, so a big
// file goes straight to it when it is installed, and any file OCCT leaves
// empty goes there after. Solids are grouped into a few tints (FreeCAD without
// its GUI has no colours) so an assembly's parts tell apart. Nothing is
// written when neither can mesh the file: a caller never gets an empty model.

const BIG = 12 << 20;

const FREECAD_PY = `
import os, struct, json, array, Part
src, out, tol = os.environ["BP_IN"], os.environ["BP_OUT"], float(os.environ.get("BP_TOL", "0.1"))
s = Part.read(src)
shapes = list(s.Solids) or [s]
pal = [(0.8, 0.8, 0.8), (0.6, 0.65, 0.72), (0.74, 0.68, 0.58), (0.52, 0.62, 0.52), (0.7, 0.58, 0.64), (0.58, 0.7, 0.72)]
groups = [[] for _ in pal]
for i, so in enumerate(shapes):
    groups[i % len(pal)].append(so)
buf = bytearray(); views = []; accs = []; prims = []; mats = []; tris = 0
for gi, g in enumerate(groups):
    pts = []; idx = []
    for so in g:
        p, f = so.tessellate(tol)
        base = len(pts); pts.extend(p)
        for t in f: idx.extend((base + t[0], base + t[1], base + t[2]))
    if not idx: continue
    tris += len(idx) // 3
    flat = [c for v in pts for c in (v.x, v.y, v.z)]
    lo = [min(flat[k::3]) for k in range(3)]; hi = [max(flat[k::3]) for k in range(3)]
    views.append({"buffer": 0, "byteOffset": len(buf), "byteLength": 12 * len(pts), "target": 34962})
    buf += array.array("f", flat).tobytes()
    accs.append({"bufferView": len(views) - 1, "componentType": 5126, "count": len(pts), "type": "VEC3", "min": lo, "max": hi})
    views.append({"buffer": 0, "byteOffset": len(buf), "byteLength": 4 * len(idx), "target": 34963})
    buf += array.array("I", idx).tobytes()
    accs.append({"bufferView": len(views) - 1, "componentType": 5125, "count": len(idx), "type": "SCALAR"})
    mats.append({"pbrMetallicRoughness": {"baseColorFactor": list(pal[gi]) + [1.0], "metallicFactor": 0.0, "roughnessFactor": 0.8}})
    prims.append({"attributes": {"POSITION": len(accs) - 2}, "indices": len(accs) - 1, "material": len(mats) - 1})
if tris == 0: raise SystemExit("no faces could be meshed")
gl = {"asset": {"version": "2.0", "generator": "backplane-step2glb (FreeCAD)", "extras": {"unit": "mm", "up": "Z"}},
      "buffers": [{"byteLength": len(buf)}], "bufferViews": views, "accessors": accs, "materials": mats,
      "meshes": [{"primitives": prims}], "nodes": [{"name": os.path.basename(src), "mesh": 0}], "scenes": [{"nodes": [0]}], "scene": 0}
js = json.dumps(gl).encode()
js += b" " * (-len(js) % 4); buf += b"\\0" * (-len(buf) % 4)
total = 12 + 8 + len(js) + 8 + len(buf)
with open(out, "wb") as o:
    o.write(struct.pack("<III", 0x46546C67, 2, total))
    o.write(struct.pack("<II", len(js), 0x4E4F534A)); o.write(js)
    o.write(struct.pack("<II", len(buf), 0x004E4942)); o.write(bytes(buf))
print("TRIS", tris)
`;

// FreeCAD's GUI, run offscreen (QT_QPA_PLATFORM=offscreen): only it reads
// a STEP's colours (the console build drops them). Each face keeps its
// colour; an object with none (FreeCAD's default grey) gets a tint of its
// own, so an assembly's parts still tell apart. It writes the GLB and the
// triangle count (in out.n: a GUI's stdout is not dependable).
const FREECAD_GUI_PY = `import os, struct, json, array
import FreeCAD as App, FreeCADGui as Gui, ImportGui
src, out, tol = os.environ["BP_IN"], os.environ["BP_OUT"], float(os.environ.get("BP_TOL", "0.25"))
try:
    doc = App.newDocument("bpmesh")
    ImportGui.insert(src, doc.Name)
    default = (0.8, 0.8, 0.901961)
    pal = [(0.62, 0.66, 0.72), (0.74, 0.68, 0.58), (0.52, 0.62, 0.52), (0.7, 0.58, 0.64), (0.58, 0.7, 0.72), (0.8, 0.8, 0.8)]
    groups = {}; n = 0
    for o in doc.Objects:
        if o.TypeId != "Part::Feature" or not o.Shape.Faces: continue
        vo = o.ViewObject
        cols = list(getattr(vo, "DiffuseColor", []) or [])
        base = tuple(round(c, 4) for c in vo.ShapeColor[:3])
        plain = all(abs(a - b) < 0.01 for a, b in zip(base, default)) and all(all(abs(a - b) < 0.01 for a, b in zip(c[:3], default)) for c in cols)
        tint = pal[n % len(pal)]; n += 1
        faces = o.Shape.Faces
        for i, f in enumerate(faces):
            c = tint if plain else tuple(round(x, 3) for x in (cols[i][:3] if len(cols) == len(faces) else (cols[0][:3] if cols else base)))
            p, t = f.tessellate(tol)
            if not t: continue
            g = groups.setdefault(c, ([], []))
            b0 = len(g[0]); g[0].extend(p); g[1].extend(k for tri in t for k in (b0 + tri[0], b0 + tri[1], b0 + tri[2]))
    buf = bytearray(); views = []; accs = []; prims = []; mats = []; tris = 0
    for c, (pts, idx) in groups.items():
        tris += len(idx) // 3
        flat = [x for v in pts for x in (v.x, v.y, v.z)]
        lo = [min(flat[k::3]) for k in range(3)]; hi = [max(flat[k::3]) for k in range(3)]
        views.append({"buffer": 0, "byteOffset": len(buf), "byteLength": 12 * len(pts), "target": 34962})
        buf += array.array("f", flat).tobytes()
        accs.append({"bufferView": len(views) - 1, "componentType": 5126, "count": len(pts), "type": "VEC3", "min": lo, "max": hi})
        views.append({"buffer": 0, "byteOffset": len(buf), "byteLength": 4 * len(idx), "target": 34963})
        buf += array.array("I", idx).tobytes()
        accs.append({"bufferView": len(views) - 1, "componentType": 5125, "count": len(idx), "type": "SCALAR"})
        mats.append({"pbrMetallicRoughness": {"baseColorFactor": list(c) + [1.0], "metallicFactor": 0.0, "roughnessFactor": 0.8}})
        prims.append({"attributes": {"POSITION": len(accs) - 2}, "indices": len(accs) - 1, "material": len(mats) - 1})
    if tris:
        gl = {"asset": {"version": "2.0", "generator": "backplane-step2glb (FreeCAD)", "extras": {"unit": "mm", "up": "Z"}},
              "buffers": [{"byteLength": len(buf)}], "bufferViews": views, "accessors": accs, "materials": mats,
              "meshes": [{"primitives": prims}], "nodes": [{"name": os.path.basename(src), "mesh": 0}], "scenes": [{"nodes": [0]}], "scene": 0}
        js = json.dumps(gl).encode(); js += b" " * (-len(js) % 4); buf += b"\\0" * (-len(buf) % 4)
        with open(out, "wb") as f:
            f.write(struct.pack("<III", 0x46546C67, 2, 12 + 8 + len(js) + 8 + len(buf)))
            f.write(struct.pack("<II", len(js), 0x4E4F534A)); f.write(js)
            f.write(struct.pack("<II", len(buf), 0x004E4942)); f.write(bytes(buf))
    with open(out + ".n", "w") as f: f.write(str(tris))
except Exception as e:
    with open(out + ".n", "w") as f: f.write("0 " + repr(e))
os._exit(0)
`;

// FreeCAD's command, as the hub finds it (BACKPLANE_FREECAD, then PATH)
function freecad(): string {
  for (const c of [process.env.BACKPLANE_FREECAD, "freecadcmd", "FreeCADCmd", "freecad.cmd", "/Applications/FreeCAD.app/Contents/Resources/bin/freecadcmd"]) {
    if (!c) continue;
    const path = nativePath(c);
    const p = /[\\/]/.test(path) ? (Bun.file(path).size > 0 ? path : null) : Bun.which(path);
    if (p) return p;
  }
  return "";
}

// FreeCAD's GUI program: beside its console one, else on PATH
function freecadGui(): string {
  const fc = freecad();
  const normal = fc.replace(/\\/g, "/");
  const dir = normal.includes("/") ? normal.slice(0, normal.lastIndexOf("/")) : "";
  // Portable Windows bundles may put an asynchronous Chocolatey shim at
  // the root. Wait on the real GUI so its temporary Python script stays alive.
  const direct = process.platform === "win32" ? [`${dir}/bin/freecad.exe`] : [];
  const near = [...direct, `${dir}/freecad.exe`, `${dir}/FreeCAD.exe`, `${dir}/freecad`, `${dir}/FreeCAD`, dir.replace(/\/Resources\/bin$/, "/MacOS/FreeCAD")];
  for (const c of [...(dir ? near : []), "freecad", "FreeCAD"]) {
    const path = nativePath(c);
    const p = /[\\/]/.test(path) ? (Bun.file(path).size > 0 ? path : null) : Bun.which(path);
    if (p) return p;
  }
  return "";
}

// the file meshed by FreeCAD's GUI, offscreen, with its colours: its
// triangle count, or 0 (no GUI, no display server needed, or it failed)
async function viaFreecadGui(input: string, out: string, linear: number): Promise<number> {
  const gui = freecadGui();
  if (!gui) return 0;
  const py = `${out}.gui.py`;
  await Bun.write(py, FREECAD_GUI_PY);
  Bun.spawnSync([gui, py], {
    env: { ...process.env, QT_QPA_PLATFORM: "offscreen", BP_IN: input, BP_OUT: out, BP_TOL: String(Math.max(linear, 0.25)) },
    stdout: "ignore", stderr: "ignore", timeout: 600_000,
  });
  await Bun.file(py).delete().catch(() => {});
  const n = Number((await Bun.file(`${out}.n`).text().catch(() => "0")).split(" ")[0]) || 0;
  await Bun.file(`${out}.n`).delete().catch(() => {});
  return n > 0 && Bun.file(out).size > 0 ? n : 0;
}

// the file meshed by FreeCAD into out: its triangle count, or 0. Its GUI
// first (colours), then its console build (tints only)
async function viaFreecad(input: string, out: string, linear: number): Promise<number> {
  const g = await viaFreecadGui(input, out, linear);
  if (g > 0) return g;
  const fc = freecad();
  if (!fc) return 0;
  const py = `${out}.py`;
  await Bun.write(py, FREECAD_PY);
  const p = Bun.spawnSync([fc, "-c", `exec(open(${JSON.stringify(py)}).read())`], {
    env: { ...process.env, BP_IN: input, BP_OUT: out, BP_TOL: String(Math.max(linear, 0.25)) },
    stdout: "pipe", stderr: "pipe",
  });
  await Bun.file(py).delete().catch(() => {});
  const m = /TRIS (\d+)/.exec(p.stdout.toString());
  return p.exitCode === 0 && m ? Number(m[1]) : 0;
}

// ---- main ------------------------------------------------------------------

const args = parseArgs(process.argv.slice(2));
args.input = nativePath(args.input);
if (args.output) args.output = nativePath(args.output);
const t0 = performance.now();
const said = (how: string, tris: number) =>
  console.error(`step2glb: ${args.input.split(/[\\/]/).pop()} -> ${args.output}: ${tris} triangles by ${how}, ${Math.round(performance.now() - t0)} ms`);
// a big file goes to FreeCAD first when there is one (OCCT's WASM build is
// slow on it and often meshes nothing)
if (!args.tree && Bun.file(args.input).size > BIG && freecad()) {
  const n = await viaFreecad(args.input, args.output!, args.linear);
  if (n > 0) { said("FreeCAD", n); process.exit(0); }
}
const [result, content] = await readStep(args.input, args.linear, args.angular);
annotateInstances(result.root, content);
const source = args.input.split(/[\\/]/).pop()!;

if (args.tree) {
  const round = (_k: string, v: unknown) => (typeof v === "number" ? Math.round(v * 1e4) / 1e4 : v);
  console.log(JSON.stringify(tree(result.root, result.meshes), round));
} else {
  const tris = result.meshes.reduce((s, m) => s + triCount(m), 0);
  if (tris === 0) {
    const n = await viaFreecad(args.input, args.output!, args.linear);
    if (n > 0) { said("FreeCAD", n); process.exit(0); }
    fail(`${args.input}: no faces could be meshed` + (freecad() ? "" : " (installing FreeCAD lets Backplane mesh large STEP files)"));
  }
  const glb = new Glb();
  const meshIds = result.meshes.map((m) => glb.mesh(m));
  const root = glb.node(result.root, result.meshes, meshIds);
  const bytes = glb.encode(root, source);
  await Bun.write(args.output!, bytes);
  console.error(
    `step2glb: ${source} -> ${args.output}: ${result.meshes.length} meshes, ${tris} triangles, ` +
      `${bytes.byteLength} bytes, ${Math.round(performance.now() - t0)} ms`,
  );
}
