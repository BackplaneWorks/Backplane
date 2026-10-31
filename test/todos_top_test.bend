import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/todo.bend as TD
import ../src/core/i18n.bend as I
import ../src/core/json.bend as J
import ../src/web/view.bend as V
import ../src/web/dom.bend as D
import ../src/mobile/screen.bend as MS

# The agent's to-do list sits at the top of the thread beside its
# subagents, one line until the person opens it (client.bend's Todos,
# view.bend's View.chrome.todos and View.todos, screen.bend's Mob.todos).
# On the web's top bar it is the cell right after Subagents, its steps a
# dock under the bar only while open; without the bar a section right
# after the subagents'. Nothing of it is left above the composer, and a
# turn, a new list or a thread switch never opens it.

def steps(+done: Nat) -> String:
  TD.Todo.show([TD.Todo{"1", "Read the board", "", "completed"}, TD.Todo{"2", "Place parts", "", "completed"},
    TD.Todo{"3", "Route power", "", Bool.pick(String, Nat.is_gt(done, 2n), "completed", "pending")},
    TD.Todo{"4", "Route signals", "Routing signals", "in_progress"}, TD.Todo{"5", "Pour ground", "", "pending"},
    TD.Todo{"6", "Run DRC", "", "pending"}, TD.Todo{"7", "Export", "", "pending"}])

def state() -> M.State:
  M.State.replay([
    M.ThreadCreated{"t1", "", "Main", M.Local{}, "", "", "claude", "", 1n},
    M.ThreadCreated{"t2", "", "Other", M.Local{}, "", "", "claude", "", 1n},
    M.ThreadCreated{"kid", "", "Kid", M.Local{}, "", "", "claude", "", 2n},
    M.ThreadLaunched{"kid", "t1", False{}, 3n},
    M.TodosSet{"t1", steps(3n), 4n}], M.State.empty())

def ui() -> U.Ui:
  U.Act.ui(U.Ui.select(U.Ui.adopt.raw(U.Ui.init(10n), state()), "t1"))

# the web's top bar on (the rail and its bar, as the web app runs)
def top(+u: U.Ui) -> U.Ui:
  U.Ui.info_set(U.Ui.info_set(u, "local.workspace.layout", "rail-v1"), "local.workspace.chrome", "top-v1")

def click(+u: U.Ui, +v: String) -> U.Ui:
  U.Act.ui(U.Ui.act(u, "fold", v))

def select(+u: U.Ui, +id: String) -> U.Ui:
  U.Act.ui(U.Ui.select(u, id))

def ts(+u: U.Ui) -> List<&2, TD.Todo>:
  U.Todos.shown(U.Ui.state(u), "t1", False{})

def bool(b: Bool) -> String:
  Bool.pick(String, b, "true", "false")

def opened(+u: U.Ui) -> String:
  bool(U.Todos.open(U.Ui.infos(u), U.Ui.state(u), "t1"))

# Reading a page
# --------------

def attr.of(as: List<&2, D.Attr>, +k: String) -> String:
  match as:
    case Nil{}:
      ""
    case D.At{+n, +v} <> t:
      Bool.pick(String, String.eq(n, k), v, attr.of(t, k))
    case D.On{_, _} <> t:
      attr.of(t, k)

# every element's class, in document order
def walk(fuel: Nat, ns: List<&2, D.Node>, out: List<&2, String>) -> List<&2, String>:
  match fuel ns:
    case 0n _:
      out
    case 1n+k Nil{}:
      out
    case 1n+k D.El{_, _, +as, kids} <> t:
      walk(k, List.append(&2, D.Node, kids, t), attr.of(as, "class") <> out)
    case 1n+k D.Txt{_} <> t:
      walk(k, t, out)

def classes(ns: List<&2, D.Node>) -> List<&2, String>:
  List.reverse(&2, String, walk(200000n, ns, Nil{}))

# its words, in document order
def words.go(fuel: Nat, ns: List<&2, D.Node>, +out: String) -> String:
  match fuel ns:
    case 0n _:
      out
    case 1n+k Nil{}:
      out
    case 1n+k D.El{_, _, _, kids} <> t:
      words.go(k, List.append(&2, D.Node, kids, t), out)
    case 1n+k D.Txt{+x} <> t:
      words.go(k, t, Bool.pick(String, String.is_empty(out), x, out ++ " " ++ x))

