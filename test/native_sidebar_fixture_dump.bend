import Base
import ./native_sidebar_projection_test.bend as F
import ./native_sidebar_scale_test.bend as K
import ../src/core/native-sidebar.bend as N
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/presentation.bend as P
import ../src/core/thin.bend as T
import ../src/core/thin-live.bend as L
import ../src/server/thin-actor.bend as A
import ../src/server/thin-view.bend as V

def request(+u: U.Ui) -> J.Json:
  +base = J.Json.obj([J.KV{"hub", J.Str{"h"}}, J.KV{"generation", J.Str{"1"}}, J.KV{"epoch", J.Str{"epoch"}}, J.KV{"viewId", J.Str{"view"}},
    J.KV{"renderer", J.Str{"ios-v1"}}, J.KV{"cid", J.Str{"fixture"}}, J.KV{"requestRevision", J.Num{"1"}},
    J.KV{"target", J.Json.get_obj(F.request(), "target")}, J.KV{"session", P.Session.pack(u)}])
  Bool.pick(J.Json, N.Ns.enabled(u), N.Obj.put(N.Obj.put(base, "nativeSidebar", J.Num{"1"}), "target", J.Json.get_obj(N.Ns.ctx(u), "target")), base)

def result(r: A.Rendered) -> J.Json:
  match r:
    case A.Rendered{_, value}: value

def answer(+u: U.Ui, +p: J.Json) -> A.Rendered:
  A.Actor.render.acted(A.Actor.acted(A.Actor.make(u, "view", J.Json.get_obj(p, "target")), False{}, p, "epoch", 10n), False{}, U.Ui.state(u), T.Index.empty(), p, "epoch", 10n, Nil{}, L.Stream.blank(), 7n)

def rendered(u: U.Ui, p: J.Json) -> J.Json:
  result(answer(u, p))

def actor(r: A.Rendered) -> A.Actor:
  match r:
    case A.Rendered{a, _}: a

def output(name: String, +u: U.Ui) -> IO(Unit):
  +p = request(u)
  IO.print(J.Json.show(J.Json.obj([J.KV{"name", J.Str{name}}, J.KV{"request", p}, J.KV{"result", rendered(u, p)}])))

def paging_request(+u: U.Ui, +initial: J.Json) -> J.Json:
  +project = Maybe.default(&2, J.Json, List.head(&2, J.Json, J.Json.elems(J.Json.get_obj(J.Json.get_obj(initial, "shown"), "projects"))), J.Null{})
  N.Obj.put(N.Obj.put(N.Obj.put(N.Obj.put(request(u), "action", J.Str{"native-sidebar-page"}), "value", J.Json.get_obj(J.Json.get_obj(J.Json.get_obj(project, "settledPage"), "next"), "value")), "requestRevision", J.Num{"2"}), "viewRevision", J.Num{"1"})

def output_paging() -> IO(Unit):
  +first = answer(F.open(), request(F.open()))
  +u = A.Actor.ui(actor(first))
  +p = paging_request(u, result(first))
  IO.print(J.Json.show(J.Json.obj([J.KV{"name", J.Str{"paging-action"}}, J.KV{"request", p}, J.KV{"result", rendered(u, p)}])))

def main() -> IO(Unit):
  do IO<Unit>:
    output("legacy-optout", F.base())
    output("closed", F.native(F.base()))
    output("open", F.open())
    output("next", F.go_next(F.open()))
    output_paging()
    output("selected-descendant", N.Ns.reduce(N.Ns.reduce(N.Ns.reduce(N.Ns.reduce(F.child_ui())))))
    output("filtered-selected-extra", F.filtered())
    output("fresh-home", F.fresh_home())
    output("dangling-home", F.dangling_home())
    output("removed-project-home", F.removed_project_home())
    output("active", F.native(U.Ui.scratch_set(F.base(), "@local.view", "active")))
    output("adaptive-metadata", K.pressure())
