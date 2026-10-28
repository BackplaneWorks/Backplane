import Base
import ./check.bend as C
import ./native_sidebar_projection_test.bend as F
import ./native_sidebar_fixture_dump.bend as D
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/thin.bend as T
import ../src/core/thin-live.bend as L
import ../src/server/thin-actor.bend as A

def action_request() -> J.Json:
  J.Json.obj([J.KV{"hub", J.Str{"h"}}, J.KV{"generation", J.Str{"1"}}, J.KV{"target", J.Json.get_obj(F.request(), "target")},
    J.KV{"nativeSidebar", J.Num{"1"}}, J.KV{"action", J.Str{"thread-tree"}}, J.KV{"value", J.Str{"h|t3"}}])

def initial() -> A.Actor:
  A.Actor.make(F.open(), "view", J.Json.get_obj(F.request(), "target"))

def action() -> A.Acted:
  A.Actor.acted(initial(), False{}, action_request(), "epoch", 10n)

# Main may owe an ordinary follow-up (for example fab.look). Use its
# unchanged actor path as control; count the requested mutation separately.
def ordinary() -> A.Rendered:
  A.Actor.render.revision(initial(), False{}, U.Ui.state(F.open()), T.Index.empty(), action_request(), "epoch", 10n, Nil{}, L.Stream.blank(), 7n)

def commands(r: A.Rendered) -> List<&2, J.Json>:
  J.Json.elems(J.Json.get_obj(A.Rendered.result(r), "cmds"))

def count(xs: List<&2, J.Json>, +method: String) -> Nat:
  match xs:
    case Nil{}: 0n
    case h <> rest:
      Nat.add(Bool.pick(Nat, String.eq(J.Json.get_str(A.Actor.rpc(h), "m", ""), method), 1n, 0n), count(rest, method))

def project(a: A.Acted) -> A.Rendered:
  A.Actor.render.acted(a, False{}, U.Ui.state(F.open()), T.Index.empty(), F.request(), "epoch", 10n, Nil{}, L.Stream.blank(), 7n)

def trials(n: Nat, a: A.Acted) -> A.Rendered:
  match n:
    case 0n: project(a)
    case 1n+k: trials(k, A.Acted.reduce(a))

def main() -> IO(Unit):
  do IO<Unit>:
    +first : A.Rendered = project(action())
    +last : A.Rendered = trials(8n, action())
    +u : U.Ui = A.Actor.ui(D.actor(last))
    C.Check.str("one real thread-tree action offers exactly one setting mutation", Nat.show(count(commands(first), "setting.set")), "1")
    C.Check.str("native post-action preserves main ordinary commands exactly", J.Json.show(J.Json.get_obj(A.Rendered.result(first), "cmds")), J.Json.show(J.Json.get_obj(A.Rendered.result(ordinary()), "cmds")))
    C.Check.str("eight projection trials retain exactly that command", J.Json.show(J.Json.get_obj(A.Rendered.result(last), "cmds")), J.Json.show(J.Json.get_obj(A.Rendered.result(first), "cmds")))
    C.Check.str("trials do not advance request counter again", Nat.show(U.Ui.rid(u)), Nat.show(U.Ui.rid(A.Actor.ui(D.actor(first)))))
    C.Check.str("native counter matches main ordinary action and followups", Nat.show(U.Ui.rid(u)), Nat.show(U.Ui.rid(A.Actor.ui(D.actor(ordinary())))))
    C.Check.str("trials preserve current destination", U.Ui.sel(u), "t1")
    C.Check.str("trials preserve accepted draft and IME", U.Ui.scratch(u, "t1"), "Unsent 日本語 IME")
    C.Check.str("accepted native draft matches fresh projection", J.Json.get_str(J.Json.get_obj(J.Json.get_obj(D.rendered(u, D.request(u)), "shown"), "thread"), "draft", "missing"), "Unsent 日本語 IME")
