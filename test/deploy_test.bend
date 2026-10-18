import Base
import ../src/core/i18n.bend as I
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/codec.bend as K
import ../src/core/json.bend as J
import ../src/core/hub.bend as H
import ../src/core/mcp.bend as MC
import ../src/core/deploy.bend as DP
import ../src/core/plane.bend as PLN

# Safe deploy and restart (core/deploy.bend, hub.bend's Deploy.*): a
# request waits for no tool call in flight and the build locks free, then
# goes once; the start after tells the requester once.

def cs(st: H.Step) -> List<&2, M.Change>:
  H.Deploy.demo.cs(st)

def outs(st: H.Step) -> Nat:
  H.Deploy.demo.go(st)

def res.cs(r: MC.Res) -> List<&2, M.Change>:
  match r:
    case MC.Res{_, _, H.Step{c, _}}:
      c

def res.body(r: MC.Res) -> String:
  match r:
    case MC.Res{_, b, _}:
      J.Json.show(b)

def res.ok(r: MC.Res) -> String:
  match r:
    case MC.Res{o, _, _}:
      Bool.show(o)

# the text of the first message a step posts
def said(xs: List<&2, M.Change>) -> String:
  match xs:
    case Nil{}:
      ""
    case M.MessagePosted{_, _, _, +text, _} <> _:
      text
    case _ <> rest:
      said(rest)

def line() -> String:
  H.Deploy.demo.line("t0")

# the same line as the last look left it: two tool calls open
def lineo() -> String:
  DP.Deploy.line.o("t0", "dev.sh", "", "0.9.0", 5n, False{}, False{}, "t1", "t1,t1")

def tool.pos(+tool: String) -> Nat:
  K.Codec.pos(MC.Mcp.names(), tool)

