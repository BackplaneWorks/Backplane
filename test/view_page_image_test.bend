import Base
import ./check.bend as C
import ./chat_sidebar_projection_test.bend as F
import ./chat_sidebar_projection_display_test.bend as PAGE
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/thin.bend as T
import ../src/core/thin-live.bend as L
import ../src/server/thin-actor.bend as A
import ../src/server/thin-view.bend as V
import ../src/web/sidebar-slot.bend as SLOT

def catalog(+n: Nat, st: M.State) -> M.State:
  match n:
    case 0n: st
    case 1n+k: catalog(k, M.State.apply(M.State.apply(st, M.ProjectCreated{"p" ++ Nat.show(n), "Visible project " ++ Nat.show(n), "/tmp/project", n}),
      M.ThreadCreated{"t" ++ Nat.show(n), "p" ++ Nat.show(n), "Thread " ++ Nat.show(n), M.Local{}, "", "", "codex", "", n}))

def state() -> M.State:
  M.State.setting(catalog(20n, F.state()), "onboard.done", "1")

def prepare(u: U.Ui) -> U.Ui:
  V.Render.ui(u, state(), T.Index.empty(), V.Target.json("thread", "self"), Nil{}, L.Stream.blank(), 100n)

def base() -> U.Ui:
  prepare(U.Ui.info_set(U.Ui.info_set(F.ordinary(state(), 100n), "local.view.lazy", "1"), "local.body.pages", "1"))

def key(u: U.Ui) -> String:
  A.SidebarImage.key.input(False{}, u, "epoch", "view", 1n)

def sidebar(u: U.Ui) -> J.Json:
  SLOT.Slot.sidebar(J.Json.get_obj(PAGE.frame(u, J.Null{}), "shown"))

def action(attrs: List<&2, J.Json>) -> Bool:
  match attrs:
    case Nil{}: False{}
    case +a <> rest: Bool.or(String.eq(J.Json.get_str(a, "action", ""), "list-page"), action(rest))

def value(attrs: List<&2, J.Json>) -> String:
  match attrs:
    case Nil{}: ""
    case +a <> rest: Bool.pick(String, String.eq(J.Json.get_str(a, "name", ""), "data-value"), J.Json.get_str(a, "value", ""), value(rest))

def controls(j: J.Json) -> List<&2, String>:
  match j:
    case J.Obj{+fs}:
      List.append(&2, String, Bool.pick(List<&2, String>, action(J.Json.elems(J.Json.get_obj(J.Obj{fs}, "attrs"))), [value(J.Json.elems(J.Json.get_obj(J.Obj{fs}, "attrs")))], Nil{}), controls(fs))
    case J.Arr{xs}: controls(xs)
    case J.Item{h, tail}: List.append(&2, String, controls(h), controls(tail))
    case J.Field{_, v, tail}: List.append(&2, String, controls(v), controls(tail))
    case _: Nil{}

def main() -> IO(Unit):
  do IO<Unit>:
    +u : U.Ui = base()
    +node : J.Json = sidebar(u)
    +edited : U.Ui = U.Ui.scratch_set(u, "self", "New 未確定 draft")
    +hit : J.Json = A.SidebarImage.lookup([A.SidebarImage{key(u), node}], key(edited))
    C.Check.str("bounded sidebar ordinary IME edit takes an actual image hit", Bool.show(SLOT.Slot.is(hit, "aside")), "True")
    +cached : J.Json = PAGE.frame(edited, hit)
    +fresh : J.Json = PAGE.frame(edited, J.Null{})
    C.Check.str("bounded cached frame equals complete fresh frame", F.equal(J.Json.show(cached), J.Json.show(fresh)), "same")
    C.Check.str("complete current composer retains actual IME draft", Bool.show(String.contains(J.Json.show(cached), "New 未確定 draft")), "True")
    +command : String = Maybe.default(&2, String, List.head(&2, String, controls(node)), "")
    C.Check.str("large real sidebar offers a reachable paging control", Bool.show(String.is_empty(command)), "False")
    +next : U.Ui = prepare(U.Act.ui(U.Ui.act(u, "list-page", command)))
    C.Check.str("list-page changes private sidebar image identity", F.equal(key(u), key(next)), "different")
    C.Check.str("list-page changes actual bounded sidebar rows", F.equal(J.Json.show(node), J.Json.show(sidebar(next))), "different")
    +miss : J.Json = A.SidebarImage.lookup([A.SidebarImage{key(u), node}], key(next))
    C.Check.str("new range cannot reuse the previous sidebar", J.Json.show(miss), "null")
    C.Check.str("new range renders a complete fresh frame after miss", F.equal(J.Json.show(PAGE.frame(next, miss)), J.Json.show(PAGE.frame(next, J.Null{}))), "same")
    C.Check.str("malformed cached tree reaches exact complete fallback", F.equal(J.Json.show(PAGE.frame(edited, J.Str{"malformed"})), J.Json.show(fresh)), "same")
