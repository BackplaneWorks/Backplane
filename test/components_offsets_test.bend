import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/components.bend as CC
import ../src/server/components.bend as S

# A first read's pages (server/components.bend's Components.chain), the
# first carrying every page's offset ("page.offsets") so the viewer can ask
# the rest at once; every page otherwise as Components.page makes it.
def rows(+n: Nat, +acc: String) -> String:
  match n:
    case 0n: acc
    case 1n+k: rows(k, Bool.pick(String, String.is_empty(acc), "", acc ++ ",") ++ "{\"id\":\"u" ++ Nat.show(k) ++ "\",\"reference\":\"R" ++ Nat.show(k) ++ "\",\"pads\":[],\"graphics\":[]}")

def data() -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse("{\"schema\":1,\"source\":\"/p/b.kicad_pcb\",\"schematic\":\"/p/b.kicad_sch\",\"sourceRevision\":\"r1\",\"schematicRevision\":\"s1\",\"components\":[" ++ rows(70n, "") ++ "]}"), J.Null{})

def offs(ps: List<&2, J.Json>) -> String:
  match ps:
    case Nil{}: ""
    case p <> t: Nat.show(J.Json.get_nat(J.Json.get_obj(p, "page"), "offset", 0n)) ++ " " ++ offs(t)

def first(ps: List<&2, J.Json>) -> J.Json:
  match ps:
    case Nil{}: J.Null{}
    case p <> _: p

def second(ps: List<&2, J.Json>) -> J.Json:
  match ps:
    case _ <> p <> _: p
    case _: J.Null{}

def main() -> IO(Unit):
  +ps = S.Components.first.go(S.Components.chain(4097n, True{}, data(), 0n, Nil{}))
  do IO<Unit>:
    C.Check.str("70 rows make three pages", offs(ps), "0 32 64 ")
    C.Check.str("the first page names every offset", J.Json.show(J.Json.get_obj(J.Json.get_obj(first(ps), "page"), "offsets")), "[0,32,64]")
    C.Check.str("and is otherwise the page as before", J.Json.show(J.Json.get_obj(first(ps), "components")), J.Json.show(J.Json.get_obj(CC.Components.page(data(), 0n, "", ""), "components")))
    C.Check.str("its cursor as before", J.Json.show(J.Json.get_obj(J.Json.get_obj(first(ps), "page"), "next")), "32")
    C.Check.str("a later page is unchanged", J.Json.show(second(ps)), J.Json.show(CC.Components.page(data(), 32n, "r1", "s1")))
