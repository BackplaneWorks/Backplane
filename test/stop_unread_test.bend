import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/hub.bend as H
import ../src/core/cmds.bend as CM

# Messages a Stop cancelled before Claude started them (hub.bend's
# Unread.note, cmds.bend's Cmds.waiting): Claude Code runs the person's
# messages one at a time, so one sent while a turn works waits queued in
# the CLI and Stop cancels it. The thread names them; they look sent.

def state() -> M.State:
  M.State.replay([M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"t", "p1", "one", M.Local{}, "", "", "claude", "", 1n},
    M.MessagePosted{"t", "u1", M.User{}, "do the long check", 2n}, M.TurnChanged{"t", M.Running{}, 2n},
    M.MessagePosted{"t", "u2", M.User{}, "also look at the antenna", 5n}, M.MessagePosted{"t", "u3", M.User{}, "and the BOM\nplease", 6n},
    M.MessagePosted{"o", "x1", M.User{}, "another thread", 6n}], M.State.empty())

# the uuid the hub gave a message's command (Hub.post.go)
def uuid(+th: String, +msg: String, +at: Nat) -> String:
  H.Hub.uuid(at, th ++ "/" ++ msg)

def said(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}:
      ""
    case M.ActivityLogged{th, _, tone, kind, text, _} <> rest:
      th ++ "|" ++ tone ++ "|" ++ kind ++ "|" ++ text ++ said(rest)
    case _ <> rest:
      said(rest)

def ledger() -> CM.Cmds:
  CM.Cmds{[CM.Cmd{"a", 2n, False{}, ""}, CM.Cmd{"b", 1n, False{}, ""}, CM.Cmd{"c", 3n, False{}, ""}, CM.Cmd{"d", 1n, False{}, ""}, CM.Cmd{"e", 0n, False{}, ""}], 1n, False{}}

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("the ledger's commands written but never started", String.join(CM.Cmds.waiting(ledger()), " "), "b d")
    C.Check.str("two unread follow-ups are named, oldest first", said(H.Unread.note(state(), "t", [uuid("t", "u3", 6n), uuid("t", "u2", 5n)], 9n)),
      "t|error|stop|Stop cancelled 2 messages the agent had not read yet: \u{201C}also look at the antenna\u{201D} · \u{201C}and the BOM please\u{201D}. Send them again if you still want them.")
    C.Check.str("one", said(H.Unread.note(state(), "t", [uuid("t", "u2", 5n)], 9n)),
      "t|error|stop|Stop cancelled a message the agent had not read yet: \u{201C}also look at the antenna\u{201D}. Send it again if you still want it.")
    C.Check.str("no uuid of this thread: nothing", Nat.show(List.length(&2, M.Change, H.Unread.note(state(), "t", [uuid("o", "x1", 6n), "zz"], 9n))), "0")
    C.Check.str("none: nothing", Nat.show(List.length(&2, M.Change, H.Unread.note(state(), "t", Nil{}, 9n))), "0")
