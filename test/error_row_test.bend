import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/hub.bend as H
import ../src/core/fold.bend as FO
import ../src/core/model.bend as M

# An agent's error result always says something on its row: its result,
# else its errors, else its subtype; and an old empty one reads as what
# happened, not an empty "result" chip.

def j(+s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def rows(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}: ""
    case M.ActivityLogged{_, _, "error", "result", +sum, _} <> t: sum ++ "|" ++ rows(t)
    case _ <> t: rows(t)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("the result's words", H.Claude.result.why(j("{\"type\":\"result\",\"is_error\":true,\"result\":\"API Error: 529 Overloaded\"}")), "API Error: 529 Overloaded")
    C.Check.str("else its errors", H.Claude.result.why(j("{\"type\":\"result\",\"subtype\":\"error_during_execution\",\"is_error\":true,\"errors\":[\"Tool failed\",\"Aborted\"]}")), "Tool failed; Aborted")
    C.Check.str("else its subtype", H.Claude.result.why(j("{\"type\":\"result\",\"subtype\":\"error_during_execution\",\"is_error\":true}")), "error_during_execution")
    C.Check.str("an empty one in an old log still says what happened", FO.Tool.text("result", ""), "The turn ended with an error; the agent gave no reason.")
    C.Check.str("a tool's summary as before", FO.Tool.text("Bash", "ls"), "ls")
    C.Check.str("a summary that is its kind shows nothing more", FO.Tool.text("Bash", "Bash"), "")
    C.Check.str("a failed command's ending (lifecycle) with no text takes the result's", rows(H.Queue.changes(H.Claude.line.v.k(4n, H.Rec.demo(M.Running{}), "t1", 5n, j("{\"type\":\"result\",\"subtype\":\"error_during_execution\",\"is_error\":true,\"errors\":[\"Aborted\"]}"), ""))), "Aborted|")
    C.Check.str("and keeps the ending's own when it has one", rows(H.Queue.changes(H.Claude.line.v.k(4n, H.Rec.demo(M.Running{}), "t1", 5n, j("{\"type\":\"result\",\"is_error\":true}"), "Session limit"))), "Session limit|")
