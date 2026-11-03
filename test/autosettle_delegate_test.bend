import Base
import ./check.bend as C
import ../src/core/autosettle.bend as A
import ../src/core/model.bend as M
import ../src/core/hub.bend as B
import ../src/core/mcp.bend as MC
import ../src/core/json.bend as J

def root() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "Project", "/tmp/autosettle", 1n},
    M.ThreadCreated{"parent", "p", "Parent", M.Local{}, "", "", "codex", "", 2n}], M.State.empty())

def spawn(+s: M.State) -> MC.Res:
  MC.Mcp.delegate.go(s, 100n, "parent", "p", M.Local{}, "", "", "codex", "", M.AutoMode{}, M.DefaultMode{}, "Update the intent file", "Worker")

def child(r: MC.Res) -> String:
  match r:
    case MC.Res{_, result, _}: J.Json.get_str(result, "childThreadId", "")

def task(r: MC.Res) -> String:
  match r:
    case MC.Res{_, result, _}: J.Json.get_str(result, "taskId", "")

def created(r: MC.Res, +s: M.State, metadata: Bool) -> M.State:
  match r metadata:
    case MC.Res{_, _, +step} True{}: M.State.replay(B.Shelve.auto.fix(s, B.Queue.changes(step)), s)
    case MC.Res{_, _, step} False{}: M.State.replay(B.Queue.changes(step), s)

def finish(+s: M.State, +id: String, +task: String, delivered: Bool) -> M.State:
  M.State.replay(List.append(&2, M.Change,
    [M.MessagePosted{id, "answer", M.Assistant{}, "Done", 300n}, M.TurnChanged{id, M.Completed{}, 300n},
     M.TaskStateSet{task, M.TaskDone{}, "Done", 300n}],
    Bool.pick(List<&2, M.Change>, delivered, [M.TaskDelivered{task, 301n}], Nil{})), s)

def due(+s: M.State, +id: String) -> String:
  Bool.show(A.Auto.due(s, M.State.thread.get(s, id), 1500n, A.Auto.index(s)))

def main() -> IO(Unit):
  +r : MC.Res = spawn(root())
  +id : String = child(r)
  +tk : String = task(r)
  +legacy : M.State = finish(created(r, root(), False{}), id, tk, True{})
  +fresh : M.State = finish(created(r, root(), True{}), id, tk, True{})
  +human : List<&2, M.Change> = [M.MessagePosted{id, "real-person-same-second", M.User{}, "More work", 100n}]
  +joined : M.State = M.State.replay(human, legacy)
  do IO<Unit>:
    C.Check.str("real delegation producer yields expected child", id, "t100-3")
    C.Check.str("legacy plain delegation prompt settles once delivered", due(legacy, id), "True")
    C.Check.str("undelivered delegated task stays visible", due(finish(created(r, root(), False{}), id, tk, False{}), id), "False")
    C.Check.str("fresh delegation records exact seed", M.Setting.get(fresh, "autosettle.seed." ++ id, ""), "m100-4")
    C.Check.str("fresh delegation audience is worker", M.Setting.get(fresh, "autosettle.audience." ++ id, ""), "worker")
    C.Check.str("fresh plain delegation prompt settles once delivered", due(fresh, id), "True")
    C.Check.str("same-second human message is not the delegation seed", due(joined, id), "False")
    C.Check.str("later linked note does not erase human unread protection", due(M.State.replay([M.MessagePosted{id, M.Link.id("parent", "later"), M.User{}, "Another worker note", 302n}], joined), id), "False")
    +sticky : M.State = M.State.replay(B.Shelve.auto.fix(fresh, human), fresh)
    C.Check.str("fresh human follow-up sets durable personal audience", M.Setting.get(sticky, "autosettle.audience." ++ id, ""), "person")
    C.Check.str("human follow-up remains unread", due(sticky, id), "False")
    C.Check.str("reading real human conversation allows normal rule", due(M.State.replay(A.Auto.read(sticky, id, 400n), sticky), id), "True")
    +personal : M.State = M.State.replay([M.ProjectCreated{"p", "Project", "/tmp/autosettle", 1n},
      M.ThreadCreated{"t100-3", "p", "Human", M.Local{}, "", "", "codex", "", 100n},
      M.ThreadParentSet{"t100-3", "parent", 100n}, M.MessagePosted{"t100-3", "m100-4", M.User{}, "Actual human prompt", 100n},
      M.MessagePosted{"t100-3", "answer", M.Assistant{}, "Done", 300n}, M.TurnChanged{"t100-3", M.Completed{}, 300n}], M.State.empty())
    C.Check.str("parent link alone never exempts a human prompt", due(personal, "t100-3"), "False")
