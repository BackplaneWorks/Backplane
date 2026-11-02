import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/core/hub.bend as H
import ../src/core/mcp.bend as MC

# An agent's question shows its options' descriptions, and what the person
# answered (or allowed) stays in the thread once the card goes.

def j(+s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def q() -> J.Json:
  j("{\"questions\":[{\"question\":\"Which connector?\",\"options\":[{\"label\":\"USB-C\",\"description\":\"reversible\"},{\"label\":\"Micro-B\",\"description\":\"cheaper\"}]}]}")

def two() -> J.Json:
  j("{\"questions\":[{\"question\":\"Which connector?\",\"options\":[{\"label\":\"USB-C\",\"description\":\"reversible\"},{\"label\":\"Micro-B\"}]},{\"question\":\"Add test points?\",\"options\":[{\"label\":\"Yes\"},{\"label\":\"No\"}]}]}")

def said(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}: ""
    case M.ActivityLogged{th, _, _, "answer", +sum, _} <> t: th ++ ": " ++ sum ++ "\n" ++ said(t)
    case _ <> t: said(t)

def asked(+kind: String, +tool: String, +detail: String) -> M.State:
  M.State.replay([M.ProjectCreated{"p", "P", "/r", 1n}, M.ThreadCreated{"t", "p", "T", M.Local{}, "", "", "claude", "", 1n},
    M.AskOpened{"q1", "t", kind, tool, detail, "", 2n}], M.State.empty())

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("the card has each option's description", MC.Perm.question.card(q()), "Which connector?\nUSB-C: reversible\nMicro-B: cheaper")
    C.Check.str("the answer is still keyed by the question", MC.Perm.question(q()), "Which connector?")
    C.Check.str("an answered question stays in the thread", said(H.Queue.changes(H.Hub.answer(asked("input", "AskUserQuestion", "Which connector?\nUSB-C: reversible"), 5n, 0, "1", "q1", "USB-C"))), "t: Which connector? → USB-C\n")
    C.Check.str("an approval too", said(H.Queue.changes(H.Hub.answer(asked("approval", "Bash", "rm -rf build/"), 5n, 0, "1", "q1", "accept"))), "t: Allowed: Bash rm -rf build/\n")
    C.Check.str("and a denial", said(H.Queue.changes(H.Hub.answer(asked("approval", "Bash", "rm -rf build/"), 5n, 0, "1", "q1", "deny"))), "t: Denied: Bash rm -rf build/\n")
    C.Check.str("several questions: the card lists them all", MC.Perm.question.card(two()), "1. Which connector? (USB-C / Micro-B)\n   USB-C: reversible\n2. Add test points? (Yes / No)\nType your answers below, then answer with them.")
    C.Check.str("with no single-choice buttons", MC.Perm.options(two()), "")
    C.Check.str("and the typed answer goes to each", J.Json.show(J.Json.get_obj(MC.Perm.answers(two(), "1 USB-C, 2 yes"), "answers")), "{\"Which connector?\":\"1 USB-C, 2 yes\",\"Add test points?\":\"1 USB-C, 2 yes\"}")
    C.Check.str("one question is answered as before", J.Json.show(J.Json.get_obj(MC.Perm.answers(q(), "USB-C"), "answers")), "{\"Which connector?\":\"USB-C\"}")
    C.Check.str("with its buttons", MC.Perm.options(q()), "USB-C\nMicro-B")
