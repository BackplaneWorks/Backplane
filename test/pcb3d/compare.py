#!/usr/bin/env python3
"""The native 3D board (kicad/pcb3d) against kicad-cli's export of the same board.

    compare.py BOARD.kicad_pcb DUMP_DIR KICAD_CLI [OUT_DIR]

DUMP_DIR holds what `pcb3d::stream::tests::dump` wrote: <stem>.native.glb and <stem>.parts.json. kicad-cli exports the board twice: bare
with outer copper (--include-tracks --include-zones, the hub's 3d-full), and with its parts. Each layer is rasterised from above at 20 um
on both sides (top and bottom split at the board's mid-plane) and compared as IoU, so the outline, cutouts, holes, pads, tracks, zones,
mask openings and silk are checked where they lie; holes are also counted. Each part is matched by its reference and its extent compared:
the centre's offset, its height, and that a back part hangs under the board. Prints a report; writes XOR images (red kicad-cli only,
green native only) into OUT_DIR. Exit 1 when a layer's IoU or a part's offset is past the limits below.
"""
import json, os, struct, subprocess, sys
import numpy as np
from PIL import Image, ImageDraw

RES = 0.02  # mm a pixel
# silk is mostly text: a TrueType face (kicad-cli draws its outlines, pcb3d the stroke font) and hairline strokes at 20 um keep its IoU lower
LIMITS = {"body": 0.985, "copper": 0.95, "mask": 0.95, "silk": 0.70}
PART_MM = 0.25  # a part's centre within this of kicad-cli's (x, y)

def glb_meshes(path):
    """Every primitive's world triangles as (node name chain, mesh name, Nx3x3 in board mm: x, y down, z up)."""
    b = open(path, "rb").read()
    n = struct.unpack_from("<I", b, 12)[0]
    j = json.loads(b[20:20 + n])
    bin_at = 20 + n + 8
    binb = b[bin_at:]
    def acc(i):
        a = j["accessors"][i]; v = j["bufferViews"][a["bufferView"]]
        off = v.get("byteOffset", 0) + a.get("byteOffset", 0)
        ct = {5126: np.float32, 5125: np.uint32, 5123: np.uint16, 5121: np.uint8}[a["componentType"]]
        k = {"SCALAR": 1, "VEC3": 3}[a["type"]]
        stride = v.get("byteStride", 0)
        if stride and stride != k * np.dtype(ct).itemsize:
            raw = np.frombuffer(binb, np.uint8, count=stride * a["count"], offset=off).reshape(a["count"], stride)
            return raw[:, :k * np.dtype(ct).itemsize].copy().view(ct).reshape(a["count"], k)
        return np.frombuffer(binb, ct, count=a["count"] * k, offset=off).reshape(a["count"], k)
    def mat(nd):
        if "matrix" in nd: return np.array(nd["matrix"], float).reshape(4, 4).T
        t = nd.get("translation", [0, 0, 0]); r = nd.get("rotation", [0, 0, 0, 1]); s = nd.get("scale", [1, 1, 1])
        x, y, z, w = r
        R = np.array([[1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)], [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)], [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)]])
        M = np.eye(4); M[:3, :3] = R * np.array(s); M[:3, 3] = t; return M
    out = []
    def walk(i, M, names):
        nd = j["nodes"][i]; M = M @ mat(nd); names = names + [nd.get("name", "")]
        if "mesh" in nd:
            me = j["meshes"][nd["mesh"]]
            for p in me["primitives"]:
                if p.get("mode", 4) != 4: continue
                P = acc(p["attributes"]["POSITION"]).astype(float)
                P = (np.c_[P, np.ones(len(P))] @ M.T)[:, :3]
                idx = acc(p["indices"]).reshape(-1) if "indices" in p else np.arange(len(P))
                T = P[idx.reshape(-1, 3)] * 1000.0
                T = T[:, :, [0, 2, 1]]  # glTF (x, y up, z) to board (x, y down, z up)
                out.append((names, me.get("name", ""), T))
        for c in nd.get("children", []): walk(c, M, names)
    for r in j["scenes"][j.get("scene", 0)]["nodes"]: walk(r, np.eye(4), [])
    return out

