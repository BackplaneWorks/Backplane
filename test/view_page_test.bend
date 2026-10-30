import Base
import ./check.bend as C
import ../src/core/view-page.bend as P
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/json.bend as J

def ids(+n: Nat) -> List<&2, String>:
  match n:
    case 0n: Nil{}
    case 1n+k: Nat.show(n) <> ids(k)

def lazy() -> M.State:
  M.State.setting(M.State.empty(), "local.view.lazy", "1")

def command(p: P.Page) -> String:
  match p:
    case P.Page{scope, _, start, limit, _}:
      J.Json.show(J.Json.obj([J.KV{"scope", J.Str{scope}}, J.KV{"offset", J.Json.nat(Nat.add(start, limit))}]))

def count(p: P.Page) -> Nat:
  List.length(&2, String, P.Page.ids(p))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("pressure reads retained one-row capacity exactly", Nat.show(P.Page.limit("threads", "1")), "1")
    C.Check.str("empty private budget uses normal finite capacity", Nat.show(P.Page.limit("threads", "")), "16")
    +all : List<&2, String> = ids(600n)
    +first : P.Page = P.Page.of(lazy(), "threads", all, "1")
    C.Check.str("600 source rows render16 plus actual selected extra", Nat.show(count(first)), "17")
    C.Check.str("selected outside ordinary range remains present", Bool.show(P.Page.has(P.Page.ids(first), "1")), "True")
    +positions : String = P.Page.positions("{}", command(first))
    +next : P.Page = P.Page.of(M.State.setting(lazy(), "local.view.pages", positions), "threads", all, "")
    C.Check.str("next page reaches new rows", Bool.show(P.Page.has(P.Page.ids(next), "584")), "True")
    C.Check.str("next page never silently carries old ordinary rows", Bool.show(P.Page.has(P.Page.ids(next), "600")), "False")
    +changed : P.Page = P.Page.of(M.State.setting(lazy(), "local.view.pages", positions), "threads", "new" <> all, "")
    C.Check.str("source membership change resets list position", Bool.show(P.Page.has(P.Page.ids(changed), "new")), "True")
    +squeezed : M.State = M.State.setting(lazy(), "local.view.budget", "2")
    C.Check.str("a squeezed budget never hides a project", Nat.show(count(P.Page.of(squeezed, "projects", ids(60n), ""))), "60")
    C.Check.str("a squeezed budget never hides a live thread", Nat.show(count(P.Page.of(squeezed, "live", ids(60n), ""))), "60")
    C.Check.str("the project list has no pager", Bool.show(String.is_empty(P.Page.scope_of(P.Page.of(squeezed, "projects", ids(60n), "")))), "True")
    C.Check.str("a settled shelf still pages", Nat.show(count(P.Page.of(squeezed, "threads", ids(60n), ""))), "2")
    C.Check.str("legacy caller preserves complete source", Nat.show(count(P.Page.of(M.State.empty(), "threads", all, ""))), "600")
    +u : U.Ui = U.Ui.info_set(U.Ui.info_set(U.Ui.scratch_set(U.Ui.select.k(U.Ui.init(1n), "a"), "a", "IME 日本語 draft"), "local.view.lazy", "1"), "local.view.pages", "{}")
    +a : U.Act = U.Ui.act(u, "list-page", command(first))
    +after : U.Ui = U.Act.ui(a)
    C.Check.str("local page never changes current thread", U.Ui.sel(after), U.Ui.sel(u))
    C.Check.str("local page never changes selected draft", U.Ui.scratch(after, "a"), "IME 日本語 draft")
    C.Check.str("local page emits no ordinary side effect", Nat.show(List.length(&2, U.Cmd, U.Act.cmds(a))), "0")
