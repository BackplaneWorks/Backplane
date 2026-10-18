import Base
import ./check.bend as C
import ../src/app/chipx.bend as CX
import ../src/app/layout.bend as L
import ../src/core/model.bend as M

def repeated(n: Nat) -> List<&2, L.TLE>:
  match n:
    case 0n: Nil{}
    case 1n+k:
      L.TLE{M.Msg{"e", "thread", M.Assistant{}, "", 0n}, L.TLPart{Nil{}, Nil{}, Nil{}, ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"], Nil{}, Nil{}}} <> repeated(k)

def repeated.deps() -> List<&2, String>:
  CX.Cache.items(repeated(20n), Nil{})

def unique(+n: Nat) -> List<&2, String>:
  match n:
    case 0n: Nil{}
    case 1n+k: Nat.show(k) <> unique(k)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("plain timeline keeps stamp on unrelated changes", Bool.show(CX.Cache.same("k", ["k"], ["other"])), "True")
    C.Check.str("chip dependency invalidates timeline stamp", Bool.show(CX.Cache.same("k", ["k", "target"], ["target"])), "False")
    C.Check.str("unrelated chip target keeps stamp", Bool.show(CX.Cache.same("k", ["k", "target"], ["other"])), "True")
    C.Check.str("empty dirty keeps chip stamp", Bool.show(CX.Cache.same("k", ["k", "target"], Nil{})), "True")
    C.Check.str("changed locale rebuilds", Bool.show(CX.Cache.same("ja", ["en", "target"], Nil{})), "False")
    C.Check.str("global dirty reaches chips", Bool.show(CX.Cache.hit(["*"], ["target"])), "True")
    C.Check.str("capped dependency summary is conservative", Bool.show(CX.Cache.hit(["other"], ["*"])), "True")
    C.Check.str("hidden timeline drops stamp metadata", Nat.show(List.length(&2, String, CX.Cache.marks(False{}, False{}, "", Nil{}, ["k", "target"]))), "0")
    C.Check.str("200 references to ten targets keep ten dependencies", Nat.show(List.length(&2, String, repeated.deps())), "10")
    C.Check.str("repeated targets ignore unrelated dirty ids", Bool.show(CX.Cache.hit(["other"], repeated.deps())), "False")
    C.Check.str("repeated targets still invalidate named target", Bool.show(CX.Cache.hit(["j"], repeated.deps())), "True")
    C.Check.str("64 distinct targets retain precise summary", Nat.show(List.length(&2, String, CX.Cache.ids(unique(64n), Nil{}))), "64")
    C.Check.str("65 distinct targets retain conservative cap", Bool.show(CX.Cache.hit(["other"], CX.Cache.ids(unique(65n), Nil{}))), "True")
