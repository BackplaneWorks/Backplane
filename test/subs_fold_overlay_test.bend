import Base
import ./check.bend as C
import ../src/core/subs.bend as SB
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/i18n.bend as I
import ../src/core/json.bend as J
import ../src/web/presentation.bend as P
import ../src/web/view.bend as V
import ../src/web/dom.bend as D

# The Subagents fold shows at the click (subs-feedback.js): the hub draws
# the rows shut too, hidden, and the page's overlay turns the head and the
# section before the hub's view comes back.

def row(+key: String, +word: String, live: Bool) -> SB.SubRow:
  SB.SubRow{key, True{}, key, "claude", word, "", live, False{}, 0n, Nil{}}

def rows() -> List<&2, SB.SubRow>:
  [row("a1", "running", True{}), row("a2", "done", False{})]

def shut() -> Map<&2, String>:
  Map.set(&2, String, Map.new(&2, String), "subs:t1", "0live:a1")

def page(+info: Map<&2, String>) -> D.Node:
  D.El{"div", "", Nil{}, List.append(&2, D.Node, V.View.chrome.subs(I.En{}, M.State.empty(), info, "t1", rows()), V.View.subs.dock(I.En{}, M.State.empty(), info, "t1", rows()))}

def show(n: D.Node) -> String:
  J.Json.show(P.Node.write(n))

def yes(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

def has(+n: D.Node, +s: String) -> String:
  yes(String.contains(show(n), s))

def main() -> IO(Unit):
  do IO<Unit>:
    +closed : D.Node = page(shut())
    +opened : D.Node = page(Map.new(&2, String))
    C.Check.str("a shut dock is drawn, hidden", has(closed, "\"subs dock shut\""), "yes")
    C.Check.str("a shut dock still has its rows", has(closed, "\"a2\""), "yes")
    C.Check.str("an open dock is drawn shown", has(opened, "\"subs dock\""), "yes")
    C.Check.str("open, the head's value keeps the live keys too", has(opened, "t1\\tlive:a1"), "yes")
    C.Check.str("shut, the head's value keeps the live keys", has(closed, "t1\\tlive:a1"), "yes")
    # the overlay: "1" open, "0" shut, "" as drawn
    +shown : D.Node = P.Node.overlay.subs(closed, "", False{}, "1")
    C.Check.str("opened at the click, the dock shows", has(shown, "\"subs dock\""), "yes")
    C.Check.str("opened at the click, no section stays shut", has(shown, " shut\""), "no")
    C.Check.str("opened at the click, the head says so", has(shown, "\"aria-expanded\",\"value\":\"true\""), "yes")
    C.Check.str("opened at the click, the head's next click shuts", has(shown, "\"action\":\"subs-shut\""), "yes")
    +hidden : D.Node = P.Node.overlay.subs(opened, "", False{}, "0")
    C.Check.str("shut at the click, the dock hides", has(hidden, "\"subs dock shut\""), "yes")
    C.Check.str("shut at the click, the head's next click opens", has(hidden, "\"action\":\"subs-open\""), "yes")
    C.Check.str("no click pending: the hub's picture as drawn", yes(String.eq(show(P.Node.overlay.subs(closed, "", False{}, "")), show(closed))), "yes")
    C.Check.str("shutting twice adds no second shut", has(P.Node.overlay.subs(closed, "", False{}, "0"), "shut shut"), "no")
    # the hub takes the head's value either way
    +u : U.Ui = U.Act.ui(U.Ui.act(U.Ui.init(110n), "subs-open", "t1\tlive:a1"))
    C.Check.str("an open with the shut's value opens that thread", U.Ui.info(u, "subs:t1"), "1")
    C.Check.str("an open with the thread alone still opens it", U.Ui.info(U.Act.ui(U.Ui.act(U.Ui.init(110n), "subs-open", "t1")), "subs:t1"), "1")
