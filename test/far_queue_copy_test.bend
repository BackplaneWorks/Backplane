import Base
import ./check.bend as C
import ../src/core/hub.bend as B
import ../src/core/copy.bend as CP

law queue_copy_refuses_network:
  {CP.Copy.outs(True{}, [B.FarQueued{}]) == [] : List<&2, B.Out>}
def queue_copy_refuses_network(): {==}

def git(os: List<&2, B.Out>) -> String:
  match os:
    case B.Git{_, _, j, _, c, a, _} <> _: j ++ " " ++ c ++ " " ++ a
    case _: "none"

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("copy refuses queued network dispatch", Nat.show(List.length(&2, B.Out, CP.Copy.outs(True{}, [B.FarQueued{}]))), "0")
    C.Check.str("ordinary hub still dispatches its queue", Nat.show(List.length(&2, B.Out, CP.Copy.outs(False{}, [B.FarQueued{}]))), "1")
    C.Check.str("a copy runs a new thread's worktree job on no repository", git(CP.Copy.outs(True{}, [B.Git{0, "", "worktree", "t1", "/real/project", "auto", ""}])), "worktree / auto")
    C.Check.str("an ordinary hub on the project", git(CP.Copy.outs(False{}, [B.Git{0, "", "worktree", "t1", "/real/project", "auto", ""}])), "worktree /real/project auto")
    C.Check.str("other git that writes is still dropped", Nat.show(List.length(&2, B.Out, CP.Copy.outs(True{}, [B.Git{0, "", "step", "t1", "/real/project", "", ""}]))), "0")
    C.Check.str("copy keeps the client reply when dropping network work", Nat.show(List.length(&2, B.Out, CP.Copy.outs(True{}, [B.FarQueued{}, B.Send{1, "answer"}]))), "1")
