import Base
import ./check.bend as C
import ../src/core/pathrecords.bend as PR
import ../src/server/filelist.bend as F
import ../src/core/json.bend as J

def shown(paths: List<&2, String>) -> String:
  J.Json.show(J.Json.arr(F.FileList.strs(paths)))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("NUL records preserve newlines and Unicode", shown(PR.Paths.read("café.step\0line\nbreak.step\0雪.kicad_pcb\0")), "[\"café.step\",\"line\\nbreak.step\",\"雪.kicad_pcb\"]")
    C.Check.str("quotes and backslashes are path characters", shown(PR.Paths.read("quote\".step\0back\\slash.step\0")), "[\"quote\\\".step\",\"back\\\\slash.step\"]")
    C.Check.str("whitespace-only paths survive", shown(PR.Paths.read(" \0\t\0")), "[\" \",\"\\t\"]")
    C.Check.str("empty records are ignored", shown(PR.Paths.read("\0a\0\0b\0")), "[\"a\",\"b\"]")
    C.Check.str("unterminated final record survives", shown(PR.Paths.read("first\0last")), "[\"first\",\"last\"]")
    C.Check.str("packing preserves complete records", shown(PR.Paths.read(PR.Paths.pack(["line\nbreak.step", " ", "back\\slash.step"]))), "[\"line\\nbreak.step\",\" \",\"back\\\\slash.step\"]")
    C.Check.str("legacy kept file lists still decode", shown(PR.Paths.kept(False{}, "old.step\nother.step")), "[\"old.step\",\"other.step\"]")
    C.Check.str("new kept lists preserve an embedded newline", shown(PR.Paths.kept(True{}, "line\nbreak.step\0other.step")), "[\"line\\nbreak.step\",\"other.step\"]")
    C.Check.str("the server returns an unchanged JSON string array", J.Json.show(F.FileList.result("café.step\0line\nbreak.step\0")), "[\"café.step\",\"line\\nbreak.step\"]")
