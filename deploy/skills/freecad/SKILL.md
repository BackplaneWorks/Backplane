---
name: freecad
description: "Use for mechanical modeling, STEP export, assemblies, and fit checks in FreeCAD."
---

# freecad

Model with FreeCAD. Each part is one script in the project's `mech/` folder with its dimensions as named parameters at the top, in millimetres, Z up, the part's front facing -Y; it checks `shape.isValid()`, saves `mech/<part>.FCStd` and exports `mech/<part>.step`.

An assembly is one more script that places the parts and exports one STEP. Give every part a colour that reads as what it is (a printed housing, aluminium, rubber, the board's green): STEP files keep colours only when FreeCAD's GUI writes them, so run the script with the GUI offscreen (no window opens): `QT_QPA_PLATFORM=offscreen timeout 300 "<FreeCAD GUI>" mech/<part>.py`. In it, after `doc.recompute()`, set each object's `ViewObject.ShapeColor = (r, g, b)` (0 to 1), export with `ImportGui.export(objects, 'mech/<part>.step')`, write any messages to a log file (the GUI's output is not shown), and end with `os._exit(0)` or it keeps running.

Bring a board or vendor STEP into an assembly with `ImportGui.insert` so its colours stay. Before the first `ImportGui.insert`, run `App.ParamGet("User parameter:BaseApp/Preferences/Mod/Import").SetInt("ImportMode", 0)` (in memory only: nothing is saved): a person's Import mode of 2 makes `ImportGui.insert` wait forever offscreen.

The console build, `"<FreeCAD console>" -c "exec(open('mech/<part>.py').read())"`, is quicker for checking geometry but writes STEP without colours.

Use the GUI and console paths reported in the system prompt. Clear DISPLAY and WAYLAND_DISPLAY on graphical Linux shells before launching offscreen. Inspect the script log and check that the saved files exist before opening the STEP with `viewer_show` (kind `mech`).

If you need a static check, run the selected Backplane renderer with `--render mech/a.step mech/b.step`. Inspect the isometric image first in `mech/renders/`, which also contains front, top and right views. Report renderer errors. For a fit check, export the KiCad board with `kicad-cli pcb export step --board-only` unless component heights matter.
