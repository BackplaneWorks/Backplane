import os, sys, struct, json, array
# One FreeCAD process converting many STEP files. BP_JOBS: a file of lines "in<TAB>out"; BP_TOL, BP_ANG: linear (mm) and angular (radians) deflection; BP_GUI=1: the GUI build
# (offscreen; the only build that reads a STEP's colours), else the console build (tints only). Several workers share one job file and claim a job with
# os.open(out + ".claim", O_CREAT|O_EXCL). A job's out is written as out.part then renamed, then out.n holds "<tris>" (or "0 <why>" and no out).
import FreeCAD as App, Part
GUI = os.environ.get("BP_GUI") == "1"
if GUI:
    import FreeCADGui as Gui, ImportGui
tol = float(os.environ.get("BP_TOL", "0.03"))
ang = float(os.environ.get("BP_ANG", "0.2"))
def mesh(shape):  # the whole shape meshed once at both deflections; each face's tessellate then reads that mesh: a face meshed alone has no angle limit (a small circle came out a hexagon), and its edges did not meet its neighbours
    try:
        import MeshPart
        MeshPart.meshFromShape(Shape=shape, LinearDeflection=tol, AngularDeflection=ang, Relative=False)
    except Exception:
        pass
lin = lambda c: [x / 12.92 if x <= 0.04045 else ((x + 0.055) / 1.055) ** 2.4 for x in c]  # a STEP's colour is display sRGB, glTF's is linear light

def glb(src, groups, out):
    buf = bytearray(); views = []; accs = []; prims = []; mats = []; tris = 0
    for c, (pts, idx) in groups:
        if not idx: continue
        tris += len(idx) // 3
        flat = [x for v in pts for x in (v.x, v.y, v.z)]
        lo = [min(flat[k::3]) for k in range(3)]; hi = [max(flat[k::3]) for k in range(3)]
        views.append({"buffer": 0, "byteOffset": len(buf), "byteLength": 12 * len(pts), "target": 34962})
        buf += array.array("f", flat).tobytes()
        accs.append({"bufferView": len(views) - 1, "componentType": 5126, "count": len(pts), "type": "VEC3", "min": lo, "max": hi})
        views.append({"buffer": 0, "byteOffset": len(buf), "byteLength": 4 * len(idx), "target": 34963})
        buf += array.array("I", idx).tobytes()
        accs.append({"bufferView": len(views) - 1, "componentType": 5125, "count": len(idx), "type": "SCALAR"})
        mats.append({"pbrMetallicRoughness": {"baseColorFactor": lin(c) + [1.0], "metallicFactor": 0.0, "roughnessFactor": 0.8}})
        prims.append({"attributes": {"POSITION": len(accs) - 2}, "indices": len(accs) - 1, "material": len(mats) - 1})
    if not tris: raise RuntimeError("no faces could be meshed")
    gl = {"asset": {"version": "2.0", "generator": "backplane-step2glb (FreeCAD)", "extras": {"unit": "mm", "up": "Z"}},
          "buffers": [{"byteLength": len(buf)}], "bufferViews": views, "accessors": accs, "materials": mats,
          "meshes": [{"primitives": prims}], "nodes": [{"name": os.path.basename(src), "mesh": 0}], "scenes": [{"nodes": [0]}], "scene": 0}
    js = json.dumps(gl).encode(); js += b" " * (-len(js) % 4); buf += b"\0" * (-len(buf) % 4)
    with open(out + ".part", "wb") as f:
        f.write(struct.pack("<III", 0x46546C67, 2, 12 + 8 + len(js) + 8 + len(buf)))
        f.write(struct.pack("<II", len(js), 0x4E4F534A)); f.write(js)
        f.write(struct.pack("<II", len(buf), 0x004E4942)); f.write(bytes(buf))
    os.rename(out + ".part", out)
    return tris

def gui_job(src, out):  # freecad_gui.py's meshing and colours
    doc = App.newDocument("bpmesh")
    try:
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
            shape = o.Shape; mesh(shape)
            faces = shape.Faces
            for i, f in enumerate(faces):
                c = tint if plain else tuple(round(x, 3) for x in (cols[i][:3] if len(cols) == len(faces) else (cols[0][:3] if cols else base)))
                p, t = f.tessellate(tol)
                if not t: continue
                g = groups.setdefault(c, ([], []))
                b0 = len(g[0]); g[0].extend(p); g[1].extend(k for tri in t for k in (b0 + tri[0], b0 + tri[1], b0 + tri[2]))
        return glb(src, list(groups.items()), out)
    finally:
        App.closeDocument(doc.Name)

def console_job(src, out):  # freecad_console.py's meshing
    s = Part.read(src)
    shapes = list(s.Solids) or [s]
    pal = [(0.8, 0.8, 0.8), (0.6, 0.65, 0.72), (0.74, 0.68, 0.58), (0.52, 0.62, 0.52), (0.7, 0.58, 0.64), (0.58, 0.7, 0.72)]
    groups = [([], []) for _ in pal]
    for i, so in enumerate(shapes):
        pts, idx = groups[i % len(pal)]
        mesh(so); p, f = so.tessellate(tol)
        base = len(pts); pts.extend(p)
        for t in f: idx.extend((base + t[0], base + t[1], base + t[2]))
    return glb(src, list(zip(pal, groups)), out)

try:
    if GUI:
        # see freecad_gui.py: a person's preferences can make a STEP import wait forever offscreen. In memory only: this process ends with os._exit.
        App.ParamGet("User parameter:BaseApp/Preferences/Mod/Import").SetInt("ImportMode", 0)
        App.ParamGet("User parameter:BaseApp/Preferences/Mod/Part/STEP").SetBool("VisibleImportDialog", False)
    jobs = [l.rstrip("\n").split("\t") for l in open(os.environ["BP_JOBS"]) if "\t" in l]
    for src, out in jobs:
        try:
            fd = os.open(out + ".claim", os.O_CREAT | os.O_EXCL | os.O_WRONLY); os.close(fd)
        except OSError:
            continue
        try:
            n = (gui_job if GUI else console_job)(src, out)
            msg = str(n)
        except BaseException as e:
            msg = "0 " + repr(e)
        with open(out + ".n", "w") as f: f.write(msg)
except BaseException as e:
    sys.stderr.write(repr(e))
os._exit(0)
