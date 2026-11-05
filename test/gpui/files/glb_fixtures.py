#!/usr/bin/env python3
"""glb_fixtures.py DIR: writes hand-made GLBs into DIR, each aiming at one rule of core/glb.bend, gfx/mesh.bend or core/plot.bend's Solid, including malformed files
whose reading must also match. Deterministic: the same bytes every run. Near-plane clipping cannot be reached from here (a fitted camera stands outside the model)."""
import json, math, os, struct, sys
def glb(j, bin=b"", junk=b""):
    t = (j if isinstance(j, str) else json.dumps(j)).encode(); t += b" " * (-len(t) % 4); bin += b"\0" * (-len(bin) % 4)
    return struct.pack("<III", 0x46546C67, 2, 28 + len(t) + len(bin)) + struct.pack("<II", len(t), 0x4E4F534A) + t + struct.pack("<II", len(bin), 0x004E4942) + bin + junk
F = lambda xs: struct.pack(f"<{len(xs)}f", *xs)
def doc(prims, acc, views, mats=None, nodes=None, **kw):
    d = {"scenes": [{"nodes": [0]}], "nodes": nodes or [{"mesh": 0}], "meshes": [{"primitives": prims}], "accessors": acc, "bufferViews": views}
    if mats is not None: d["materials"] = mats
    d.update(kw); return d
V3 = lambda n, bv=0, **k: {"bufferView": bv, "componentType": 5126, "count": n, "type": "VEC3", **k}
IX = lambda n, ct, bv=1, **k: {"bufferView": bv, "componentType": ct, "count": n, "type": "SCALAR", **k}
col = lambda r, g, b, a=1, **k: {"pbrMetallicRoughness": {"baseColorFactor": [r, g, b, a]}, **k}
CUBE_P = [x for v in [(0,0,0),(1,0,0),(1,1,0),(0,1,0),(0,0,1),(1,0,1),(1,1,1),(0,1,1)] for x in v]
CUBE_I = [0,2,1, 0,3,2, 4,5,6, 4,6,7, 0,1,5, 0,5,4, 1,2,6, 1,6,5, 2,3,7, 2,7,6, 3,0,4, 3,4,7]
TRI = [0,0,0, 1,0,0, 0,1,0]
def indexed(ct, fmt):
    ib = struct.pack(f"<{len(CUBE_I)}{fmt}", *CUBE_I); p = F(CUBE_P)
    return glb(doc([{"attributes": {"POSITION": 0}, "indices": 1, "material": 0}], [V3(8), IX(36, ct)], [{"byteOffset": 0}, {"byteOffset": len(p)}], [col(.8, .3, .2)]), p + ib)
def quad(z, s=1.0): return [-s,-s,z, s,-s,z, s,s,z, -s,-s,z, s,s,z, -s,s,z]
def sphere(n, m):
    out = []; cols = []
    pt = lambda i, j: (math.sin(math.pi * i / n) * math.cos(2 * math.pi * j / m), math.cos(math.pi * i / n), math.sin(math.pi * i / n) * math.sin(2 * math.pi * j / m))
    for i in range(n):
        for j in range(m):
            a, b, c, d = pt(i, j), pt(i + 1, j), pt(i + 1, j + 1), pt(i, j + 1)
            out += [*a, *c, *b, *a, *d, *c]
    return out
def lit(s): return s
fx = {}
fx["tri"] = glb(doc([{"attributes": {"POSITION": 0}}], [V3(3)], [{}]), F(TRI))
fx["cube-u8"] = indexed(5121, "B"); fx["cube-u16"] = indexed(5123, "H"); fx["cube-u32"] = indexed(5125, "I")
# transforms: TRS (a unit quaternion), a matrix, children, a repeated mesh
q = math.sqrt(.5)
fx["nodes"] = glb(doc([{"attributes": {"POSITION": 0}, "indices": 1}], [V3(8), IX(36, 5125)], [{}, {"byteOffset": 96}], nodes=[
    {"children": [1, 2, 3], "translation": [.25, .5, -.75]}, {"mesh": 0, "scale": [2, .5, 1.5], "rotation": [0, q, 0, q]},
    {"mesh": 0, "matrix": [.5,0,0,0, 0,.5,0,0, 0,0,.5,0, 3,0,1,1]}, {"translation": [0, 2, 0], "rotation": [.1825742, .3651484, .5477226, .7302967], "children": [4]}, {"mesh": 0}]), F(CUBE_P) + struct.pack("<36I", *CUBE_I))
