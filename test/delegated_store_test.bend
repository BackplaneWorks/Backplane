import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/storedb.bend as S
import ../src/core/client.bend as U

def changes() -> List<&2, M.Change>:
  [M.ThreadCreated{"parent", "p", "Parent", M.Local{}, "", "", "claude", "", 1n},
    M.ThreadCreated{"child", "p", "Child", M.Local{}, "", "", "claude", "", 1n},
    M.TaskDelegated{"k", "parent", "child", 1n, "claude", "", 2n}]

def lines(cs: List<&2, M.Change>) -> List<&2, S.LL>:
  match cs:
    case Nil{}: Nil{}
    case c <> rest: S.LL{1n, True{}, Some{c}} <> lines(rest)

def recs(cs: List<&2, M.Change>) -> Map<&2, String>:
  S.Mat.recs(S.Mat.replay(lines(cs), S.Mat.empty()))

def parent(+cs: List<&2, M.Change>) -> String:
  U.Str.get(recs(cs), S.Tab.key("setting", ["thread.parent.child"]))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("incremental checkpoint records delegated parent", parent(changes()), "thread.parent.child\u{1f}parent\u{1f}0")
    C.Check.str("delivery preserves recorded parent", parent(List.append(&2, M.Change, changes(), [M.TaskStateSet{"k", M.TaskDone{}, "done", 3n}, M.TaskDelivered{"k", 3n}])), parent(changes()))
    C.Check.str("explicit detach durably records a root", parent(List.append(&2, M.Change, changes(), [M.ThreadParentSet{"child", "", 3n}])), "thread.parent.child\u{1f}\u{1f}0")
    C.Check.str("reparent replaces durable parent", parent(List.append(&2, M.Change, changes(), [M.ThreadParentSet{"child", "other", 3n}])), "thread.parent.child\u{1f}other\u{1f}0")
