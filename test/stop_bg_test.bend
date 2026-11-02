import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/core/hub.bend as B

# A Claude turn that ends interrupted, stopped or failed stops the
# background tasks it left at work (hub.bend's Bg.stop.outs), so none of
# them can wake the thread later (Claude.line.v.k's 6). A row already
# ended, a turn that completed, and a Codex thread send nothing.

def base(+agent: String, +bg: String) -> M.State:
  M.State.replay([M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"t1", "p1", "x", M.Local{}, "", "", agent, "", 1n},
    M.TurnChanged{"t1", M.Running{}, 2n}, M.WorkSet{"t1", "bg", bg, 2n}], M.State.empty())

def two() -> String:
  "b1\tsleep 600\tc1\tsleep 600\tshell\nb2\tnpm run dev\tc2\tnpm run dev\tshell"

def ended() -> String:
  "b1\t" ++ M.Bg.ended.mark() ++ "completed"

def stops(os: List<&2, B.Out>) -> List<&2, String>:
  match os:
    case Nil{}:
      Nil{}
    case B.AgentSend{_, +line} <> +t:
      +o = Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{})
      +r = J.Json.get_obj(o, "request")
      Bool.pick(List<&2, String>, String.eq(J.Json.get_str(r, "subtype", ""), "stop_task"), J.Json.get_str(r, "task_id", "") <> stops(t), stops(t))
    case _ <> t:
      stops(t)

def outs(st: B.Step) -> List<&2, B.Out>:
  match st:
    case B.Step{_, os}:
      os

def line(+v: Nat, +s: M.State) -> String:
  String.join(stops(outs(B.Claude.line.v.k(v, s, "t1", 5n, J.Null{}, "x"))), ",")

def stop(+s: M.State) -> String:
  String.join(stops(outs(B.Retire.stop(s, 5n, "t1", Nil{}))), ",")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("stop stops both", stop(base("claude", two())), "b1,b2")
    C.Check.str("interrupt stops both", line(5n, base("claude", two())), "b1,b2")
    C.Check.str("failure stops both", line(4n, base("claude", two())), "b1,b2")
    C.Check.str("completed leaves them", line(3n, base("claude", two())), "")
    C.Check.str("an ended row is left", line(5n, base("claude", ended())), "")
    C.Check.str("nothing out, nothing sent", stop(base("claude", "")), "")
    C.Check.str("codex is stopped whole", stop(base("codex", two())), "")
    C.Check.str("request shape", B.Claude.stop_task("r", "b1"), "{\"type\":\"control_request\",\"request_id\":\"r\",\"request\":{\"subtype\":\"stop_task\",\"task_id\":\"b1\"}}")
