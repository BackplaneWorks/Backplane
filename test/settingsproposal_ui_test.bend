import Base
import ./check.bend as C
import ./i18n_webphone_test.bend as T
import ../src/core/i18n.bend as I
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/web/view.bend as W
import ../src/web/dom.bend as D
import ../src/mobile/screen.bend as S

def ask() -> M.Ask:
  M.Ask{"p1", "t1", "approval", "settings_proposal", "Enabled: off → on\nHierarchy: a → b", "", True{}, ""}

def web() -> String:
  T.words(W.View.ask.std(I.En{}, ask(), ""))

def phone() -> String:
  J.Json.show(S.Mob.ask.std.loc(I.En{}, ask(), ""))

def actions.go(fuel: Nat, nodes: List<&2, D.Node>) -> String:
  match fuel nodes:
    case 0n _: ""
    case 1n+_ Nil{}: ""
    case 1n+n D.Txt{_} <> rest: actions.go(n, rest)
    case 1n+n D.El{_, _, attrs, kids} <> rest:
      W.View.attr(attrs, "data-value") ++ "|" ++ actions.go(n, List.append(&2, D.Node, kids, rest))

def actions() -> String:
  actions.go(1000n, [W.View.ask.std(I.En{}, ask(), "")])

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("web shows exact proposed values", Bool.show(String.contains(web(), "Enabled: off → on")), "True")
    C.Check.str("web shows explicit approval", Bool.show(String.contains(web(), "Approve")), "True")
    C.Check.str("web approval bound to ask", Bool.show(String.contains(actions(), "p1|accept")), "True")
    C.Check.str("web offers no session grant", Bool.show(String.contains(actions(), "p1|session")), "False")
    C.Check.str("phone shows exact proposed values", Bool.show(String.contains(phone(), "Enabled: off → on")), "True")
    C.Check.str("phone approval bound to ask", Bool.show(String.contains(phone(), "p1|accept")), "True")
    C.Check.str("phone offers no session grant", Bool.show(String.contains(phone(), "p1|session")), "False")
