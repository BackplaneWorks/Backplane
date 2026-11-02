import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/usage.bend as UZ
import ../src/core/usage_page.bend as UP
import ../src/core/client.bend as U

def fixture() -> M.State:
  M.State.replay([
    M.ProjectCreated{"p", "Board", "/tmp/usage-fixture", 1n},
    M.ThreadCreated{"t", "p", "Work", M.Local{}, "", "", "claude", "m", 1n},
    M.TurnUsage{"t", "claude", "m", "one", 10n, 20n, 30n, 40n, 5n, 100000000n, "irwotc", "", 1792100000n},
    M.TurnUsage{"t", "codex", "m", "two", 100n, 0n, 0n, 10n, 2n, 0n, "iot", "", 1792100001n},
    M.TurnUsage{"t", "codex", "m", "unknown", 0n, 0n, 0n, 0n, 0n, 0n, "", "", 1792100002n}], M.State.empty())

def text(rs: List<&2, UZ.URow>) -> String:
  match rs:
    case Nil{}: ""
    case +r <> rest: UZ.URow.label(r) ++ " | " ++ UZ.URow.note(r) ++ "\n" ++ text(rest)

def many(+n: Nat, st: M.State) -> M.State:
  match n:
    case 0n: st
    case 1n+k:
      many(k, M.State.tokens(st, "t", "codex", "model" ++ Nat.show(k), 1n, 0n, 0n, 1n, 0n, 0n, "io", "key" ++ Nat.show(k), "", 1792100000n))

def check(+label: String, value: Bool) -> IO(Unit):
  C.Check.str(label, Bool.show(value), "True")

def main() -> IO(Unit):
  do IO<Unit>:
    +st : M.State = fixture()
    +v : String = UP.Up.total(st, 1792100002n, "month")
    C.Check.str("sum excludes reasoning subset", Nat.show(UP.Up.tokens(v)), "210")
    C.Check.str("unknown token turn remains visible", UP.Up.coverage(v), "2 of 3 recorded turns supplied token readings. Missing token fields remain unknown.")
    C.Check.str("partial reported cost", UP.Up.cost(v), "$0.1000 reported for 1 of 3 turns; remaining costs unknown.")
    C.Check.str("cost absent is unknown", UP.Up.cost("1/1/0/0/1/0/0/0/1"), "Unknown: no turn reported a cost.")
    C.Check.str("explicit zero cost stays reported", UP.Up.cost("1/1/0/0/1/0/0/1/1"), "$0.0000 reported for 1 of 1 turns.")
    C.Check.str("small reported cost is not rounded to zero", UP.Up.usd(1n), "<$0.0001")
    C.Check.str("invalid period defaults to rolling month", UP.Up.period("24h"), "30days")
    C.Check.str("invalid metric cleans to tokens", UP.Up.metric("bill"), "tokens")
    C.Check.str("today crosses UTC boundary", UP.Up.total(st, 1792100002n, "today"), v)
    C.Check.str("following month has no fabricated zero", UP.Up.total(st, 1794787200n, "month"), "")
    C.Check.str("old epoch never leaks", UP.Up.total(M.State.setting(st, "tok.epoch", "new"), 1792100002n, "month"), "")
    +page : String = text(UP.Up.rows(st, U.Ui.projects(st), 1792100002n, "month", "tokens", 0n, 0n))
    check("both providers represented", Bool.and(String.contains(page, "claude · m"), String.contains(page, "codex · m")))
    check("project represented", String.contains(page, "Board | 210 recorded tokens"))
    check("reasoning labeled included", String.contains(page, "7 recorded reasoning (included in output)"))
    check("no subscription bill claim", String.contains(page, "No subscription bill or estimated prices"))
    check("daily breakdown not monthly fiction", String.contains(text(UP.Up.rows(st, U.Ui.projects(st), 1792100002n, "today", "tokens", 0n, 0n)), "Select This calendar month"))
    check("limits unknown is not unlimited", String.contains(text(UP.Up.rows(st, Nil{}, 1792100002n, "month", "limits", 0n, 0n)), "Unknown, not unlimited"))
    C.Check.str("empty model index gives no model", Nat.show(List.length(&2, UZ.URow, UP.Up.models(M.State.empty(), 1792100002n, "tokens", 0n))), "0")
    +large : M.State = many(29n, st)
    C.Check.str("bounded first models", Nat.show(List.length(&2, UZ.URow, UP.Up.models(large, 1792100002n, "tokens", 0n))), "12")
    C.Check.str("bounded second models", Nat.show(List.length(&2, UZ.URow, UP.Up.models(large, 1792100002n, "tokens", 1n))), "12")
    C.Check.str("last model page", Nat.show(List.length(&2, UZ.URow, UP.Up.models(large, 1792100002n, "tokens", 2n))), "7")
    C.Check.str("beyond last model page", Nat.show(List.length(&2, UZ.URow, UP.Up.models(large, 1792100002n, "tokens", 3n))), "0")
    +u : U.Ui = U.Ui.init(1792100002n)
    C.Check.str("offline usage never says connected", U.Usage.page.connection(u), "Disconnected / cached; readings may be stale")
    C.Check.str("online flag is a connection label", U.Usage.page.connection(U.Ui.online(u, True{})), "Connected")
    +a : U.Act = U.Ui.act(u, "usage-period", "today")
    check("controls change local UI", String.eq(U.Usage.page.period(U.Act.ui(a)), "today"))
    C.Check.str("controls emit no request or persisted event", Nat.show(List.length(&2, U.Cmd, U.Act.cmds(a))), "0")
    +paged : U.Ui = U.Act.ui(U.Ui.act(u, "usage-model-page", "next"))
    C.Check.str("model page advances locally", Nat.show(U.Usage.page.index(paged, "model")), "1")
    C.Check.str("project page independent", Nat.show(U.Usage.page.index(paged, "project")), "0")
    C.Check.str("period change resets breakdown page", Nat.show(U.Usage.page.index(U.Act.ui(U.Ui.act(paged, "usage-period", "today")), "model")), "0")
    IO.print("ok usage page complete")
