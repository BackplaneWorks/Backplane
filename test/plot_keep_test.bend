import Base
import ./check.bend as C
import ../src/server/plot.bend as PV
import ../src/core/plot.bend as P

# The plot actor keeps the sources a view watches, the last 3D models, and
# now the last 8 plots of saved versions (history steps): they never change
# and scrubbing comes back to them. A live file's plot no view watches goes.

def plotted(+key: String, +path: String) -> PV.Src:
  PV.Src{key, 0, path, 0, "", 0n, False{}, Some{P.Plot.of([], 0)}, ""}

def versions(+n: Nat, acc: List<&2, PV.Src>) -> List<&2, PV.Src>:
  match n:
    case 0n: acc
    case 1n+k: versions(k, plotted("v" ++ Nat.show(k), "git\t/r/.git\tc" ++ Nat.show(k) ++ "\tb.kicad_pcb") <> acc)

def keys(ss: List<&2, PV.Src>, acc: String) -> String:
  match ss:
    case Nil{}: acc
    case s <> t: keys(t, acc ++ PV.Src.key.of(s) ++ ",")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("a live plot no view watches goes", keys(PV.Src.gc([plotted("live", "/r/b.kicad_pcb")], Nil{}), ""), "")
    C.Check.str("a version's plot stays", keys(PV.Src.gc([plotted("v", "git\t/r/.git\tc1\tb.kicad_pcb")], Nil{}), ""), "v,")
    C.Check.str("the newest 8 versions stay", keys(PV.Src.gc(versions(10n, Nil{}), Nil{}), ""), "v0,v1,v2,v3,v4,v5,v6,v7,")
    C.Check.str("a version not plotted yet goes", keys(PV.Src.gc([PV.Src{"w", 0, "git\t/r/.git\tc\tb.kicad_pcb", 0, "", 0n, False{}, None{}, ""}], Nil{}), ""), "")
