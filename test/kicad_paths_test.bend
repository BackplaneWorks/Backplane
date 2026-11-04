import Base
import ./check.bend as C
import ../src/server/kicad.bend as K
import ../src/core/json.bend as J

def paths(cs: List<&2, K.KCandidate>) -> String:
  match cs:
    case Nil{}: ""
    case K.KCandidate{p, source} <> t:
      source ++ "=" ++ p ++ "\n" ++ paths(t)

def candidates(path: String, platform: String) -> String:
  paths(K.Rt.candidates("", "", "", "/app", "/data", "/user", path, platform, Nil{}))

def roundtrip(+p: String) -> String:
  J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(J.Json.show(J.Json.obj([J.KV{"path", J.Str{p}}]))), J.Null{}), "path", "")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("manual Windows root with backspace sequence", K.Rt.candidate("C:\\backplane_kicad\\"), "C:/backplane_kicad/bin/kicad-cli.exe")
    C.Check.str("manual Windows root with newline and tab sequences", K.Rt.candidate("D:\\new tools\\test\\"), "D:/new tools/test/bin/kicad-cli.exe")
    C.Check.str("explicit Windows executable", K.Rt.candidate("C:\\Program Files\\KiCad\\bin\\kicad-cli.exe"), "C:/Program Files/KiCad/bin/kicad-cli.exe")
    C.Check.str("POSIX candidate", K.Rt.candidate("/opt/kicad"), "/opt/kicad/bin/kicad-cli")
    C.Check.str("app candidate", K.Rt.candidate("/Applications/KiCad.app"), "/Applications/KiCad.app/Contents/MacOS/kicad-cli")
    C.Check.str("JSON backslashes stay literal", roundtrip("C:\\backplane_kicad\\new\\test\\"), "C:\\backplane_kicad\\new\\test\\")
    C.Check.str("JSON spaces and drives", roundtrip("D:\\Program Files\\KiCad\\bin\\kicad-cli.exe"), "D:\\Program Files\\KiCad\\bin\\kicad-cli.exe")
    C.Check.str("native Windows PATH drive and semicolon", Bool.pick(String, String.contains(candidates("C:\\backplane_kicad\\bin;D:\\new tools\\test\\bin", "CYGWIN_NT-10.0 x86_64"), "path=D:/new tools/test/bin/kicad-cli.exe\n"), "yes", "no"), "yes")
    C.Check.str("native Windows PATH preserves first drive", Bool.pick(String, String.contains(candidates("C:\\backplane_kicad\\bin;D:\\new tools\\test\\bin", "CYGWIN_NT-10.0 x86_64"), "path=C:/backplane_kicad/bin/kicad-cli.exe\n"), "yes", "no"), "yes")
    C.Check.str("single native Windows PATH drive", Bool.pick(String, String.contains(candidates("C:\\backplane_kicad\\bin", "CYGWIN_NT-10.0 x86_64"), "path=C:/backplane_kicad/bin/kicad-cli.exe\n"), "yes", "no"), "yes")
    C.Check.str("POSIX PATH", Bool.pick(String, String.contains(candidates("/one:/two", "Linux x86_64"), "path=/two/kicad-cli\n"), "yes", "no"), "yes")
    C.Check.str("Cygwin PATH stays colon separated with exe", Bool.pick(String, String.contains(candidates("/one:/two", "CYGWIN_NT-10.0 x86_64"), "path=/two/kicad-cli.exe\n"), "yes", "no"), "yes")
    C.Check.str("UNC runtime root", K.Rt.candidate("\\\\server\\new tools\\test\\"), "//server/new tools/test/bin/kicad-cli.exe")
    C.Check.str("forward Windows drive root", K.Rt.candidate("C:/"), "C:/bin/kicad-cli.exe")
    C.Check.str("Windows_NT PATH", Bool.pick(String, String.contains(candidates("C:/tools", "Windows_NT AMD64"), "path=C:/tools/kicad-cli.exe\n"), "yes", "no"), "yes")
    C.Check.str("managed Windows exact exe", Bool.pick(String, String.contains(candidates("", "CYGWIN_NT-10.0 x86_64"), "installed=/data/kicad/10.99.0-bp.6/bin/kicad-cli.exe\n"), "yes", "no"), "yes")
    C.Check.str("POSIX semicolon remains a filename character", Bool.pick(String, String.contains(candidates("/one;two:/three", "Linux x86_64"), "path=/one;two/kicad-cli\n"), "yes", "no"), "yes")
    C.Check.str("Cygwin UNC PATH still splits at colon", Bool.pick(String, String.contains(candidates("//server/tools:/two", "CYGWIN_NT-10.0 x86_64"), "path=/two/kicad-cli.exe\n"), "yes", "no"), "yes")
