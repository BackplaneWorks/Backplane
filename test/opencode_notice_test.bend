import Base
import ./check.bend as C
import ../src/core/acp.bend as A
import ../src/core/msp.bend as P
import ../src/core/opencode.bend as O
import ../src/core/hub.bend as H
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/thin-chat.bend as T

def turn() -> O.OcpTurn:
  O.OcpTurn{A.AcpTurn{"/tmp/fixture", "http://127.0.0.1:1/mcp/fixture", "RULES", "hello", "old", False{}}, ""}

def sends(o: A.AcpOut) -> String:
  match o:
    case A.AcpOut{_, send, _, _}: String.join(send, "\n")

def emissions(o: A.AcpOut) -> List<&2, String>:
  match o:
    case A.AcpOut{_, _, emit, _}: emit

def notice.kind(s: H.Step) -> String:
  match s:
    case H.Step{[M.ActivityLogged{_, _, tone, kind, _, _}], Nil{}}: tone ++ "/" ++ kind
    case _: "unexpected"

def notice.visible(s: H.Step) -> String:
  match s:
    case H.Step{[M.ActivityLogged{th, id, tone, kind, text, at}], Nil{}}:
      Bool.show(Bool.not(T.Chat.receipt(M.Act{id, th, tone, kind, text, at})))
    case _: "unexpected"

def main() -> IO(Unit):
  do IO<Unit>:
    +init : A.AcpOut = P.Proto.line(A.Acp.new(), P.POc{turn()}, "{\"id\":1,\"result\":{}}")
    C.Check.str("OpenCode protocol dispatch uses resume", Bool.show(String.contains(sends(init), "session/resume")), "True")
    +renewed : A.AcpOut = O.Ocp.line(A.Acp.new(), turn(), "{\"id\":7,\"result\":{\"sessionId\":\"fresh\"}}")
    +line : String = Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, emissions(renewed), 1n)), "")
    +step : H.Step = H.Codex.line.parsed(J.Json.parse(line), M.State.empty(), "fixture", 10n)
    C.Check.str("renewal notice is one informational session activity without side effects", notice.kind(step), "info/session")
    C.Check.str("renewal notice survives bot receipt filter", notice.visible(step), "True")
