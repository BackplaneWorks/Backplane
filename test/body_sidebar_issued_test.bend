import Base
import ./check.bend as C
import ./native_sidebar_projection_test.bend as F
import ./body_issued_test.bend as ISSUED
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/thin.bend as T
import ../src/core/thin-live.bend as L
import ../src/core/model.bend as M
import ../src/server/thin-actor.bend as A
import ../src/server/thin-body.bend as B

def picture_request(target: J.Json) -> J.Json:
  J.Json.obj([J.KV{"bodyPages", J.Num{"1"}}, J.KV{"target", target}, J.KV{"action", J.Str{""}}])

def first(xs: List<&2, A.Actor>) -> A.Actor:
  match xs:
    case a <> _: a
    case Nil{}: ISSUED.actor()

def rendered_actor(r: A.Rendered) -> A.Actor:
  match r:
    case A.Rendered{a, _}: a

def adjacent(a: A.Actor) -> A.Rendered:
  A.Actor.render.revision(a, True{}, U.Ui.state(F.base()), T.Index.empty(),
    picture_request(J.Json.get_obj(F.request(), "target")), "epoch", 10n, Nil{}, L.Stream.blank(), 7n)

def read_request() -> J.Json:
  ISSUED.request("previous-painted-body", "7", "t1")

def read_code(r: B.Read) -> String:
  match r:
    case B.Read{code, _}: code

def main() -> IO(Unit):
  do IO<Unit>:
    +issued : A.Actor = first(A.Bodies.record([ISSUED.actor()], ISSUED.result([ISSUED.part("previous-painted-body", "7", "t1")])))
    +page : A.Rendered = adjacent(issued)
    +kept : A.Actor = rendered_actor(page)
    C.Check.str("actual adjacent fresh frame preserves previously issued painted body", Bool.show(A.Bodies.allowed([kept], read_request())), "True")
    C.Check.str("next frame does not republish the previous body's metadata", Bool.show(String.contains(J.Json.show(A.Rendered.result(page)), "previous-painted-body")), "False")
    C.Check.str("fresh current frame keeps exact unsent IME draft", U.Ui.scratch(A.Actor.ui(kept), "t1"), "Unsent 日本語 IME")
    C.Check.str("fresh current frame keeps actual destination", U.Ui.sel(A.Actor.ui(kept)), "t1")
    +invalid : A.Actor = A.Sidebar.save(False{}, issued, "", J.Null{})
    C.Check.str("invalidating sidebar image cannot revoke an issued body", Bool.show(A.Bodies.allowed([invalid], read_request())), "True")
    +changed : A.Actor = A.Actor.refresh([kept], True{}, U.Ui.state(F.base()), J.Json.obj(Nil{}), J.Json.obj(Nil{}), 11n,
      picture_request(J.Json.obj([J.KV{"kind", J.Str{"thread"}}, J.KV{"id", J.Str{"t2"}}])), "view")
    C.Check.str("changing actual destination still revokes previous body scope", Bool.show(A.Bodies.allowed([changed], read_request())), "False")
    +new_view : A.Actor = A.Actor.refresh([kept], True{}, U.Ui.state(F.base()), J.Json.obj(Nil{}), J.Json.obj(Nil{}), 11n,
      picture_request(J.Json.get_obj(F.request(), "target")), "another-view")
    C.Check.str("changing actual view still revokes previous body scope", Bool.show(A.Bodies.allowed([new_view], read_request())), "False")
    +source : T.Row = T.Row{7n, M.Msg{"previous-painted-body", "t1", M.Assistant{}, "source", 1n}, Map.new(&2, Bool), T.Blocks.of("source")}
    +cursor : String = J.Json.get_str(B.Body.part(source, "epoch", J.Json.get_obj(F.request(), "target")), "cursor", "")
    +wrong_target : J.Json = J.Json.obj([J.KV{"target", J.Json.obj([J.KV{"kind", J.Str{"thread"}}, J.KV{"id", J.Str{"t2"}}])},
      J.KV{"entryId", J.Str{"previous-painted-body"}}, J.KV{"bodyVersion", J.Str{"7"}}, J.KV{"cursor", J.Str{cursor}}])
    +denied : B.Read = B.Body.row.read(source, "t2", wrong_target, "epoch")
    C.Check.str("actual target A cursor stays STALE after routing to B", read_code(denied), "STALE_CURSOR")
