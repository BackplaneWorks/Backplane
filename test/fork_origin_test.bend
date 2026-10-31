import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/client.bend as U

def state() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "P", "/r", 1n}, M.ThreadCreated{"a", "p", "Source conversation", M.Local{}, "", "", "claude", "", 1n}, M.ThreadCreated{"b", "p", "Fork", M.Local{}, "", "", "claude", "", 2n}, M.ThreadForked{"b", "a", 2n}], M.State.empty())

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("fork links to source", U.Tasks.up(state(), "b"), "a")
    C.Check.str("fork names source", U.Tasks.up.label(state(), "b", "a"), "← forked from Source conversation")
    C.Check.str("ordinary thread has no origin", U.Tasks.up(state(), "a"), "")
