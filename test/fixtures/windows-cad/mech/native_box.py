"""Native FreeCAD GUI smoke: a valid saved solid and colored STEP export."""
length_mm = 24.0
width_mm = 18.0
height_mm = 6.0
hole_diameter_mm = 4.0

import json
import math
import os
from pathlib import Path
import traceback

root = Path(__file__).resolve().parent
code = 1
try:
    import FreeCAD as App
    import Part
    import ImportGui

    document = App.newDocument("NativeWindowsBox")
    box = Part.makeBox(length_mm, width_mm, height_mm,
                       App.Vector(-length_mm / 2, -width_mm / 2, 0))
    hole = Part.makeCylinder(hole_diameter_mm / 2, height_mm)
    feature = document.addObject("PartDesign::Feature", "NativeBox")
    feature.Shape = box.cut(hole)
    feature.ViewObject.ShapeColor = (0.9, 0.35, 0.1)
    document.recompute()
    assert feature.Shape.isValid()
    expected = length_mm * width_mm * height_mm - math.pi * (hole_diameter_mm / 2) ** 2 * height_mm
    assert abs(feature.Shape.Volume - expected) < 0.001
    model = root / "native_box.FCStd"
    step = root / "native_box.step"
    document.saveAs(str(model))
    ImportGui.export([feature], str(step))
    App.closeDocument(document.Name)
    reopened = App.openDocument(str(model))
    assert reopened.getObject("NativeBox").Shape.isValid()
    restored = Part.Shape()
    restored.read(str(step))
    assert restored.isValid() and abs(restored.Volume - expected) < 0.001
    assert "COLOUR_RGB" in step.read_text()
    result = {"version": App.Version(), "volume_mm3": restored.Volume,
              "fcstd": str(model), "step": str(step), "colored_step": True}
    (root / "result.json").write_text(json.dumps(result, indent=2))
    (root / "result.log").write_text("WINDOWS_FREECAD_SAVE_REOPEN_COLORED_STEP_PASS\n")
    code = 0
except BaseException:
    (root / "result.log").write_text(traceback.format_exc())
finally:
    os._exit(code)
