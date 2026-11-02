import Base
import ./check.bend as C
import ./usage_page_test.bend as F
import ../src/core/client.bend as U
import ../src/core/i18n.bend as I
import ../src/core/json.bend as J
import ../src/web/dom.bend as D
import ../src/web/view.bend as W
import ../src/mobile/screen.bend as S
import ../src/mobile/hubs.bend as H

def words(fuel: Nat, nodes: List<&2, D.Node>, acc: String) -> String:
  match fuel nodes:
    case 0n _: acc
    case 1n+_ Nil{}: acc
    case 1n+k D.Txt{t} <> rest: words(k, rest, acc ++ t ++ "|")
    case 1n+k D.El{_, _, _, kids} <> rest: words(k, List.append(&2, D.Node, kids, rest), acc)

def check(label: String, yes: Bool) -> IO(Unit):
  C.Check.str(label, Bool.show(yes), "True")

def main() -> IO(Unit):
  do IO<Unit>:
    +u : U.Ui = U.Ui.adopt(U.Ui.init(1792100002n), F.fixture())
    +web : String = words(10000n, W.View.usage.page(I.En{}, u), "")
    +phone : String = J.Json.show(J.Json.arr(S.Mob.usage.page(I.En{}, u)))
    check("web cached label", String.contains(web, "Disconnected / cached"))
    check("phone cached label", String.contains(phone, "Disconnected / cached"))
    check("web controls", Bool.and(String.contains(web, "Today (UTC)"), String.contains(web, "Reported cost")))
    check("web real aggregate", String.contains(web, "210 total"))
    check("phone same aggregate", String.contains(phone, "210 total"))
    check("phone period action", String.contains(phone, "usage-period"))
    check("phone cost action", String.contains(phone, "usage-metric"))
    +hs : H.Hubs = H.Hubs{"fixture", 1792100002n, "a", [H.Hub{"a", u}, H.Hub{"b", u}]}
    +a : H.HAct = H.Hubs.act(hs, "usage-metric", "b|cost")
    +after : H.Hubs = H.Hubs.after(a)
    C.Check.str("named machine changes", U.Usage.page.metric(H.Hubs.ui(after, "b")), "cost")
    C.Check.str("focused machine untouched", U.Usage.page.metric(H.Hubs.ui(after, "a")), "tokens")
    C.Check.str("control does not move focus", H.Hubs.cur(after), "a")
    C.Check.str("control emits no socket request", Nat.show(List.length(&2, H.HCmd, H.Hubs.cmds(a))), "0")
    +bad : H.Hubs = H.Hubs.after(H.Hubs.act(hs, "usage-metric", "missing|cost"))
    C.Check.str("unknown machine never falls back", U.Usage.page.metric(H.Hubs.ui(bad, "a")), "tokens")
    C.Check.str("unknown machine preserves other", U.Usage.page.metric(H.Hubs.ui(bad, "b")), "tokens")
    +local : H.Hubs = H.Hubs.after(H.Hubs.act(hs, "usage-period", "today"))
    C.Check.str("unqualified control goes to current", U.Usage.page.period(H.Hubs.ui(local, "a")), "today")
    C.Check.str("other period remains independent", U.Usage.page.period(H.Hubs.ui(local, "b")), "30days")
    C.Check.str("hidden phone page produces no usage rows", Nat.show(List.length(&2, J.Json, H.Hubs.usage.rows(I.En{}, False{}, u))), "0")
    check("visible phone page has controls", String.contains(J.Json.show(J.Json.arr(H.Hubs.usage.rows(I.En{}, True{}, u))), "usage-period"))
