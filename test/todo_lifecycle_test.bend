import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/todo.bend as TD
import ../src/core/i18n.bend as I
import ../src/web/dom.bend as D
import ../src/web/view.bend as V

# Synthetic events go through the real client fold and shared projection.
# No provider, user log, hub or window is started.
def plan(+status: String) -> String:
  TD.Todo.show([TD.Todo{"1", "Migrate Muse", "Migrating Muse", status}])

def mixed() -> String:
  TD.Todo.show([TD.Todo{"1", "Read", "", "completed"}, TD.Todo{"2", "Fix", "Fixing", "in_progress"}])

def base() -> U.Ui:
  U.Ui.fold([M.ProjectCreated{"p", "Synthetic", "/tmp/todo-lifecycle", 1n},
    M.ThreadCreated{"a", "p", "Muse migration", M.Local{}, "", "", "muse", "", 2n},
    M.ThreadCreated{"b", "p", "Other conversation", M.Local{}, "", "", "codex", "", 2n},
    M.MessagePosted{"a", "m1", M.User{}, "Migrate Muse", 3n},
    M.MessagePosted{"a", "m2", M.Assistant{}, "Migration complete", 4n}], U.Ui.init(100n))

def step(+u: U.Ui, +c: M.Change) -> U.Ui:
  U.Ui.fold([c], u)

def running() -> U.Ui:
  step(base(), M.TurnChanged{"a", M.Running{}, 5n})

def current() -> U.Ui:
  step(running(), M.TodosSet{"a", plan("completed"), 6n})

def ended() -> U.Ui:
  step(current(), M.TurnChanged{"a", M.Completed{}, 7n})

def next() -> U.Ui:
  step(ended(), M.TurnChanged{"a", M.Running{}, 8n})

def shown(+u: U.Ui, +th: String) -> List<&2, TD.Todo>:
  U.Todos.shown(U.Ui.state(u), th, M.State.running.k(M.State.thread.get(U.Ui.state(u), th)))

def count(+u: U.Ui, +th: String) -> String:
  Nat.show(List.length(&2, TD.Todo, shown(u, th)))

def ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}: u

# Read the actual web card's DOM text, including its head and step rows.
def text(ns: List<&2, D.Node>) -> String:
  match ns:
    case Nil{}: ""
    case D.Txt{s} <> t: s ++ text(t)
    case D.El{_, _, _, ks} <> t: text(ks) ++ text(t)

def card(+u: U.Ui, +th: String) -> String:
  text([V.View.todos(I.En{}, U.Ui.infos(u), th, shown(u, th))])

