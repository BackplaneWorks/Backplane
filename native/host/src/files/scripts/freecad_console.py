
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
js += b" " * (-len(js) % 4); buf += b"\0" * (-len(buf) % 4)
total = 12 + 8 + len(js) + 8 + len(buf)
with open(out, "wb") as o:
    o.write(struct.pack("<III", 0x46546C67, 2, total))
    o.write(struct.pack("<II", len(js), 0x4E4F534A)); o.write(js)
    o.write(struct.pack("<II", len(buf), 0x004E4942)); o.write(bytes(buf))
print("TRIS", tris)
