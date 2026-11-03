import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/hub.bend as H
import ../src/core/fold.bend as FO
import ../src/core/model.bend as M
import ../src/core/i18n.bend as I

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

# A reply that is Claude Code's API error reads as one sentence (ApiErr.say),
# and the turn's error row that repeats it or says nothing is left out.
def say(+x: String) -> String:
  FO.ApiErr.say(I.En{}, x)

def ids(es: List<&2, M.Entry>) -> String:
  match es:
    case Nil{}: ""
    case M.Msg{+id, _, _, _, _} <> t: id ++ "|" ++ ids(t)
    case M.Act{+id, _, _, _, _, _} <> t: id ++ "|" ++ ids(t)

def overloaded() -> String:
  "API Error: 529 {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}"

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
    C.Check.str("an overloaded API reads as one sentence", say(overloaded()), "Claude's API was overloaded (529) and ended the turn.")
    C.Check.str("a rate limit", say("API Error: 429 {\"type\":\"error\",\"error\":{\"type\":\"rate_limit_error\",\"message\":\"Too many\"}}"), "Claude's API hit its rate limit (429) and ended the turn.")
    C.Check.str("a server error by its status alone", say("API Error: 503 upstream connect error"), "Claude's API had an internal error (503) and ended the turn.")
    C.Check.str("a refused sign-in", say("API Error: 401 {\"type\":\"error\",\"error\":{\"type\":\"authentication_error\",\"message\":\"invalid x-api-key\"}}"), "Claude's API refused the sign-in (401) and ended the turn.")
    C.Check.str("any other says the API's words", say("API Error: 400 {\"type\":\"error\",\"error\":{\"type\":\"invalid_request_error\",\"message\":\"prompt is too long: 210000 tokens > 200000 maximum\"}}"), "Claude's API answered with an error (400) and ended the turn: prompt is too long: 210000 tokens > 200000 maximum.")
    C.Check.str("with no status, its words without their last dot", say("API Error: Connection error."), "Claude's API answered with an error and ended the turn: Connection error.")
    C.Check.str("in Japanese", FO.ApiErr.say(I.Ja{}, overloaded()), "ClaudeのAPIが過負荷になり (529)、ターンが終了しました。")
    C.Check.str("an API error reply is one", Bool.pick(String, FO.ApiErr.is(overloaded()), "True", "False"), "True")
    C.Check.str("a reply that mentions one is not", Bool.pick(String, FO.ApiErr.is("The API Error: 529 means overloaded"), "True", "False"), "False")
    C.Check.str("the empty error row after an API error reply is left out", ids(FO.Fold.shown([M.Msg{"m1", "t1", M.Assistant{}, overloaded(), 1n}, M.Act{"e1", "t1", "error", "result", "", 2n}])), "m1|")
    C.Check.str("so is one that repeats it", ids(FO.Fold.shown([M.Msg{"m1", "t1", M.Assistant{}, overloaded(), 1n}, M.Act{"e1", "t1", "error", "result", overloaded(), 2n}])), "m1|")
    C.Check.str("an error row that says more stays", ids(FO.Fold.shown([M.Msg{"m1", "t1", M.Assistant{}, overloaded(), 1n}, M.Act{"e1", "t1", "error", "result", "Aborted", 2n}])), "m1|e1|")
    C.Check.str("an empty error row after any other reply stays (it says the agent gave no reason)", ids(FO.Fold.shown([M.Msg{"m1", "t1", M.Assistant{}, "Starting on it", 1n}, M.Act{"e1", "t1", "error", "result", "", 2n}])), "m1|e1|")
    C.Check.str("a repeated non-API error stays as it was", ids(FO.Fold.shown([M.Msg{"m1", "t1", M.Assistant{}, "You've hit your session limit", 1n}, M.Act{"e1", "t1", "error", "result", "You've hit your session limit", 2n}])), "m1|e1|")
    C.Check.str("a blank reply between keeps it out", ids(FO.Fold.shown([M.Msg{"m1", "t1", M.Assistant{}, overloaded(), 1n}, M.Msg{"m2", "t1", M.Assistant{}, "  ", 2n}, M.Act{"e1", "t1", "error", "result", "", 3n}])), "m1|")