def raster(tris, box):
    x0, y0, x1, y1 = box
    w, h = int((x1 - x0) / RES) + 1, int((y1 - y0) / RES) + 1
    im = Image.new("1", (w, h), 0); d = ImageDraw.Draw(im)
    for t in tris:
        pts = [((p[0] - x0) / RES, (p[1] - y0) / RES) for p in t]
        if abs((pts[1][0] - pts[0][0]) * (pts[2][1] - pts[0][1]) - (pts[1][1] - pts[0][1]) * (pts[2][0] - pts[0][0])) < 1e-9: continue  # a wall seen edge-on
        d.polygon(pts, fill=1)
    return np.array(im, bool)

def holes(mask):
    """Regions of the board's bounding box not covered and not reached from its border: the holes and cutouts."""
    im = Image.fromarray(np.where(mask, 255, 0).astype(np.uint8)).convert("L")
    h, w = mask.shape
    pad = Image.new("L", (w + 2, h + 2), 0); pad.paste(im, (1, 1))
    ImageDraw.floodfill(pad, (0, 0), 128)
    ys, xs = np.nonzero(np.array(pad) == 0)
    n = 0
    for y, x in zip(ys.tolist(), xs.tolist()):
        if pad.getpixel((x, y)) != 0: continue  # filled by an earlier hole
        ImageDraw.floodfill(pad, (x, y), 64); n += 1
        if n > 20000: break
    return n

def iou(a, b):
    u = (a | b).sum(); return 1.0 if u == 0 else (a & b).sum() / u

def xor_png(a, b, path):
    img = np.zeros(a.shape + (3,), np.uint8); img[a & b] = (70, 70, 70); img[a & ~b] = (230, 40, 40); img[b & ~a] = (40, 210, 60)
    Image.fromarray(img).save(path)

