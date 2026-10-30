import Base
import ../../src/server/plot.bend as PV
import ../../src/server/proc.bend as PR
import ../../src/core/json.bend as J

# Narrow probe of the actual exporter/cache/promotion functions. All paths and
# XDG_CACHE_HOME used by its JS runner are temporary; no hub is started.
def strings(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}: Nil{}
    case s <> t: J.Str{s} <> strings(t)

def flags(+path: String, +out: String) -> IO(Unit):
  IO.print(J.Json.show(J.Json.obj([
    J.KV{"compactParts", J.Json.arr(strings(PV.Pv.glb.flags(False{}, False{}, out, path)))},
    J.KV{"compactBare", J.Json.arr(strings(PV.Pv.glb.flags(True{}, False{}, out, path)))},
    J.KV{"fullParts", J.Json.arr(strings(PV.Pv.glb.flags(False{}, True{}, out, path)))},
    J.KV{"fullBare", J.Json.arr(strings(PV.Pv.glb.flags(True{}, True{}, out, path)))}])))

def paths(+path: String) -> IO(Unit):
  do IO<Unit>:
    a : String <- PV.Pv.board.out(path, False{})
    b : String <- PV.Pv.board.out(path, True{})
    c : String <- PV.Pv.board.out.recipe(path, False{}, False{})
    d : String <- PV.Pv.board.out.recipe(path, False{}, True{})
    e : String <- PV.Pv.board.out.recipe(path, True{}, True{})
    f : String <- PV.Pv.solid.out(False{}, False{}, False{}, True{}, path)
    g : String <- PV.Pv.solid.out(False{}, False{}, False{}, False{}, path)
    IO.print(J.Json.show(J.Json.obj([J.KV{"compactParts", J.Str{a}}, J.KV{"compactBare", J.Str{b}},
      J.KV{"recipeCompact", J.Str{c}}, J.KV{"fullParts", J.Str{d}}, J.KV{"fullBare", J.Str{e}},
      J.KV{"solidFull", J.Str{f}}, J.KV{"solidCompact", J.Str{g}}])))

def sources(+path: String) -> IO(Unit):
  do IO<Unit>:
    a : String <- PV.Pv.solid.out(True{}, False{}, False{}, False{}, path)
    b : String <- PV.Pv.solid.out(True{}, False{}, False{}, True{}, path)
    c : String <- PV.Pv.solid.out(False{}, True{}, False{}, False{}, path)
    d : String <- PV.Pv.solid.out(False{}, True{}, False{}, True{}, path)
    IO.print(J.Json.show(J.Json.obj([J.KV{"glbCompact", J.Str{a}}, J.KV{"glbFull", J.Str{b}},
      J.KV{"stepCompact", J.Str{c}}, J.KV{"stepFull", J.Str{d}}])))

def ran(r: PR.Ran) -> IO(Unit):
  match r:
    case PR.Ran{code, log}: IO.print(J.Json.show(J.Json.obj([J.KV{"code", J.Json.u32(code)}, J.KV{"log", J.Str{log}}])))

def dispatch(a: Bool, b: Bool, c: Bool, d: Bool, path: String, out: String) -> IO(Unit):
  match a b c d:
    case True{} _ _ _: flags(path, out)
    case False{} True{} _ _: paths(path)
    case False{} False{} True{} _: sources(path)
    case False{} False{} False{} True{}:
      IO.bind(PR.Ran, Unit, PV.Pv.board.ran(PR.Ran{Maybe.default(&2, U32, U32.read(out), 7), "probe"}, path), ran)
    case _ _ _ _: IO.print("{}")

def args(xs: List<&1, String>) -> IO(Unit):
  match xs:
    case _ <> +mode <> path <> rest:
      out = Maybe.default(&1, String, List.head(&1, String, rest), "")
      a = String.eq(mode, "flags")
      b = String.eq(mode, "paths")
      c = String.eq(mode, "sources")
      d = String.eq(mode, "promote")
      dispatch(a, b, c, d, path, out)
    case _: IO.print("{}")

def main() -> IO(Unit):
  IO.bind(List<&1, String>, Unit, IO.args(), args)