def words(ns: List<&2, D.Node>) -> String:
  words.go(10000n, ns, "")

# an element's attribute, the first with a class starting with p
def attr.at(fuel: Nat, ns: List<&2, D.Node>, +p: String, +k: String) -> String:
  match fuel ns:
    case 0n _:
      ""
    case 1n+j Nil{}:
      ""
    case 1n+j D.El{_, _, +as, kids} <> t:
      Bool.pick(String, String.starts_with(attr.of(as, "class"), p), attr.of(as, k), attr.at(j, List.append(&2, D.Node, kids, t), p, k))
    case 1n+j D.Txt{_} <> t:
      attr.at(j, t, p, k)

# where the first class starting with p is (999999: none)
def pos.go(xs: List<&2, String>, +p: String, +i: Nat) -> Nat:
  match xs:
    case Nil{}:
      999999n
    case +x <> t:
      Bool.pick(Nat, String.starts_with(x, p), i, pos.go(t, p, 1n+i))

def pos(+xs: List<&2, String>, +p: String) -> Nat:
  pos.go(xs, p, 0n)

# how many classes starting with p come after the element of class mark
def after.go(xs: List<&2, String>, +p: String, +mark: String, +seen: Bool, +n: Nat) -> Nat:
  match xs:
    case Nil{}:
      n
    case +x <> t:
      after.go(t, p, mark, Bool.or(seen, String.eq(x, mark)), Bool.pick(Nat, Bool.and(seen, String.starts_with(x, p)), 1n+n, n))

# (mark "": all of them)
def after(+xs: List<&2, String>, +p: String, +mark: String) -> String:
  Nat.show(after.go(xs, p, mark, String.is_empty(mark), 0n))

def node.cls(n: D.Node) -> String:
  match n:
    case D.El{_, _, as, _}:
      attr.of(as, "class")
    case D.Txt{_}:
      ""

def only.k(hit: Bool, n: D.Node, rest: List<&2, D.Node>) -> List<&2, D.Node>:
  match hit:
    case True{}:
      n <> rest
    case False{}:
      rest

# the elements among ns whose class starts with p
def only(ns: List<&2, D.Node>, +p: String) -> List<&2, D.Node>:
  match ns:
    case Nil{}:
      Nil{}
    case +n <> t:
      only.k(String.starts_with(node.cls(n), p), n, only(t, p))

def page(+u: U.Ui) -> List<&2, String>:
  classes([V.View.thread(I.En{}, u, U.Ui.thread(U.Ui.state(u), "t1"))])

def cells(+u: U.Ui) -> List<&2, D.Node>:
  V.View.chrome.cells(False{}, I.En{}, U.Ui.state(u), U.Ui.infos(u), "t1", M.Idle{}, 0n, "", 10n)

# each cell's class, in order
def tops(ns: List<&2, D.Node>) -> List<&2, String>:
  match ns:
    case Nil{}:
      Nil{}
    case n <> t:
      node.cls(n) <> tops(t)

# the cell right after the subagents' (its class)
def next.k(xs: List<&2, String>, +found: Bool) -> String:
  match xs found:
    case Nil{} _:
      ""
    case +x <> _ True{}:
      x
    case +x <> t False{}:
      next.k(t, String.starts_with(x, "kv subs-head"))

def mob(+u: U.Ui) -> J.Json:
  MS.Mob.todos(I.En{}, U.Ui.infos(u), U.Ui.state(u), "t1", ts(u))

# a turn starts, a new list comes, the turn ends, the person goes to
# another thread and back
def lived(+u: U.Ui) -> U.Ui:
  select(select(U.Ui.fold([M.TurnChanged{"t1", M.Running{}, 5n}, M.TodosSet{"t1", steps(2n), 6n}, M.TurnChanged{"t1", M.Idle{}, 7n}], u), "t2"), "t1")

