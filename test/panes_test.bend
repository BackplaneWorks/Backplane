import Base
import ../src/core/i18n.bend as I
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/todo.bend as TD
import ../src/app/nui.bend as N
import ../src/app/layout.bend as L
import ../src/core/subs.bend as SB
import ../src/core/model.bend as M

# Long threads' panes: the todo list starts shut, opens and shuts with its
# head and shows every step on "Show all" (client.bend's Todos), and the
# window's scrolling
# panes (the subagents panel, the todo card) move by a row a wheel notch
# within their rows (nui.bend's Pane, layout.bend's Pane).

def todos.go(n: Nat, +i: Nat, acc: List<&2, TD.Todo>) -> List<&2, TD.Todo>:
  match n:
    case 0n:
      acc
    case 1n+k:
      todos.go(k, 1n+i, TD.Todo{Nat.show(i), "step " ++ Nat.show(i), "", Bool.pick(String, Nat.is_lt(i, 3n), "completed", "pending")} <> acc)

# n steps, the first three done
def todos(+n: Nat) -> List<&2, TD.Todo>:
  List.reverse(&2, TD.Todo, todos.go(n, 0n, Nil{}))

def ui.of(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def fold(+u: U.Ui, +v: String) -> U.Ui:
  ui.of(U.Ui.act(u, "fold", v))

# the person opens t1's list (its head, shut)
def opened(+u: U.Ui) -> U.Ui:
  fold(u, U.Todos.toggle(False{}, "t1"))

def hit.value(m: Maybe<&2, N.Hit>) -> String:
  match m:
    case Some{N.Hit{_, _, _, _, _, value}}:
      value
    case None{}:
      ""

def nui.of(s: N.Step) -> N.Nui:
  match s:
    case N.Step{n, _, _}:
      n

def bool(b: Bool) -> String:
  Bool.pick(String, b, "true", "false")

def rows(+u: U.Ui, +ts: List<&2, TD.Todo>) -> String:
  Nat.show(List.length(&2, TD.TRow, U.Todos.rows(U.Ui.infos(u), "t1", U.Todos.cap.all(), ts)))

# a notch down over a pane, then where it is
def notch(+n: N.Nui, +last: Nat, +dir: U32) -> N.Nui:
  nui.of(N.Pane.step(n, "subs:t1", last, dir))

def at(+n: N.Nui) -> String:
  Nat.show(L.Pane.off(U.Ui.infos(N.Nui.ui(n)), "subs:t1"))

# a pane's hits as the frame keeps them: topmost first (layout.bend's
# final reverse), so N.Hit.find answers what a click there does
def hits(o: L.Out) -> List<&2, N.Hit>:
  match o:
    case L.Out{_, hs}:
      List.reverse(&2, N.Hit, hs)

def hit.action(m: Maybe<&2, N.Hit>) -> String:
  match m:
    case Some{N.Hit{_, _, _, _, act, _}}:
      act
    case None{}:
      ""

def clicked(+u: U.Ui, h: Maybe<&2, N.Hit>) -> U.Ui:
  match h:
    case Some{N.Hit{_, _, _, _, act, value}}:
      ui.of(U.Ui.act(u, act, value))
    case None{}:
      u

def sub.row() -> SB.SubRow:
  SB.SubRow{"c", True{}, "Inspect power", "subagent", "running", "", True{}, False{}, 0n, Nil{}}

# the subagents panel at x 100..900, its head at y 100..130
def subs.out(+open: Bool) -> L.Out:
  L.Lay.subs(L.Ty.none(), [sub.row()], SB.Subs.panel.rows(open, [sub.row()]), open, "t1", 0n, 100, 900, 100)

def main() -> IO(Unit):
  do IO<Unit>:
    # hit order: a pane's whole-box "pane" hit (the wheel) sits under
    # every hit drawn on it, so a click on the head or a row is theirs
    C.Check.str("subagents head beats its scrolling pane", hit.action(N.Hit.find(hits(subs.out(True{})), 200, 110)), "subs-shut")
    C.Check.str("a shut subagents head opens it", hit.action(N.Hit.find(hits(subs.out(False{})), 200, 110)), "subs-open")
    +shut : U.Ui = clicked(U.Ui.init(0n), N.Hit.find(hits(subs.out(True{})), 200, 110))
    C.Check.str("a head click shuts the panel", bool(SB.Subs.panel.open(U.Ui.infos(shut), "t1", [sub.row()])), "false")
    +back : U.Ui = clicked(shut, N.Hit.find(hits(subs.out(False{})), 200, 110))
    C.Check.str("a second head click opens it", bool(SB.Subs.panel.open(U.Ui.infos(back), "t1", [sub.row()])), "true")
    C.Check.str("a subagent row stays clickable", hit.action(N.Hit.find(hits(subs.out(True{})), 200, 140)), "fold")
    C.Check.str("the wheel still finds the pane under the head", Bool.show(String.starts_with(N.Hit.value_at(hits(subs.out(True{})), "pane", 200, 110), "subs:t1|")), "True")
    +task : SB.SubRow = SB.SubRow{"kid", False{}, "Inspect board", "opus", "running", "", True{}, False{}, 0n, Nil{}}
    C.Check.str("Stop beats its row", hit.action(N.Hit.find(hits(L.Sa.row(L.Ty.none(), task, 100, 900, 100)), (L.Sa.stop.x(L.Ty.none(), "running", "task-cancel", 900) + 2 : U32), 110)), "task-cancel")
    C.Check.str("the row beside Stop selects", hit.action(N.Hit.find(hits(L.Sa.row(L.Ty.none(), task, 100, 900, 100)), 200, 110)), "select")
    +review : U.RpView = U.RpView{"erc", Nil{}, "ERC", "[Board](/tmp/board.kicad_pcb)", "", False{}}
    C.Check.str("a review link beats its scrolling pane", hit.action(N.Hit.find(hits(L.Rpw.body(True{}, L.Ty.none(), review, "t1", 0n, 100, 900, 500)), 125, 150)), "file-open")
    # the window's todo list at the top (its head at y 500..530), shut
    +todo_out : L.Out = L.Lay.todos.k(True{}, L.Ty.none(), todos(4n), Nil{}, "", False{}, False{}, "t1", 0n, 4n, 100, 900, 500)
    C.Check.str("the todo head beats its scrolling pane", hit.action(N.Hit.find(hits(todo_out), 200, 510)), "fold")
    C.Check.str("a shut todo head opens it", hit.value(N.Hit.find(hits(todo_out), 200, 510)), "1todos:t1")
    C.Check.str("shut, the window's list is one line", U32.show(L.Lay.tdbox(Nil{}, "")), "32")
    C.Check.str("and takes that and its gap from the top", U32.show(L.Lay.tdh(todos(4n), Nil{}, "")), "44")
    C.Check.str("no list takes nothing", U32.show(L.Lay.tdh(Nil{}, Nil{}, "")), "0")
    +ask : M.Ask = M.Ask{"a", "t1", "approval", "Bash", "Run ls", "", True{}, ""}
    +ask_out : L.Out = L.Ask.card(L.Ty.none(), ask, "", Map.new(&2, String), 100, 900, 100)
    C.Check.str("an approval's button beats its scrolling pane", hit.action(N.Hit.find(hits(ask_out), 880, (100 + L.Ask.h(L.Ty.none(), ask, Map.new(&2, String), 768) - 20 : U32))), "answer")
    +u : U.Ui = U.Ui.init(0n)
    +many : List<&2, TD.Todo> = todos(200n)
    C.Check.str("a short list has no last line", U.Todos.more(I.En{}, False{}, todos(4n)), "")
    C.Check.str("a long one offers every step", U.Todos.more(I.En{}, False{}, many), "Show all 200")
    C.Check.str("shown all, it offers fewer", U.Todos.more(I.En{}, True{}, many), "Show fewer")
    C.Check.str("the list starts shut", bool(U.Todos.open(U.Ui.infos(u), "t1")), "false")
    C.Check.str("and shows no steps", rows(u, many), "0")
    C.Check.str("its head opens it", bool(U.Todos.open(U.Ui.infos(opened(u)), "t1")), "true")
    C.Check.str("open, it shows the fitted few", rows(opened(u), many), "8")
    C.Check.str("its head shuts it again", bool(U.Todos.open(U.Ui.infos(fold(opened(u), U.Todos.toggle(True{}, "t1"))), "t1")), "false")
    C.Check.str("shut, it shows no steps", rows(fold(opened(u), U.Todos.toggle(True{}, "t1")), many), "0")
    C.Check.str("and opens again", bool(U.Todos.open(U.Ui.infos(fold(fold(opened(u), U.Todos.toggle(True{}, "t1")), U.Todos.toggle(False{}, "t1"))), "t1")), "true")
    C.Check.str("show all shows every step", rows(fold(opened(u), U.Todos.more.ask(False{}, "t1")), many), "200")
    C.Check.str("show fewer goes back", rows(fold(fold(opened(u), U.Todos.more.ask(False{}, "t1")), U.Todos.more.ask(True{}, "t1")), many), "8")
    C.Check.str("show all never opens a shut list", rows(fold(u, U.Todos.more.ask(False{}, "t1")), many), "0")
    C.Check.str("another thread's list is its own", bool(U.Todos.open(U.Ui.infos(fold(u, U.Todos.toggle(False{}, "t2"))), "t1")), "false")
    C.Check.str("and stays open when another opens", bool(U.Todos.open(U.Ui.infos(fold(opened(u), U.Todos.toggle(False{}, "t2"))), "t1")), "true")
    C.Check.str("a pane's hit value: key", N.Pane.at("subs:t1|7", 0n), "subs:t1")
    C.Check.str("a pane's hit value: its last row", N.Pane.at("subs:t1|7", 1n), "7")
    +n : N.Nui = N.Nui.init(u, 1400, 860)
    C.Check.str("a pane starts at its first row", at(n), "0")
    C.Check.str("a notch down moves it a row", at(notch(n, 7n, 1)), "1")
    C.Check.str("three notches, three rows", at(notch(notch(notch(n, 7n, 1), 7n, 1), 7n, 1)), "3")
    C.Check.str("a notch up comes back", at(notch(notch(notch(n, 7n, 1), 7n, 1), 7n, 0)), "1")
    C.Check.str("up at the top stays there", at(notch(n, 7n, 0)), "0")
    C.Check.str("it stops at the last row", at(notch(notch(notch(n, 2n, 1), 2n, 1), 2n, 1)), "2")
    C.Check.str("an offset past the rows is clamped", Nat.show(L.Pane.clamp(9n, 4n)), "3")
    C.Check.str("an offset within them stays", Nat.show(L.Pane.clamp(2n, 4n)), "2")
    C.Check.str("the head says which rows show", L.Pane.range(3n, 4n, 12n), " · 4–7 of 12")
    C.Check.str("and nothing when all do", L.Pane.range(0n, 12n, 12n), "")
