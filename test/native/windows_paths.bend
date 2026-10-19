import Base
import ../../src/server/host.bend as H
import ../../src/server/kicad.bend as K
import ../check.bend as C
import ../../src/server/freecad.bend as F
import ../../src/core/prompt.bend as PM

# Runtime metadata must resolve above bin for an explicit Windows .exe.
def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("Windows CLI candidate", K.Rt.candidate("C:/Program Files/KiCad/bin/kicad-cli.exe"), "C:/Program Files/KiCad/bin/kicad-cli.exe")
    C.Check.str("Windows runtime metadata root", K.Rt.root("C:\\Program Files\\KiCad\\bin\\kicad-cli.exe"), "C:/Program Files/KiCad")
    C.Check.str("POSIX runtime root", K.Rt.root("/opt/kicad/bin/kicad-cli"), "/opt/kicad")
    C.Check.str("macOS runtime root", K.Rt.root("/Applications/KiCad.app/Contents/MacOS/kicad-cli"), "/Applications/KiCad.app")
    C.Check.str("Windows FreeCAD GUI prompt", PM.Prompt.mech.gui("C:/FreeCAD/FreeCADCmd.exe"), "C:/FreeCAD/FreeCAD.exe")
    path : String <- H.Sys.native_path("/tmp/Backplane 日本/file.step")
    IO.print("BACKPLANE_NATIVE_PATH=" ++ path)
    freecad : String <- F.FreeCAD.find()
    IO.print("BACKPLANE_FREECAD_FOUND=" ++ freecad)
