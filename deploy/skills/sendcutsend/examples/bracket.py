# An L bracket for SendCutSend, as a FreeCAD part script: copy it to mech/<part>.py and change the parameters.
# It reads the material's inside bend radius and K-factor from SendCutSend's live specs (scs.py material SKU),
# builds the formed part, and writes mech/<part>.FCStd, mech/<part>.step (the folded part, what you upload) and
# mech/<part>.flat.dxf (its flat pattern with the bend line on layer BEND, what the DFM check measures).
#   QT_QPA_PLATFORM=offscreen freecad mech/bracket.py     (colours kept)   or   freecadcmd -c "exec(open('mech/bracket.py').read())"
import json, math, os, subprocess, sys

SKU = "ALU-063"          # 5052 H32 aluminium, 0.063"; list others with scs.py materials --service bending
WIDTH = 80.0             # along the bend (mm)
BASE = 40.0              # outside length of the base flange (mm)
UP = 30.0                # outside height of the upright flange (mm)
BASE_HOLES = [(14.0, 12.0, 4.5), (66.0, 12.0, 4.5)]   # (x, y from the base's free edge, diameter) mm
UP_HOLES = [(40.0, 20.0, 4.37)]                       # (x, z from the bottom, diameter) mm: Ø4.37 is F-M3-1's hole
OUT = os.environ.get("PART_OUT", "mech")
NAME = os.environ.get("PART_NAME", "bracket")

# the sendcutsend skill's scripts folder (beside its SKILL.md); the agent fills it in when it copies this file
SKILL = os.environ.get("SCS_SKILL") or "SKILL_SCRIPTS_DIR"
sys.path.insert(0, SKILL)
import scs_sheet  # noqa: E402

log = open(os.path.join(OUT, NAME + ".log"), "w")
spec = json.loads(subprocess.run(["python3", "-I", os.path.join(SKILL, "scs.py"), "material", SKU], capture_output=True, text=True, check=True).stdout)
bend = spec["specs"]["bending_specs"]
T = float(spec["catalog"]["thickness"]) * 25.4
R = float(bend["effective_bend_radius"]) * 25.4
K = float(bend["k_factor"])
print(f"{SKU}: t {T:.3f} mm, inside radius {R:.3f} mm, K {K}", file=log)

import FreeCAD as App  # noqa: E402
import Part  # noqa: E402

yb = BASE - R - T                       # where the bend starts on the base
base = Part.makeBox(WIDTH, yb, T)
shell = Part.makeCylinder(R + T, WIDTH, App.Vector(0, yb, T + R), App.Vector(1, 0, 0)).cut(Part.makeCylinder(R, WIDTH, App.Vector(0, yb, T + R), App.Vector(1, 0, 0)))
shell = shell.common(Part.makeBox(WIDTH, R + T, T + R, App.Vector(0, yb, 0)))
up = Part.makeBox(WIDTH, T, UP - T - R, App.Vector(0, yb + R, T + R))
part = base.fuse(shell).fuse(up)
for x, y, d in BASE_HOLES:
    part = part.cut(Part.makeCylinder(d / 2, T * 3, App.Vector(x, y, -T)))
for x, z, d in UP_HOLES:
    part = part.cut(Part.makeCylinder(d / 2, T * 3, App.Vector(x, yb + R - T, z), App.Vector(0, 1, 0)))
part = part.removeSplitter()
assert part.isValid(), "invalid solid"

doc = App.newDocument(NAME)
obj = doc.addObject("Part::Feature", NAME)
obj.Shape = part
doc.recompute()
doc.saveAs(os.path.join(OUT, NAME + ".FCStd"))
step = os.path.join(OUT, NAME + ".step")
try:
    import FreeCADGui  # noqa: F401
    import ImportGui
    obj.ViewObject.ShapeColor = (0.78, 0.79, 0.80)  # bare aluminium
    ImportGui.export([obj], step)
except Exception:
    part.exportStep(step)

# the flat pattern: base, the bend's developed length, then the upright
ba = scs_sheet.bend_allowance(90, R, T, K)
L = yb + ba + (UP - T - R)
y_bend = yb + ba / 2
circles = [(x, y, d / 2) for x, y, d in BASE_HOLES] + [(x, yb + ba + (z - T - R), d / 2) for x, z, d in UP_HOLES]
scs_sheet.write_dxf(os.path.join(OUT, NAME + ".flat.dxf"), units="mm", outline=[(0, 0), (WIDTH, 0), (WIDTH, L), (0, L)], circles=circles, bends=[((0, y_bend), (WIDTH, y_bend))])
print(f"flat {WIDTH:.2f} x {L:.2f} mm, bend line at {y_bend:.3f} mm; volume {part.Volume:.1f} mm3", file=log)
log.close()
if "FreeCADGui" in sys.modules:
    os._exit(0)
