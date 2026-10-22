import Base
import ../check.bend as C
import ../../src/server/kicad.bend as K
import ../../src/server/proc.bend as P

# (native: scripts/build-js.sh test/native/kicad_files.bend build/kicad_files)
# Which KiCad project a folder's threads work on (K.Files.read): one in the
# root, one only in a folder below it, several, and one named in
# .backplane.json. Fixtures are made under a temporary folder.

def Kf.rel(+p: String, +root: String) -> String:
  Bool.pick(String, String.starts_with(p, root), "~" ++ String.drop(p, String.length(root)), p)

def Kf.show(fs: K.Files, +root: String) -> String:
  match fs:
    case K.Files{+pro, +pcb, +sch, +all}:
      String.join([Kf.rel(pro, root), Kf.rel(pcb, root), Kf.rel(sch, root), String.join(String.lines(all), ",")], "|")

def Kf.sh() -> String:
  "set -e; r=$1; rm -rf \"$r\"; mkdir -p \"$r/top\" \"$r/deep/hw/main\" \"$r/many/a\" \"$r/many/b/c\" \"$r/many/.backplane/wt\" \"$r/named/x\" \"$r/named/y\" \"$r/none\"; cd \"$r\"; touch top/t.kicad_pro top/t.kicad_pcb top/t.kicad_sch deep/hw/main/m.kicad_pro deep/hw/main/m.kicad_pcb deep/hw/main/m.kicad_sch many/a/a.kicad_pro many/a/a.kicad_sch many/b/c/b.kicad_pro many/b/c/b.kicad_pcb many/.backplane/wt/w.kicad_pro named/x/x.kicad_pro named/y/y.kicad_pro named/y/y.kicad_pcb; printf '{\"project\": \"y/y.kicad_pro\"}' > named/.backplane.json"

def main() -> IO(Unit):
  do IO<Unit>:
    +r : String = "/tmp/bp-kicad-files-test"
    mk : P.Ran <- P.Proc.run("sh", ["-c", Kf.sh(), "sh", r], "/")
    top : K.Files <- K.Files.read(r ++ "/top")
    C.Check.str("in the root", Kf.show(top, r ++ "/top"), "~/t.kicad_pro|~/t.kicad_pcb|~/t.kicad_sch|t.kicad_pro")
    deep : K.Files <- K.Files.read(r ++ "/deep")
    C.Check.str("in a folder below", Kf.show(deep, r ++ "/deep"), "~/hw/main/m.kicad_pro|~/hw/main/m.kicad_pcb|~/hw/main/m.kicad_sch|hw/main/m.kicad_pro")
    many : K.Files <- K.Files.read(r ++ "/many")
    C.Check.str("several: the shallowest, each listed, worktrees left out", Kf.show(many, r ++ "/many"), "~/a/a.kicad_pro||~/a/a.kicad_sch|a/a.kicad_pro,b/c/b.kicad_pro")
    named : K.Files <- K.Files.read(r ++ "/named")
    C.Check.str("named in .backplane.json", Kf.show(named, r ++ "/named"), "~/y/y.kicad_pro|~/y/y.kicad_pcb||x/x.kicad_pro,y/y.kicad_pro")
    none : K.Files <- K.Files.read(r ++ "/none")
    C.Check.str("none", Kf.show(none, r ++ "/none"), "|||")
    rm : P.Ran <- P.Proc.run("rm", ["-rf", r], "/")
    return Unit{}