def legacy(+todos: String) -> U.Ui:
  step(running(), M.SettingSet{"todos.a", todos, 6n})

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("completed current plan stays during its running turn", count(current(), "a"), "1")
    C.Check.str("completed plan hides at turn end", count(ended(), "a"), "0")
    C.Check.str("completed plan stays hidden while idle", count(step(ended(), M.TurnChanged{"a", M.Idle{}, 8n}), "a"), "0")
    C.Check.str("unrelated new turn cannot revive completed plan", count(next(), "a"), "0")
    C.Check.str("unchanged provider snapshot cannot reclaim completed plan", count(step(next(), M.TodosSet{"a", plan("completed"), 9n}), "a"), "0")
    C.Check.str("equivalent serialized snapshot cannot reclaim plan", count(step(next(), M.TodosSet{"a", "[ {\"status\":\"completed\", \"form\":\"Migrating Muse\", \"text\":\"Migrate Muse\", \"id\":\"1\"} ]", 9n}), "a"), "0")
    C.Check.str("repeated Running for steering preserves current plan", count(step(current(), M.TurnChanged{"a", M.Running{}, 8n}), "a"), "1")
    C.Check.str("repeated snapshot during owner turn preserves plan", count(step(current(), M.TodosSet{"a", plan("completed"), 8n}), "a"), "1")
    C.Check.str("interruption hides completed plan", count(step(current(), M.TurnChanged{"a", M.Interrupted{}, 7n}), "a"), "0")
    C.Check.str("failure hides completed plan", count(step(current(), M.TurnChanged{"a", M.Failed{}, 7n}), "a"), "0")
    +pending : U.Ui = step(running(), M.TodosSet{"a", plan("pending"), 6n})
    +mix : U.Ui = step(running(), M.TodosSet{"a", mixed(), 6n})
    C.Check.str("pending survives turn end", count(step(pending, M.TurnChanged{"a", M.Completed{}, 7n}), "a"), "1")
    C.Check.str("mixed unfinished plan survives end", count(step(mix, M.TurnChanged{"a", M.Completed{}, 7n}), "a"), "2")
    +interrupted : U.Ui = step(mix, M.TurnChanged{"a", M.Interrupted{}, 7n})
    C.Check.str("interrupted in-progress plan remains", count(interrupted, "a"), "2")
    +continued : U.Ui = step(interrupted, M.TurnChanged{"a", M.Running{}, 8n})
    C.Check.str("unfinished continuation remains", count(continued, "a"), "2")
    C.Check.str("unchanged unfinished snapshot remains", count(step(continued, M.TodosSet{"a", mixed(), 9n}), "a"), "2")
    +finished : U.Ui = step(step(pending, M.TurnChanged{"a", M.Completed{}, 7n}), M.TurnChanged{"a", M.Running{}, 8n})
    C.Check.str("continuation completing its plan claims current ownership", count(step(finished, M.TodosSet{"a", plan("completed"), 9n}), "a"), "1")
    C.Check.str("explicit empty replacement removes unfinished plan", count(step(mix, M.TodosSet{"a", "[]", 9n}), "a"), "0")
    C.Check.str("new pending replacement shows", count(step(next(), M.TodosSet{"a", plan("pending"), 9n}), "a"), "1")
    +replacement : String = TD.Todo.show([TD.Todo{"1", "New work", "", "completed"}])
    C.Check.str("new completed replacement belongs to current turn", count(step(next(), M.TodosSet{"a", replacement, 9n}), "a"), "1")
    C.Check.str("idle replacement does not own next turn", count(step(step(ended(), M.TodosSet{"a", replacement, 8n}), M.TurnChanged{"a", M.Running{}, 9n}), "a"), "0")
    C.Check.str("missing ownership on legacy completed cache hides it", count(legacy(plan("completed")), "a"), "0")
    C.Check.str("legacy pending cache remains", count(legacy(plan("pending")), "a"), "1")
    C.Check.str("unchanged legacy snapshot cannot acquire ownership", count(step(legacy(plan("completed")), M.TodosSet{"a", plan("completed"), 9n}), "a"), "0")
    +other : U.Ui = step(step(next(), M.TurnChanged{"b", M.Running{}, 9n}), M.TodosSet{"b", plan("completed"), 10n})
    C.Check.str("fresh plan on another thread shows", count(other, "b"), "1")
    +selected : U.Ui = ui(U.Ui.select(ui(U.Ui.select(other, "b")), "a"))
    C.Check.str("switching conversations cannot revive old plan", count(selected, "a"), "0")
    C.Check.str("switching preserves other current plan", count(selected, "b"), "1")
    C.Check.str("original stored plan preserved", M.Setting.get(U.Ui.state(next()), "todos.a", ""), plan("completed"))
    C.Check.str("historical messages preserved", Nat.show(List.length(&2, M.Entry, M.State.mine(U.Ui.state(next()), "a"))), "2")
    C.Check.str("DOM shows current completed plan", card(current(), "a"), "Todo 1/1[x] Migrate Muse")
    C.Check.str("DOM has no stale completed head or rows", card(next(), "a"), "")
    C.Check.str("DOM retains mixed progress", card(interrupted, "a"), "Todo 1/2[x] Read[>] Fixing")
