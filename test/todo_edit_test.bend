import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/todo.bend as TD
import ../src/core/hub.bend as H
import ../src/core/client.bend as U
import ../src/core/codec.bend as CC
import ../src/core/json.bend as J
import ../src/core/far.bend as FR
import ../src/core/farsnap.bend as FS

# Exercise the event projection and the provider adapters, including the
# exact stale-list shape found in Bub's saved history.
def initial() -> M.State:
  M.State.replay([M.ThreadCreated{"t", "", "Bub", M.Local{}, "", "", "claude", "", 1n},
    M.ThreadCreated{"other", "", "Other bot", M.Local{}, "", "", "claude", "", 1n},
    M.TurnChanged{"t", M.Running{}, 2n}, M.TodosSet{"t", TD.Todo.show([TD.Todo{"1", "Read", "", "pending"}, TD.Todo{"2", "Build", "", "pending"}]), 3n}], M.State.empty())

def change(+s: M.State, +op: String, +id: String, +text: String, +status: String, +to: Nat) -> M.State:
  M.State.apply(s, M.TodosChanged{"t", "person", TD.Todo.ask("t", op, id, text, status, to), 4n})

def agent(+s: M.State) -> M.State:
  M.State.apply(s, M.TodosChanged{"t", "agent", TD.Todo.show([TD.Todo{"1", "Read", "", "pending"}, TD.Todo{"2", "Build", "", "pending"}]), 5n})

def texts(ts: List<&2, TD.Todo>) -> String:
  match ts:
    case Nil{}: ""
    case +h <> t: TD.Todo.text(h) ++ "/" ++ TD.Todo.status(h) ++ ";" ++ texts(t)

def step(s: M.State, st: H.Step) -> M.State:
  match st:
    case H.Step{cs, _}: M.State.replay(cs, s)

def body(+name: String, +input: J.Json) -> J.Json:
  J.Json.obj([J.KV{"type", J.Str{"tool_use"}}, J.KV{"id", J.Str{"create-use"}}, J.KV{"name", J.Str{name}}, J.KV{"input", input}])

def tools(+s: M.State, bs: List<&2, J.Json>) -> M.State:
  M.State.replay(H.Claude.todos(s, "t", 6n, bs), s)

def counts(st: H.Step) -> String:
  match st:
    case H.Step{cs, _}: Nat.show(List.length(&2, M.Change, cs))

def note.entries(es: List<&2, M.Entry>) -> Bool:
  match es:
    case Nil{}: False{}
    case M.Msg{_, _, _, +text, _} <> t: Bool.or(String.starts_with(text, "The person updated your to-do list:"), note.entries(t))
    case _ <> t: note.entries(t)

def request(+s: M.State, +th: String, +text: String) -> H.Step:
  H.Hub.request(s, 9n, 7, J.Json.obj([J.KV{"id", J.Num{"1"}}, J.KV{"m", J.Str{"todos.edit"}}, J.KV{"p", J.Json.obj([
    J.KV{"thread", J.Str{th}}, J.KV{"op", J.Str{"set"}}, J.KV{"id", J.Str{"1"}}, J.KV{"text", J.Str{text}}])}]))

