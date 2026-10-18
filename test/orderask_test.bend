import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/hub.bend as H

# An order's question (order_ask) parks nothing: the person's answer is
# posted in the buy thread as their message, however long they took, and a
# dismissal only closes it (hub.bend's Hub.answer.order). A Stop leaves it
# open (Retire.hit).

def log(+turn: M.TurnState) -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "lamp", "/tmp/lamp", 1n},
   M.ThreadCreated{"t1", "p1", "Buy: lamp", M.Local{}, "", "", "claude", "", 1n},
   M.TurnChanged{"t1", turn, 2n},
   M.AskOpened{"a3", "t1", "input", "order_ask", "Fix these before ordering, or order as it is?", "Fix them first\nOrder anyway", 3n}]

def st(+turn: M.TurnState) -> M.State:
  M.State.replay(log(turn), M.State.empty())

def said(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}:
      ""
    case M.AskClosed{+id, _, _} <> t:
      "closed:" ++ id ++ " " ++ said(t)
    case M.MessagePosted{+th, _, M.User{}, +text, _} <> t:
      "user@" ++ th ++ ":" ++ text ++ " " ++ said(t)
    case _ <> t:
      said(t)

def answered(+turn: M.TurnState, +ans: String) -> String:
  said(H.Queue.changes(H.Hub.answer(st(turn), 9n, 0, "r", "a3", ans)))

def open(+s: M.State) -> String:
  Nat.show(H.Pend.open(M.Asks.all(s), "t1"))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("a choice is posted as the person's message", Bool.show(String.contains(answered(M.Completed{}, "Fix them first"), "user@t1:Fix them first")), "True")
    C.Check.str("and the question closes", Bool.show(String.contains(answered(M.Completed{}, "Fix them first"), "closed:a3")), "True")
    C.Check.str("a typed answer is posted as it was typed", Bool.show(String.contains(answered(M.Completed{}, "  Use PLA please "), "user@t1:Use PLA please")), "True")
    C.Check.str("a dismissal only closes it", answered(M.Completed{}, ""), "closed:a3 ")
    C.Check.str("a Stop leaves it open", open(H.Retire.after(st(M.Running{}), H.Hub.interrupt(st(M.Running{}), 9n, 0, "r", "t1"))), "1")