def main() -> IO(Unit):
  do IO<Unit>:
    +u : U.Ui = ui()
    +t : U.Ui = top(ui())
    C.Check.str("the thread has a list", Nat.show(List.length(&2, TD.Todo, ts(u))), "7")
    # the top bar: right after Subagents, one line, shut
    C.Check.str("the bar has the subagents' cell", Bool.show(Nat.is_lt(pos(tops(cells(t)), "kv subs-head"), 999999n)), "True")
    C.Check.str("the to-do cell comes right after it", next.k(tops(cells(t)), False{}), "kv todos-head")
    C.Check.str("it reads To-do and the count", words(only(cells(t), "kv todos")), "To-do 3/7")
    C.Check.str("it starts shut", attr.at(1000n, cells(t), "kv todos-head", "aria-expanded"), "false")
    C.Check.str("a click asks to open it", attr.at(1000n, cells(t), "kv todos-head", "data-value"), "1todos:t1")
    # under the bar: nothing while shut, the steps once opened, never by the composer
    C.Check.str("shut, the page shows no steps", after(page(t), "todos", ""), "0")
    C.Check.str("opened, its steps sit under the bar", Bool.show(Nat.is_lt(pos(page(click(t, "1todos:t1")), "todos dock"), pos(page(click(t, "1todos:t1")), "timeline"))), "True")
    C.Check.str("and the cell says it is open", attr.at(1000n, cells(click(t, "1todos:t1")), "kv todos-head", "aria-expanded"), "true")
    C.Check.str("nothing of it after the timeline (by the composer)", after(page(click(t, "1todos:t1")), "todo", "timeline"), "0")
    C.Check.str("the composer is still there", Bool.show(Nat.is_lt(pos(page(click(t, "1todos:t1")), "composer"), 999999n)), "True")
    C.Check.str("a second click shuts it", after(page(click(click(t, "1todos:t1"), "0todos:t1")), "todos", ""), "0")
    # without the bar: a section right after the subagents', one line, shut
    C.Check.str("the section follows the subagents'", Bool.show(Bool.and(Nat.is_lt(pos(page(u), "subs"), pos(page(u), "todos shut")), Nat.is_lt(pos(page(u), "todos shut"), pos(page(u), "timeline")))), "True")
    C.Check.str("its head reads To-do and the count", words([V.View.todos(I.En{}, U.Ui.infos(u), U.Ui.state(u), "t1", ts(u))]), "To-do 3/7")
    C.Check.str("shut, it shows no steps", after(page(u), "todo ", ""), "0")
    C.Check.str("nothing of it by the composer", after(page(click(u, "1todos:t1")), "todo", "timeline"), "0")
    C.Check.str("opened, its steps show", after(page(click(u, "1todos:t1")), "todo ", ""), "7")
    # nothing else opens it
    C.Check.str("a turn, a new list and a thread switch leave it shut", opened(lived(u)), "false")
    C.Check.str("and the cell too", attr.at(1000n, cells(top(lived(u))), "kv todos-head", "aria-expanded"), "false")
    C.Check.str("the person's open stays through them", opened(lived(click(u, "1todos:t1"))), "true")
    C.Check.str("another thread's click leaves it shut", opened(click(u, "1todos:t2")), "false")
    # the phones: the count for the bar's button, shut, no lines
    C.Check.str("phones: the count", J.Json.get_str(mob(u), "count", ""), "3/7")
    C.Check.str("phones: shut", bool(J.Json.get_bool(mob(u), "open", True{})), "false")
    C.Check.str("phones: no lines shut", Nat.show(List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(mob(u), "lines")))), "0")
    C.Check.str("phones: a tap asks to open", J.Json.get_str(mob(u), "toggle", ""), "1todos:t1")
    C.Check.str("phones: opened, it shows its lines", Nat.show(List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(mob(click(u, "1todos:t1")), "lines")))), "7")
    C.Check.str("phones: and a tap shuts it", J.Json.get_str(mob(click(u, "1todos:t1")), "toggle", ""), "0todos:t1")
    C.Check.str("phones: a turn and a switch leave it shut", bool(J.Json.get_bool(mob(lived(u)), "open", True{})), "false")