def main() -> IO(Unit):
  do IO<Unit>:
    +edited : M.State = change(initial(), "set", "1", "Person text", "completed", 0n)
    C.Check.str("person text and checkbox survive repeated agent snapshot", texts(M.Todos.of(agent(edited), "t")), "Person text/completed;Build/pending;")
    C.Check.str("other bot has no list", texts(M.Todos.of(edited, "other")), "")
    C.Check.str("person deletion survives repeated agent snapshot", texts(M.Todos.of(agent(change(initial(), "delete", "1", "", "", 0n)), "t")), "Build/pending;")
    C.Check.str("person order survives repeated agent snapshot", texts(M.Todos.of(agent(change(initial(), "move", "2", "", "", 0n)), "t")), "Build/pending;Read/pending;")
    C.Check.str("person addition survives agent snapshot", texts(M.Todos.of(agent(change(initial(), "add", "h:mine", "Extra", "", 0n)), "t")), "Read/pending;Build/pending;Extra/pending;")
    +finished : M.State = M.State.apply(initial(), M.TurnChanged{"t", M.Completed{}, 6n})
    C.Check.str("unfinished list stays after its turn", Bool.show(M.Todos.visible(finished, "t")), "True")
    +next : M.State = M.State.apply(finished, M.TurnChanged{"t", M.Running{}, 7n})
    C.Check.str("next turn without writing hides previous list", Bool.show(M.Todos.visible(next, "t")), "False")
    C.Check.str("client hides that previous list too", Nat.show(List.length(&2, TD.Todo, U.Todos.shown(next, "t", True{}))), "0")
    +fresh : M.State = tools(next, [body("TaskCreate", J.Json.obj([J.KV{"subject", J.Str{"New task"}}]))])
    C.Check.str("new turn TaskCreate starts a new list", texts(M.Todos.of(fresh, "t")), "New task/pending;")
    +patched : M.State = tools(edited, [body("TaskUpdate", J.Json.obj([J.KV{"taskId", J.Str{"1"}}, J.KV{"status", J.Str{"pending"}}]))])
    C.Check.str("explicit TaskUpdate can restore an old status", texts(M.Todos.of(patched, "t")), "Person text/pending;Build/pending;")
    +created : M.State = change(fresh, "set", "u:create-use", "Person renamed", "completed", 0n)
    +named : M.State = M.State.apply(created, M.TodosChanged{"t", "tools", J.Json.show(J.Json.arr([J.Json.obj([J.KV{"name", J.Str{"TaskNamed"}}, J.KV{"use", J.Str{"create-use"}}, J.KV{"number", J.Str{"7"}}])])), 8n})
    C.Check.str("TaskCreate result preserves person edits", texts(M.Todos.of(named, "t")), "Person renamed/completed;")
    C.Check.str("TaskCreate result gives stable id", TD.Todo.id(Maybe.default(&2, TD.Todo, List.head(&2, TD.Todo, M.Todos.of(named, "t")), TD.Todo{"", "", "", ""})), "7")
    C.Check.str("completion reads done", TD.Todo.count(M.Todos.of(named, "t")), "done")
    +done : M.State = M.State.apply(named, M.TurnChanged{"t", M.Completed{}, 9n})
    C.Check.str("completed list is cleared when turn ends", texts(M.Todos.of(done, "t")), "")
    C.Check.str("completed list is not visible", Bool.show(M.Todos.visible(done, "t")), "False")
    +u : U.Ui = U.Ui.info_set(U.Ui.adopt.raw(U.Ui.init(1n), fresh), "fold:todos:t", "1")
    +closed : U.Ui = U.Ui.fold([M.TodosChanged{"t", "agent", TD.Todo.show([TD.Todo{"u:create-use", "New task", "", "completed"}]), 9n}], u)
    C.Check.str("done auto collapses even if open", Bool.show(U.Todos.open(U.Ui.infos(closed), "t")), "False")
    +snap : M.State = M.State.apply(M.State.empty(), M.TodosChanged{"t", "snapshot", M.Todos.snapshot(edited, "t"), 8n})
    C.Check.str("snapshot keeps person edit and agent baseline", texts(M.Todos.of(agent(snap), "t")), "Person text/completed;Build/pending;")
    +c : M.Change = M.TodosChanged{"t", "person", TD.Todo.ask("t", "set", "1", "Saved", "", 0n), 8n}
    C.Check.str("edit event round trips", J.Json.show(CC.Change.encode(Maybe.default(&2, M.Change, CC.Change.decode(CC.Change.encode(c)), M.TodosSet{"", "", 0n}))), J.Json.show(CC.Change.encode(c)))
    +idle : M.State = M.State.apply(initial(), M.TurnChanged{"t", M.Completed{}, 8n})
    +queued : M.State = step(idle, request(idle, "t", "Next turn"))
    C.Check.str("idle edit saves without starting agent", Bool.show(M.State.running.k(M.State.thread.get(queued, "t"))), "False")
    C.Check.str("idle edit queues a note for next turn", Bool.show(String.contains(M.Queue.of(queued, "t"), "The person updated your to-do list:")), "True")
    C.Check.str("edit note contains only the changed item", Bool.show(String.contains(M.Queue.of(queued, "t"), "Build")), "False")
    +running : M.State = step(initial(), request(initial(), "t", "Running edit"))
    C.Check.str("running edit delivers agent note", Bool.show(note.entries(M.State.mine(running, "t"))), "True")
    C.Check.str("missing thread cannot edit", counts(request(initial(), "missing", "Bad")), "0")
