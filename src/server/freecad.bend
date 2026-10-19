import Base
import ./proc.bend as P

# FreeCAD, for agents' mechanical work. It runs headless (freecadcmd runs
# a Python script with FreeCAD's modules) and Backplane never bundles it:
# BACKPLANE_FREECAD names one, else the first of FreeCAD's command names
# on PATH (freecad.cmd is the snap's), then macOS's app bundle. "" when
# there is none, and Settings and the agents say where to get it.

def FreeCAD.sh() -> String:
  "for c in \"$BACKPLANE_FREECAD\" freecadcmd FreeCADCmd freecad.cmd /Applications/FreeCAD.app/Contents/Resources/bin/freecadcmd; do [ -n \"$c\" ] || continue; if command -v cygpath >/dev/null 2>&1; then c=$(cygpath -u \"$c\"); fi; p=$(command -v \"$c\" 2>/dev/null) && [ -n \"$p\" ] && { echo \"$p\"; exit 0; }; done; exit 1"

def FreeCAD.found(r: P.Ran) -> String:
  match r:
    case P.Ran{0, +out}:
      String.trim(out)
    case _:
      ""

def FreeCAD.find() -> IO(String):
  IO.bind(P.Ran, String, P.Proc.run("sh", ["-c", FreeCAD.sh()], "/"), r => IO.pure(String, FreeCAD.found(r)))