def main() -> IO(Unit):
  do IO<Unit>:
    +busy : M.State = H.Deploy.demo(True{}, line())
    +busyo : M.State = H.Deploy.demo(True{}, lineo())
    +clear : M.State = H.Deploy.demo(False{}, line())
    # tool calls in flight: the hub's list, not the log
    +op : List<&2, String> = H.Deploy.demo.open(True{})
    C.Check.str("a call opens", Nat.show(DP.Open.count(op)), "1")
    C.Check.str("opening it again changes nothing", Nat.show(DP.Open.count(DP.Open.add(op, "t1", "a1"))), "1")
    C.Check.str("another call of the thread", Nat.show(DP.Open.count(DP.Open.add(op, "t1", "a2"))), "2")
    C.Check.str("a result shuts its call", Nat.show(DP.Open.count(DP.Open.shut(op, "t1", "a1"))), "0")
    C.Check.str("another call's result shuts nothing", Nat.show(DP.Open.count(DP.Open.shut(op, "t1", "zz"))), "1")
    C.Check.str("a turn's end shuts them all", Nat.show(DP.Open.count(DP.Open.gone(DP.Open.add(op, "t1", "a2"), "t1"))), "0")
    C.Check.str("and only that thread's", Nat.show(DP.Open.count(DP.Open.gone(DP.Open.add(op, "t2", "b1"), "t1"))), "1")
    C.Check.str("a thread with a similar id keeps its calls", Nat.show(DP.Open.count(DP.Open.gone(DP.Open.add(Nil{}, "t10", "a1"), "t1"))), "1")
    C.Check.str("the open threads", DP.Open.threads(DP.Open.add(DP.Open.add(op, "t1", "a2"), "t2", "b1")), "t2,t1,t1")
    C.Check.str("the line recorded none", Nat.show(DP.Deploy.count(line())), "0")
    C.Check.str("the line recorded two", Nat.show(DP.Deploy.count(lineo())), "2")
    C.Check.str("the requester's own deploy call is never opened", Nat.show(List.length(&2, H.Out, H.Tools.opens([M.ActivityLogged{"t1", "a2", "tool", "mcp__backplane__deploy_restart", "x", 6n}], "t1"))), "0")
    C.Check.str("another tool call is", Nat.show(List.length(&2, H.Out, H.Tools.opens([M.ActivityLogged{"t1", "a2", "tool", "Bash", "x", 6n}], "t1"))), "1")
    C.Check.str("a subagent's call is not the thread's", Nat.show(List.length(&2, H.Out, H.Tools.opens([M.ActivityLogged{"t1.log", "a2", "tool", "Bash", "x", 6n}], "t1"))), "0")
    C.Check.str("a turn that ends ends its calls", Nat.show(List.length(&2, H.Out, H.Tools.gone([M.TurnChanged{"t1", M.Idle{}, 6n}]))), "1")
    C.Check.str("a turn that starts does not", Nat.show(List.length(&2, H.Out, H.Tools.gone([M.TurnChanged{"t1", M.Running{}, 6n}]))), "0")
    C.Check.str("an archive ends them", Nat.show(List.length(&2, H.Out, H.Tools.gone([M.ThreadArchived{"t1", True{}, 6n}]))), "1")
    C.Check.str("a result line shuts its calls", Nat.show(List.length(&2, H.Out, H.Tools.shuts(J.Json.elems(Maybe.default(&2, J.Json, J.Json.parse("[{\"type\":\"tool_result\",\"tool_use_id\":\"a1\"},{\"type\":\"text\"},{\"type\":\"tool_result\",\"tool_use_id\":\"a2\"}]"), J.Null{})), "t1"))), "2")
    # waiting
    C.Check.str("waits while a tool runs", Nat.show(outs(H.Deploy.check(busy, 9n, True{}, H.Deploy.demo.open(True{})))), "0")
    C.Check.str("waits while a lock is held", Nat.show(outs(H.Deploy.check(clear, 9n, False{}, Nil{}))), "0")
    C.Check.str("goes when clear", Nat.show(outs(H.Deploy.check(clear, 9n, True{}, Nil{}))), "1")
    C.Check.str("goes between calls, though the newest entry is a tool call", Nat.show(outs(H.Deploy.check(busy, 9n, True{}, H.Deploy.demo.open(False{})))), "1")
    C.Check.str("what is open is remembered", DP.Deploy.open(M.Setting.get(M.State.replay(cs(H.Deploy.check(busy, 9n, True{}, H.Deploy.demo.open(True{}))), busy), "deploy.wait", "")), "t1")
    C.Check.str("and not rewritten when it is the same", Nat.show(List.length(&2, M.Change, cs(H.Deploy.check(M.State.replay(cs(H.Deploy.check(busy, 9n, True{}, H.Deploy.demo.open(True{}))), busy), 10n, True{}, H.Deploy.demo.open(True{}))))), "0")
    C.Check.str("the lock is remembered", Bool.show(DP.Deploy.locked(M.Setting.get(M.State.replay(cs(H.Deploy.check(clear, 9n, False{}, Nil{})), clear), "deploy.wait", ""))), "True")
    C.Check.str("done keeps who was cut", DP.Deploy.cut(M.Setting.get(M.State.replay(cs(H.Deploy.check(clear, 9n, True{}, Nil{})), clear), "deploy.done", "")), "t1")
    C.Check.str("done is the wait's line", DP.Deploy.by(M.Setting.get(M.State.replay(cs(H.Deploy.check(clear, 9n, True{}, Nil{})), clear), "deploy.done", "")), "t0")
    C.Check.str("the wait is gone after the go", Bool.show(DP.Deploy.waiting(M.State.replay(cs(H.Deploy.check(clear, 9n, True{}, Nil{})), clear))), "False")
    # the tick always looks (the hub's open calls are known to the server)
    C.Check.str("tick: a look", Nat.show(List.length(&2, H.Out, H.Step.outs_of(H.Deploy.tick(busy, 9n)))), "1")
    C.Check.str("tick: nothing waits, no look", Nat.show(List.length(&2, H.Out, H.Step.outs_of(H.Deploy.tick(M.State.empty(), 9n)))), "0")
    # slow: told once after 30 minutes
    C.Check.str("not told early", Nat.show(H.Deploy.demo.notes(H.Deploy.tick(busyo, 100n))), "0")
    +late : H.Step = H.Deploy.tick(busyo, 2000n)
    C.Check.str("told when slow", Nat.show(H.Deploy.demo.notes(late)), "1")
    C.Check.str("what blocks", Bool.show(String.contains(said(cs(late)), "waiting for 2 tool calls")), "True")
    C.Check.str("only once", Nat.show(H.Deploy.demo.notes(H.Deploy.tick(M.State.replay(cs(late), busyo), 2100n))), "0")
    # after
    +done : M.State = H.Deploy.demo.done("t0")
    C.Check.str("the start tells the requester", said(cs(H.Deploy.after(done, 9n))), "Backplane restarted for a deploy by t0 (dev.sh): 0.9.0 → 0.1.0-dev.")
    C.Check.str("a cut thread is not told twice", Nat.show(H.Deploy.demo.notes(H.Deploy.after(H.Deploy.demo.done("t1"), 9n))), "0")
    C.Check.str("a person's deploy tells no one", Nat.show(H.Deploy.demo.notes(H.Deploy.after(H.Deploy.demo.done(""), 9n))), "0")
    C.Check.str("a failed swap tells the requester", Bool.show(String.contains(said(cs(H.Deploy.failed(done, 9n, "disk full"))), "disk full")), "True")
    C.Check.str("and clears done", Nat.show(H.Deploy.demo.sets(cs(H.Deploy.failed(done, 9n, "disk full")), "deploy.done", True{})), "1")
    # the continue message of a cut thread carries the words
    C.Check.str("the words ride the continue message", Bool.show(String.contains(DP.Deploy.suffix(done, "0.2.0"), "restarted for a deploy by t0 (dev.sh): 0.9.0 → 0.2.0. Your turn was resumed.")), "True")
    C.Check.str("no deploy, no words", DP.Deploy.suffix(clear, "0.2.0"), "")
    C.Check.str("a continue message with the words is still the continue message", Bool.show(H.Carry.is(M.Carry.text() ++ DP.Deploy.suffix(done, "0.2.0"))), "True")
    C.Check.str("another message is not", Bool.show(H.Carry.is("go on")), "False")
    # words
    C.Check.str("the plane's line", DP.Deploy.row(busyo), "Restart pending · waiting for 2 tool calls")
    C.Check.str("two calls and the lock", DP.Deploy.blocking(2n, True{}), "waiting for 2 tool calls and the build lock")
    C.Check.str("no row when nothing waits", DP.Deploy.row(M.State.replay([M.SettingSet{"deploy.wait", "", 7n}], busy)), "")
    C.Check.str("the plane row", Nat.show(List.length(&2, PLN.PRow, PLN.Plane.deploy.rows(I.En{}, busyo))), "1")
    # MCP
    C.Check.str("the tools are named", Bool.show(Nat.is_lt(tool.pos("deploy_cancel"), 100n)), "True")
    +r : MC.Res = MC.Mcp.call.k(tool.pos("deploy_restart"), clear, 20n, "t0", J.Null{}, Maybe.default(&2, J.Json, J.Json.parse("{\"note\":\"ship\",\"stage\":\"/tmp/x\"}"), J.Null{}))
    C.Check.str("deploy_restart answers at once", res.ok(r), "True")
    C.Check.str("it says scheduled", Bool.show(String.contains(res.body(r), "scheduled")), "True")
    C.Check.str("it sets the wait", Nat.show(H.Deploy.demo.sets(res.cs(r), "deploy.wait", False{})), "1")
    C.Check.str("by the caller, with its stage", DP.Deploy.stage(M.Setting.get(M.State.replay(res.cs(r), M.State.empty()), "deploy.wait", "")), "/tmp/x")
    C.Check.str("the person's script is no thread", DP.Deploy.by(M.Setting.get(M.State.replay(res.cs(MC.Mcp.call.k(tool.pos("deploy_restart"), clear, 20n, "deploy", J.Null{}, J.Null{})), M.State.empty()), "deploy.wait", "")), "")
    C.Check.str("a relative stage is refused", res.ok(MC.Mcp.call.k(tool.pos("deploy_restart"), clear, 20n, "t0", J.Null{}, Maybe.default(&2, J.Json, J.Json.parse("{\"stage\":\"rel\"}"), J.Null{}))), "False")
    C.Check.str("status says who waits", Bool.show(String.contains(res.body(MC.Mcp.call.k(tool.pos("deploy_status"), busyo, 20n, "t0", J.Null{}, J.Null{})), "\"inflight\":2")), "True")
    C.Check.str("cancel clears", Bool.show(DP.Deploy.waiting(M.State.replay(res.cs(MC.Mcp.call.k(tool.pos("deploy_cancel"), busy, 20n, "t0", J.Null{}, J.Null{})), busy))), "False")
    C.Check.str("the status tool only reads", Bool.show(H.Gen.reads("deploy_status")), "True")
    C.Check.str("restarting changes", Bool.show(H.Gen.reads("deploy_restart")), "False")
    # a second request replaces the first
    +again : M.State = M.State.replay(res.cs(r), busy)
    C.Check.str("a newer request replaces the wait", DP.Deploy.stage(M.Setting.get(M.State.replay(cs(H.Deploy.ask(again, 30n, "t0", "n", "/tmp/y")), again), "deploy.wait", "")), "/tmp/y")
    # request methods
    C.Check.str("deploy.restart refuses a relative stage", Nat.show(H.Deploy.demo.sets(cs(H.Deploy.restart(H.Deploy.stage.ok("rel"), clear, 9n, 1, "5", "rel", "")), "deploy.wait", False{})), "0")
    C.Check.str("and takes an absolute one", Nat.show(H.Deploy.demo.sets(cs(H.Deploy.restart(H.Deploy.stage.ok("/tmp/z"), M.State.empty(), 9n, 1, "5", "/tmp/z", "")), "deploy.wait", False{})), "1")
    C.Check.str("an install in place is allowed", Bool.show(H.Deploy.stage.ok("")), "True")