# materials: several primitives, blends (a translucent wall over an opaque one), alpha near the opaque line, no material, an index past the table
fx["glass"] = glb(doc([{"attributes": {"POSITION": 0}, "material": 0}, {"attributes": {"POSITION": 1}, "material": 1}, {"attributes": {"POSITION": 2}, "material": 2}], [V3(6), V3(6, 1), V3(6, 2)],
    [{}, {"byteOffset": 72}, {"byteOffset": 144}], [col(0, .9, .1), col(1, 0, 0, .5, alphaMode="BLEND"), col(0, 0, 1, .95, alphaMode="BLEND")]), F(quad(0)) + F(quad(.5, .8)) + F(quad(1, .4)))
fx["mats"] = glb(doc([{"attributes": {"POSITION": 0}}, {"attributes": {"POSITION": 1}, "material": 7}, {"attributes": {"POSITION": 2}, "material": 2}], [V3(6), V3(6, 1), V3(6, 2)],
    [{}, {"byteOffset": 72}, {"byteOffset": 144}], [col(2, -1, .5), {"name": "plain"}, col(.25, .5, .75)]), F(quad(0)) + F(quad(.5, .8)) + F(quad(1, .4)))
# equal depths: coplanar squares of different colours, in one primitive and across the 1024-triangle chunks
ties = []; 
for k in range(1300): ties += quad(0, 1 + (k % 5) * .1)
fx["ties"] = glb(doc([{"attributes": {"POSITION": 0}, "material": 0}, {"attributes": {"POSITION": 1}, "material": 1}, {"attributes": {"POSITION": 2}, "material": 2}], [V3(6), V3(6, 1), V3(len(ties) // 3, 2)],
    [{}, {"byteOffset": 72}, {"byteOffset": 144}], [col(1, 0, 0), col(0, 1, 0), col(0, 0, 1)]), F(quad(0)) + F(quad(0)) + F(ties))
# many triangles: chunks, regions and tiles
s = sphere(90, 120); fx["sphere"] = glb(doc([{"attributes": {"POSITION": 0}, "material": 0}], [V3(len(s) // 3)], [{}], [col(.3, .6, .9)]), F(s))
s = sphere(200, 300); fx["sphere-big"] = glb(doc([{"attributes": {"POSITION": 0}, "material": 0}], [V3(len(s) // 3)], [{}], [col(.7, .7, .2)]), F(s))
# sizes: metres, a thousandth of that, far from the origin (the depth precision of thin layers)
fx["tiny"] = glb(doc([{"attributes": {"POSITION": 0}, "indices": 1}], [V3(8), IX(36, 5125)], [{}, {"byteOffset": 96}], nodes=[{"mesh": 0, "scale": [.001, .0016, .0003]}]), F(CUBE_P) + struct.pack("<36I", *CUBE_I))
fx["far"] = glb(doc([{"attributes": {"POSITION": 0}, "indices": 1}], [V3(8), IX(36, 5125)], [{}, {"byteOffset": 96}], nodes=[{"mesh": 0, "translation": [1000, -2000, 500], "scale": [.1, .1, .1]}]), F(CUBE_P) + struct.pack("<36I", *CUBE_I))
layers = quad(0) + quad(.0001) + quad(.00002, .7) + quad(-.0001, .9)
fx["thin"] = glb(doc([{"attributes": {"POSITION": 0}, "material": 0}, {"attributes": {"POSITION": 1}, "material": 1}, {"attributes": {"POSITION": 2}, "material": 2}, {"attributes": {"POSITION": 3}, "material": 0}], [V3(6), V3(6, 1), V3(6, 2), V3(6, 3)],
    [{}, {"byteOffset": 72}, {"byteOffset": 144}, {"byteOffset": 216}], [col(.1, .5, .2), col(.9, .9, .1), col(.8, .1, .1)]), F(layers))
# accessors: interleaved positions, an unaligned start, an unaligned stride, a short BIN, a count past the data, a partial last word
inter = b"".join(F(CUBE_P[i*3:i*3+3]) + F([0, 0, 1]) for i in range(8))
fx["interleaved"] = glb(doc([{"attributes": {"POSITION": 0}, "indices": 1}], [V3(8), IX(36, 5123)], [{"byteStride": 24}, {"byteOffset": 192}]), inter + struct.pack("<36H", *CUBE_I))
fx["unaligned"] = glb(doc([{"attributes": {"POSITION": 0}, "indices": 1}], [V3(8), IX(36, 5123)], [{"byteOffset": 2}, {"byteOffset": 99}]), b"\0\0" + F(CUBE_P) + b"\0" + struct.pack("<36H", *CUBE_I))
odd = b"".join(F(CUBE_P[i*3:i*3+3]) + b"\xee" * 2 for i in range(8))
fx["odd-stride"] = glb(doc([{"attributes": {"POSITION": 0}, "indices": 1}], [V3(8), IX(36, 5123)], [{"byteStride": 14}, {"byteOffset": 112}]), odd + struct.pack("<36H", *CUBE_I))
fx["short-bin"] = glb(doc([{"attributes": {"POSITION": 0}}], [V3(12)], [{}]), F(TRI + TRI[:6]))
fx["short-unaligned"] = glb(doc([{"attributes": {"POSITION": 0}}], [V3(12)], [{"byteOffset": 2}]), b"\0\0" + F(TRI + TRI[:6]))
g = bytearray(glb(doc([{"attributes": {"POSITION": 0}}], [V3(3)], [{}]), F(TRI))); fx["truncated"] = bytes(g[:-10])
g = bytearray(glb(doc([{"attributes": {"POSITION": 0}}], [V3(3)], [{}]), F(TRI))); struct.pack_into("<I", g, 20 + len(json.dumps(doc([{"attributes": {"POSITION": 0}}], [V3(3)], [{}]))) + (-len(json.dumps(doc([{"attributes": {"POSITION": 0}}], [V3(3)], [{}]))) % 4), 34); fx["bin-len-odd"] = bytes(g)
# indices that wrap at the table's power of two or lie past the table
fx["wrap"] = glb(doc([{"attributes": {"POSITION": 0}, "indices": 1}], [V3(5), IX(12, 5125)], [{}, {"byteOffset": 60}]), F([0,0,0, 1,0,0, 0,1,0, 1,1,0, 0,0,1]) + struct.pack("<12I", 0,1,2, 8,9,10, 5,1,2, 3,4,15) )
fx["wrap-tables"] = glb(doc([{"attributes": {"POSITION": 4}, "material": 3}], [V3(3)], [{}], [col(1, 0, 0), col(0, 1, 0)], nodes=[{"mesh": 2}], ), F(TRI))
fx["leftover-index"] = glb(doc([{"attributes": {"POSITION": 0}, "indices": 1}], [V3(3), IX(5, 5121)], [{}, {"byteOffset": 36}]), F(TRI) + bytes([0, 1, 2, 2, 1]))
fx["leftover-verts"] = glb(doc([{"attributes": {"POSITION": 0}}], [V3(4)], [{}]), F(TRI + [1, 1, 0]))
# numbers: strings for numbers, fractions, exponents, big numbers, duplicate keys, lenient punctuation
fx["strings"] = glb('{"scenes":[{"nodes":["0",0]}],"nodes":[{"mesh":"0"}],"meshes":[{"primitives":[{"attributes":{"POSITION":"0"},"mode":"4"}]}],"accessors":[{"bufferView":"0","count":"3","type":"VEC3"}],"bufferViews":[{"byteOffset":"0"}]}', F(TRI))
fx["fractions"] = glb('{"scenes":[{"nodes":[0.0, 0]}],"nodes":[{"mesh":0.0},{"mesh":0}],"meshes":[{"primitives":[{"attributes":{"POSITION":0}}]}],"accessors":[{"count":3.0,"type":"VEC3"},{"count":3e0}],"bufferViews":[{}]}', F(TRI))
fx["exponents"] = glb('{"scenes":[{"nodes":[0]}],"nodes":[{"mesh":0,"scale":[1e0,2E0,1.5e+0],"translation":[1e-3,-2.5E-2,.5]}],"meshes":[{"primitives":[{"attributes":{"POSITION":0},"material":0}]}],"materials":[{"pbrMetallicRoughness":{"baseColorFactor":[1e-1,0.123456789012345678,9.99e-1,1]}}],"accessors":[{"count":3,"type":"VEC3"}],"bufferViews":[{}]}', F(TRI))
fx["duplicates"] = glb('{"scene":0,"scene":5,"scenes":[{"nodes":[0]}],"scenes":[],"nodes":[{"mesh":0}],"meshes":[{"primitives":[{"attributes":{"POSITION":0}}]}],"accessors":[{"count":3,"count":0,"type":"VEC3"}],"bufferViews":[{}]}', F(TRI))
fx["lenient"] = glb('{"scenes" [{"nodes" [0]}] "nodes":[{"mesh":0},,] "meshes":[{"primitives":[{"attributes":{"POSITION":0}}]}] "accessors":[{"count":3 "type":"VEC3"}] "bufferViews":[{}] : ,}', F(TRI))
fx["bad-json"] = glb('{"scenes":[{"nodes":[0]}', F(TRI))
fx["bad-key"] = glb('{"scenes":[{"nodes":[0]}],1:2}', F(TRI))
fx["bad-escape"] = glb('{"a":"\\uZZ00","scenes":[]}', F(TRI))
fx["unicode"] = glb('{"asset":{"generator":"café \\u00e9 \\ud83d\\ude00 \\ud83d"},"scenes":[{"nodes":[0]}],"nodes":[{"name":"=>[0:1:1]","mesh":0}],"meshes":[{"name":"\\"quoted\\\\","primitives":[{"attributes":{"POSITION":0}}]}],"accessors":[{"count":3,"type":"VEC3"}],"bufferViews":[{}]}', F(TRI))
fx["invalid-utf8"] = glb(('{"asset":"a').encode('latin1').decode('latin1') + '","scenes":[{"nodes":[0]}],"nodes":[{"mesh":0}],"meshes":[{"primitives":[{"attributes":{"POSITION":0}}]}],"accessors":[{"count":3,"type":"VEC3"}],"bufferViews":[{}]}', F(TRI)).replace(b'"asset":"a"', b'"asset":"\xff"')
# structure: other modes, no scene, scene index past the end, no nodes, a cycle, a node with no mesh, an empty primitive list
fx["modes"] = glb(doc([{"attributes": {"POSITION": 0}, "mode": 1}, {"attributes": {"POSITION": 0}, "mode": 4}, {"attributes": {"POSITION": 0}, "mode": 5}], [V3(3)], [{}]), F(TRI))
d = doc([{"attributes": {"POSITION": 0}}], [V3(3)], [{}]); del d["scenes"]; fx["no-scenes"] = glb(d, F(TRI))
fx["scene-wraps"] = glb(doc([{"attributes": {"POSITION": 0}}], [V3(3)], [{}], scene=4), F(TRI))
fx["cycle"] = glb(doc([{"attributes": {"POSITION": 0}}], [V3(3)], [{}], nodes=[{"mesh": 0, "children": [1], "translation": [1, 0, 0]}, {"children": [0, 0], "translation": [0, 1, 0]}]), F(TRI))
fx["empty-mesh"] = glb(doc([], [], []))
fx["no-position"] = glb(doc([{"attributes": {}}], [V3(3)], [{}]), F(TRI))
# values: NaN and infinities among good triangles, degenerate triangles, a triangle edge on against the camera
nan, inf = float("nan"), float("inf")
fx["nan"] = glb(doc([{"attributes": {"POSITION": 0}}], [V3(9)], [{}]), F(TRI + [nan, 0, 0, 1, 0, 0, 0, 1, 0] + [2, 2, 0, 3, 2, 0, 2, 3, 0]))
fx["inf"] = glb(doc([{"attributes": {"POSITION": 0}}], [V3(9)], [{}]), F(TRI + [inf, 0, 0, 1, 0, 0, 0, 1, 0] + [2, 2, 0, 3, 2, 0, 2, 3, 0]))
fx["degenerate"] = glb(doc([{"attributes": {"POSITION": 0}}], [V3(12)], [{}]), F(TRI + [1, 1, 1] * 3 + [0, 0, 1, 1, 1, 1, 2, 2, 1] + [0, 0, 2, 3, 0, 2, 0, 3, 2]))
fx["negative"] = glb(doc([{"attributes": {"POSITION": 0}, "indices": 1}], [V3(8), IX(36, 5125)], [{}, {"byteOffset": 96}], nodes=[{"mesh": 0, "scale": [-1, 1, 1]}]), F(CUBE_P) + struct.pack("<36I", *CUBE_I))
# what is no GLB at all
fx["empty"] = b""; fx["magic"] = b"glTF"; fx["header"] = struct.pack("<III", 0x46546C67, 2, 12); fx["text"] = b"this is not a model\n"
if __name__ == "__main__":
    os.makedirs(sys.argv[1], exist_ok=True)
    for k, v in fx.items(): open(os.path.join(sys.argv[1], k + ".glb"), "wb").write(v)
    print(len(fx), "fixtures")