def main():
    board, dump, cli = sys.argv[1], sys.argv[2], sys.argv[3]
    out = sys.argv[4] if len(sys.argv) > 4 else dump
    stem = os.path.splitext(os.path.basename(board))[0]
    os.makedirs(out, exist_ok=True)
    kfull, kparts = os.path.join(out, f"{stem}.kicad-full.glb"), os.path.join(out, f"{stem}.kicad-parts.glb")
    if not os.path.exists(kfull):
        subprocess.run([cli, "pcb", "export", "glb", "--no-components", "--include-pads", "--include-soldermask", "--include-silkscreen", "--include-tracks", "--include-zones", "--cut-vias-in-body", "-f", "-o", kfull, board], check=True, capture_output=True)
    if not os.path.exists(kparts):
        subprocess.run([cli, "pcb", "export", "glb", "--subst-models", "--include-pads", "--include-soldermask", "--include-silkscreen", "-f", "-o", kparts, board], check=True, capture_output=True)
    kk = glb_meshes(kfull); nn = glb_meshes(os.path.join(dump, f"{stem}.native.glb"))
    role = lambda name: name.rsplit("_", 1)[-1] if "_" in name else name
    kz = np.concatenate([t[:, :, 2].ravel() for _, m, t in kk if role(m) == "PCB"])
    mid = (kz.min() + kz.max()) / 2
    allk = np.concatenate([t for _, _, t in kk])
    box = (allk[:, :, 0].min() - 1, allk[:, :, 1].min() - 1, allk[:, :, 0].max() + 1, allk[:, :, 1].max() + 1)
    def pick(ms, want, side=None):
        ts = [t for _, m, t in ms if want(m)]
        if not ts: return np.zeros((0, 3, 3))
        T = np.concatenate(ts)
        if side is None: return T
        z = T[:, :, 2].mean(axis=1)
        return T[z > mid] if side == "top" else T[z < mid]
    K = lambda *roles: (lambda m: role(m) in roles)
    N = lambda *names: (lambda m: m.replace("pcb3d_", "") in names)
    rows, bad = [], []
    checks = [
        ("body", "board outline, cutouts, holes", pick(kk, K("PCB")), pick(nn, N("laminate")), "body"),
        ("copper top", "pads, tracks, zones, vias", pick(kk, K("copper", "pad", "via"), "top"), pick(nn, N("copper_top", "finish_top"), "top"), "copper"),
        ("copper bottom", "", pick(kk, K("copper", "pad", "via"), "bottom"), pick(nn, N("copper_bottom", "finish_bottom"), "bottom"), "copper"),
        ("mask top", "mask and its openings", pick(kk, K("soldermask"), "top"), pick(nn, N("mask_top"), "top"), "mask"),
        ("mask bottom", "", pick(kk, K("soldermask"), "bottom"), pick(nn, N("mask_bottom"), "bottom"), "mask"),
        ("silk top", "", pick(kk, K("silkscreen"), "top"), pick(nn, N("silk_top"), "top"), "silk"),
        ("silk bottom", "", pick(kk, K("silkscreen"), "bottom"), pick(nn, N("silk_bottom"), "bottom"), "silk"),
    ]
    for name, what, a, b, lim in checks:
        ra, rb = raster(a, box), raster(b, box)
        v = iou(ra, rb)
        extra = ""
        if name == "body": extra = f"holes {holes(ra)} vs {holes(rb)}"
        if ra.sum() == 0 and rb.sum() == 0: v = 1.0
        rows.append((name, v, f"{ra.sum() * RES * RES:.1f}", f"{rb.sum() * RES * RES:.1f}", extra))
        xor_png(ra, rb, os.path.join(out, f"{stem}.{name.replace(' ', '-')}.xor.png"))
        if v < LIMITS[lim]: bad.append(f"{name} IoU {v:.4f} < {LIMITS[lim]}")
    print(f"## {stem}\n\n| layer | IoU | kicad-cli mm² | native mm² | |\n|---|---|---|---|---|")
    for r in rows: print(f"| {r[0]} | {r[1]:.4f} | {r[2]} | {r[3]} | {r[4]} |")
    # parts: kicad-cli's nodes are named by reference
    kp = glb_meshes(kparts); ext = {}
    board_roles = {"PCB", "soldermask", "silkscreen", "pad", "copper", "via"}
    for names, m, t in kp:
        if role(m) in board_roles and any(stem in x or "PCB" in x for x in names[:2]) and len(names) <= 2: continue
        ref = next((x for x in names if x and not x.startswith("=>") and x[0].isalpha() and any(c.isdigit() for c in x) and len(x) <= 8), None)
        if ref is None: continue
        lo, hi = t.reshape(-1, 3).min(0), t.reshape(-1, 3).max(0)
        if ref in ext: lo, hi = np.minimum(lo, ext[ref][0]), np.maximum(hi, ext[ref][1])
        ext[ref] = (lo, hi)
    mine = json.load(open(os.path.join(dump, f"{stem}.parts.json")))
    got = {}
    for p in mine:
        lo, hi = np.array(p["box"][:3]), np.array(p["box"][3:])
        if p["ref"] in got: lo, hi = np.minimum(lo, got[p["ref"]][0]), np.maximum(hi, got[p["ref"]][1])
        got[p["ref"]] = (lo, hi, p["side"], p["model"])
    off, worst, flips, both, flipped, through = [], ("", 0.0), 0, 0, [], []
    for ref, (lo, hi) in ext.items():
        if ref not in got: continue
        glo, ghi, side, model = got[ref]
        if model == "box": continue
        both += 1
        c = (lo + hi) / 2; g = (glo + ghi) / 2
        d = float(np.hypot(*(c - g)[:2])); off.append(d)
        if d > worst[1]: worst = (ref, d)
        wrong = (side == "B" and not (ghi[2] <= mid + 0.05 and hi[2] <= mid + 0.05)) or (side == "F" and not (glo[2] >= mid - 0.05))
        # a part through the board (a THT pin, a connector's peg) reaches the other side in both models: only a disagreement is a flip
        if wrong and not ((glo[2] < mid) == (lo[2] < mid) and (ghi[2] > mid) == (hi[2] > mid)): flips += 1; flipped.append(ref)
        elif wrong: through.append(ref)
    off = np.array(off) if off else np.zeros(1)
    backs = sum(1 for r in got.values() if r[2] == "B" and r[3] != "box")
    print(f"\nparts: {both} matched by reference ({len(ext)} in kicad-cli's model, {len(got)} placed here, {backs} on the back); centre offset median {np.median(off) * 1000:.0f} um, "
          f"95% {np.percentile(off, 95) * 1000:.0f} um, worst {worst[1] * 1000:.0f} um ({worst[0]}); on the wrong side: {flips} {flipped}; through the board in both: {len(through)}")
    if np.percentile(off, 95) > PART_MM: bad.append(f"parts: 95% centre offset {np.percentile(off, 95):.3f} mm")
    if flips: bad.append(f"{flips} parts on the wrong side")
    if bad: print("\nFAIL: " + "; ".join(bad)); sys.exit(1)

if __name__ == "__main__":
    main()
