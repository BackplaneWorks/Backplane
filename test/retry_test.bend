import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/tools.bend as TO
import ../src/core/i18n.bend as I

# Retry on a failed turn (core/tools.bend's Tools.retry, client.bend's
# Ui.retry): where Stop sits while a turn runs, a failed turn offers Retry,
# which sends the person's last message in that thread again.

def ui(cs: List<&2, M.Change>) -> U.Ui:
  +st = M.State.replay(List.append(&2, M.Change, [M.ProjectCreated{"p", "P", "/r", 1n}, M.ThreadCreated{"t", "p", "T", M.Local{}, "", "", "claude", "", 1n},
    M.ThreadCreated{"o", "p", "O", M.Local{}, "", "", "claude", "", 1n}], cs), M.State.empty())
  U.Act.ui(U.Ui.act(U.Ui.adopt(U.Ui.init(0n), st), "select", "t"))

def failed() -> U.Ui:
  ui([M.MessagePosted{"t", "m1", M.User{}, "Move J1 right", 2n}, M.MessagePosted{"t", "a1", M.Assistant{}, "done", 3n},
    M.MessagePosted{"t", "m2", M.User{}, "Now route the I2C lines", 4n}, M.MessagePosted{"o", "m3", M.User{}, "another thread", 5n},
    M.MessagePosted{"t", "a2", M.Assistant{}, "API Error: 529", 6n}, M.TurnChanged{"t", M.Failed{}, 6n}])

def sent(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}: ""
    case U.Send{x} <> tail: x ++ sent(tail)
    case _ <> tail: sent(tail)

def main() -> IO(Unit):
  +out = sent(U.Act.cmds(U.Ui.act(failed(), "retry", "")))
  do IO<Unit>:
    C.Check.str("a failed turn offers Retry", String.join(TO.Tools.icons(TO.Tools.turn.loc(I.En{}, M.Failed{})), ","), "repeat")
    C.Check.str("a running turn offers Stop", String.join(TO.Tools.icons(TO.Tools.turn.loc(I.En{}, M.Running{})), ","), "stop")
    C.Check.str("a finished turn offers neither", String.join(TO.Tools.icons(TO.Tools.turn.loc(I.En{}, M.Completed{})), ","), "")
    C.Check.str("Retry starts a turn", Bool.show(String.contains(out, "turn.start")), "True")
    C.Check.str("in the thread on screen", Bool.show(String.contains(out, "\"thread\":\"t\"")), "True")
    C.Check.str("with the person's last message there", Bool.show(String.contains(out, "Now route the I2C lines")), "True")
    C.Check.str("not another thread's", Bool.show(String.contains(out, "another thread")), "False")
    C.Check.str("a thread with no message sends nothing", sent(U.Act.cmds(U.Ui.act(ui(Nil{}), "retry", ""))), "")
