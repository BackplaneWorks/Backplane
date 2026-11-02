import Base
import ./check.bend as C
import ./allowance_ui_test.bend as Q
import ./chat_sidebar_projection_display_test.bend as PAGE
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/server/thin-actor.bend as A
import ../src/web/sidebar-slot.bend as SLOT

def ui() -> U.Ui:
  U.Ui.scratch_set(U.Ui.flag_on(Q.ui(), "allowance"), "t", "Before edit")

def key(u: U.Ui) -> String:
  A.SidebarImage.key.input(False{}, u, "epoch", "view", 1n)

def shown(u: U.Ui, cached: J.Json) -> J.Json:
  J.Json.get_obj(PAGE.frame(u, cached), "shown")

def original() -> J.Json:
  SLOT.Slot.sidebar(shown(ui(), J.Null{}))

def cache(u: U.Ui) -> J.Json:
  A.SidebarImage.lookup([A.SidebarImage{key(ui()), original()}], key(u))

def valid(o: J.Json) -> String:
  Bool.show(SLOT.Slot.is(o, "aside"))

def exact(+u: U.Ui, cached: J.Json) -> String:
  Bool.show(C.Check.same(J.Json.show(PAGE.frame(u, cached)), J.Json.show(PAGE.frame(u, J.Null{}))))

# Inspect the actual completed frame's composer, not a separately rendered one.
def matches(+o: J.Json, +key: String) -> Bool:
  Bool.and(String.eq(J.Json.get_str(o, "type", ""), "element"), String.eq(J.Json.get_str(o, "key", ""), key))

def nodes(o: J.Json, +key: String, hit: Bool) -> List<&2, J.Json>:
  match o hit:
    case J.Obj{fs} True{}: [J.Obj{fs}]
    case J.Obj{fs} False{}: nodes(fs, key, False{})
    case J.Field{_, +v, t} _:
      List.append(&2, J.Json, nodes(v, key, matches(v, key)), nodes(t, key, False{}))
    case J.Arr{xs} _: nodes(xs, key, False{})
    case J.Item{+h, t} _:
      List.append(&2, J.Json, nodes(h, key, matches(h, key)), nodes(t, key, False{}))
    case _ _: Nil{}

def node(+frame: J.Json, +key: String) -> J.Json:
  SLOT.Slot.one(nodes(frame, key, matches(frame, key)))

def attr(xs: List<&2, J.Json>, +name: String) -> String:
  match xs:
    case Nil{}: ""
    case +h <> t:
      Bool.pick(String, String.eq(J.Json.get_str(h, "name", ""), name), J.Json.get_str(h, "value", ""), attr(t, name))

def draft(+u: U.Ui, cached: J.Json) -> String:
  attr(J.Json.elems(J.Json.get_obj(node(shown(u, cached), "composer"), "attrs")), "value")

def label(+u: U.Ui, cached: J.Json) -> String:
  J.Json.get_str(Maybe.default(&2, J.Json, List.head(&2, J.Json, J.Json.elems(J.Json.get_obj(node(shown(u, cached), "allowance-current"), "kids"))), J.Null{}), "text", "")

def main() -> IO(Unit):
  do IO<Unit>:
    +ordinary : U.Ui = U.Ui.scratch_set(ui(), "t", "Edited ordinary draft")
    +ime : U.Ui = U.Ui.scratch_set(ui(), "t", "未確定 日本語 👩🏽‍💻")
    C.Check.str("ordinary draft is an asserted retained image HIT with quota popup open", valid(cache(ordinary)), "True")
    C.Check.str("IME draft is an asserted retained image HIT with quota popup open", valid(cache(ime)), "True")
    C.Check.str("ordinary HIT complete updated frame equals full render", exact(ordinary, cache(ordinary)), "True")
    C.Check.str("IME HIT complete updated frame equals full render", exact(ime, cache(ime)), "True")
    C.Check.str("actual HIT frame composer has edited ordinary draft", draft(ordinary, cache(ordinary)), "Edited ordinary draft")
    C.Check.str("actual HIT frame composer has edited IME draft", draft(ime, cache(ime)), "未確定 日本語 👩🏽‍💻")
    C.Check.str("actual HIT frame quota indicator is current", label(ime, cache(ime)), "39% left · shared")
    C.Check.str("actual HIT frame keeps expanded quota details", Bool.show(matches(node(shown(ime, cache(ime)), "allowance-details"), "allowance-details")), "True")
    +miss : U.Ui = U.Ui.scratch_set(ui(), "@f.pq", "different search")
    C.Check.str("relevant search scratch is an explicit MISS", J.Json.show(cache(miss)), "null")
    C.Check.str("relevant MISS complete frame equals fresh render", exact(miss, cache(miss)), "True")
    +bad : J.Json = A.SidebarImage.lookup([A.SidebarImage{key(ui()), J.Json.obj([J.KV{"type", J.Str{"element"}}, J.KV{"tag", J.Str{"main"}}, J.KV{"key", J.Str{"sidebar"}}])}], key(ime))
    C.Check.str("malformed cached node really reached lookup", Bool.show(Bool.not(String.eq(J.Json.show(bad), "null"))), "True")
    C.Check.str("malformed cached region falls back to exact full frame", exact(ime, bad), "True")
    C.Check.str("malformed fallback preserves actual IME composer", draft(ime, bad), "未確定 日本語 👩🏽‍💻")
    C.Check.str("malformed fallback preserves actual quota label", label(ime, bad), "39% left · shared")
