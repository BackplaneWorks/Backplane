import Base
import ./check.bend as C
import ./native_sidebar_projection_test.bend as F
import ./native_sidebar_fixture_dump.bend as D
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/thin.bend as T
import ../src/core/thin-live.bend as L
import ../src/server/thin-actor.bend as A
import ../src/server/thin-view.bend as V

def request() -> J.Json:
  J.Json.obj([J.KV{"hub", J.Str{"h"}}, J.KV{"generation", J.Str{"1"}}, J.KV{"bodyPages", J.Num{"1"}}, J.KV{"target", J.Json.get_obj(F.request(), "target")},
    J.KV{"action", J.Str{"thread-tree"}}, J.KV{"value", J.Str{"t3"}}])

def initial() -> A.Actor:
  A.Actor.make(U.Ui.info_set(U.Ui.info_set(F.base(), "local.body.pages", "1"), "local.view.lazy", "1"), "view", J.Json.get_obj(F.request(), "target"))

def project(a: A.Acted) -> A.Rendered:
  A.Actor.render.acted(a, True{}, U.Ui.state(F.base()), T.Index.empty(), request(), "epoch", 10n, Nil{}, L.Stream.blank(), 7n)

def trials(n: Nat, a: A.Acted) -> A.Rendered:
  match n:
    case 0n: project(a)
    case 1n+k: trials(k, A.Acted.web_reduce(a))

def main() -> IO(Unit):
  do IO<Unit>:
    +acted : A.Acted = A.Actor.acted(initial(), True{}, request(), "epoch", 10n)
    +first : A.Rendered = project(acted)
    +last : A.Rendered = trials(10n, acted)
    C.Check.str("web fit trials never repeat the accepted ordinary action", J.Json.show(J.Json.get_obj(A.Rendered.result(last), "cmds")), J.Json.show(J.Json.get_obj(A.Rendered.result(first), "cmds")))
    C.Check.str("web fit trials never advance ordinary request counter", Nat.show(U.Ui.rid(A.Actor.ui(D.actor(last)))), Nat.show(U.Ui.rid(A.Actor.ui(D.actor(first)))))
    C.Check.str("fitting preserves selected thread", U.Ui.sel(A.Actor.ui(D.actor(last))), "t1")
    C.Check.str("fitting preserves IME and draft", U.Ui.scratch(A.Actor.ui(D.actor(last)), "t1"), "Unsent 日本語 IME")
    C.Check.str("fitted body metadata uses retained private policy", J.Json.show(J.Json.get_obj(V.Render.request(request(), A.Actor.ui(D.actor(last))), "bodyBudget")), "1")
    C.Check.str("fitted list capacity remains bounded", U.Ui.info(A.Actor.ui(D.actor(last)), "local.view.budget"), "1")
    +before : String = A.Actor.loading.key([initial()])
    C.Check.str("private fit changes invalidate complete-picture memo", Bool.show(String.eq(before, A.Actor.loading.key([D.actor(last)]))), "False")
