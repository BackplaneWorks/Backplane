import Base
import ../../src/core/hub.bend as B
import ../../src/core/model.bend as M
import ../../src/core/codec.bend as C
import ../../src/core/json.bend as J
import ../../src/core/store.bend as ST
import ../../src/core/utf8.bend as U
import ../../src/server/argv.bend as AV
import ../../src/core/bothub.bend as BH
import ../../src/core/mcp.bend as MC
import ../../src/core/google.bend as GG
import ../../src/core/rules.bend as RLS
import ../../src/core/fmcp.bend as FM
import ../../src/core/far.bend as FR

# What one streamed agent line costs the hub's pure side, on a real log:
# the line's step (B.Hub.agent_line, by verdict as the server's Claude path
# takes it, and by the old rule) and each pass Hub.step runs over every
# step (wakes, locks, alerts, pending counts, outs), then a whole message's
# fold. Each part is run REPS times; ms per run.
#   scripts/build-js.sh test/native/hub_line_bench.bend build/hub_line_bench
#   build/hub_line_bench EVENTS.jsonl THREAD [REPS]   (only reads the log)

def copy(xs: List<String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      x <> copy(t)

def arg(xs: List<&2, String>, +i: Nat, d: String) -> String:
  match xs i:
    case Nil{} _:
      d
    case x <> _ 0n:
      x
    case _ <> t 1n+k:
      arg(t, k, d)

def Step.size(s: B.Step) -> Nat:
  match s:
    case B.Step{cs, os}:
      Nat.add(List.length(&2, M.Change, cs), List.length(&2, B.Out, os))

def Step.cs(s: B.Step) -> List<&2, M.Change>:
  match s:
    case B.Step{cs, _}:
      cs

def Step.os(s: B.Step) -> List<&2, B.Out>:
  match s:
    case B.Step{_, os}:
      os

def delta() -> String:
  "{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"the regulator's enable pin is pulled up \"}},\"parent_tool_use_id\":null,\"session_id\":\"s1\",\"uuid\":\"u1\"}"

def message() -> String:
  "{\"type\":\"assistant\",\"message\":{\"id\":\"msg_b1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"Moving R12 to the 3V3 rail fixes the sequencing, and C7 can stay where it is.\"}]},\"parent_tool_use_id\":null,\"session_id\":\"s1\",\"uuid\":\"u2\"}"

def rep.vo(+r: Nat, +st: M.State, +now: Nat, +th: String, +line: String, +o: J.Json) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(Step.size(B.Hub.agent_line.vo(2n, st, now, th, line, o, "")), rep.vo(q, st, now, th, line, o))

def rep.old(+r: Nat, +st: M.State, +now: Nat, +th: String, +line: String) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(Step.size(B.Hub.agent_line(st, now, th, line)), rep.old(q, st, now, th, line))

def rep.inner(+r: Nat, +st: M.State, +now: Nat, +th: String, +o: J.Json) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(Step.size(B.Claude.line.v.k(2n, st, th, now, o, "")), rep.inner(q, st, now, th, o))

def rep.ended(+r: Nat, +st: M.State, +now: Nat, +th: String, +s: B.Step) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(Step.size(B.Ended.out(st, now, th, s)), rep.ended(q, st, now, th, s))

def rep.drain(+r: Nat, +st: M.State, +now: Nat, +th: String, +s: B.Step) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(Step.size(B.Queue.drain(st, now, th, s)), rep.drain(q, st, now, th, s))

def rep.finish(+r: Nat, +st: M.State, +now: Nat, +th: String, +s: B.Step) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(Step.size(B.Task.finish.w(st, now, th, s)), rep.finish(q, st, now, th, s))

def rep.tell(+r: Nat, +st: M.State, +now: Nat, +th: String, +s: B.Step) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(Step.size(B.Launch.tell(st, now, th, s)), rep.tell(q, st, now, th, s))

def rep.wake(+r: Nat, +st: M.State, +s: B.Step) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(Step.size(B.Wake.after(st, s)), rep.wake(q, st, s))

def rep.locks(+r: Nat, +st: M.State, +now: Nat, +s: B.Step) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(Step.size(B.Locks.after(st, now, s)), rep.locks(q, st, now, s))

def rep.alerts(+r: Nat, +st: M.State, +cs: List<&2, M.Change>) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(List.length(&2, M.Change, B.Alerts.changes(st, cs)), rep.alerts(q, st, cs))

def rep.pend(+r: Nat, +st: M.State, +cs: List<&2, M.Change>) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(List.length(&2, M.Change, B.Pend.fix(st, cs)), rep.pend(q, st, cs))

def rep.outs(+r: Nat, +cs: List<&2, M.Change>, +os: List<&2, B.Out>) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(List.length(&2, B.Out, B.Island.outs(cs, B.Alerts.outs(cs, B.Tools.outs(cs, os)))), rep.outs(q, cs, os))

def fold(cs: List<&2, M.Change>, st: M.State, n: Nat) -> Nat:
  match cs:
    case Nil{}:
      Nat.add(n, M.State.seq(st))
    case +c <> t:
      fold(t, M.State.apply(st, c), Nat.add(n, String.length(J.Json.show(C.Change.wire(c)))))

def rep.fold(+r: Nat, +st: M.State, +cs: List<&2, M.Change>) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(fold(cs, st, 0n), rep.fold(q, st, cs))

def rep.busy(+r: Nat, +st: M.State) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(M.Work.threads(st), rep.busy(q, st))

def rep.seq(+r: Nat, +st: M.State) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(M.State.seq(st), rep.seq(q, st))

def call(+tool: String, +args: String) -> String:
  "{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"tools/call\",\"params\":{\"name\":\"" ++ tool ++ "\",\"arguments\":" ++ args ++ "}}"

def Out.size(o: MC.McpOut) -> Nat:
  match o:
    case MC.McpOut{r, st, _}:
      Nat.add(String.length(r), Step.size(st))

def Out.step(o: MC.McpOut) -> B.Step:
  match o:
    case MC.McpOut{_, st, _}:
      st

def rep.mcp(+r: Nat, +st: M.State, +now: Nat, +th: String, +body: String) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(Out.size(BH.Bots.mcp(st, now, th, J.Null{}, "v", J.Json.arr(Nil{}), GG.Google.tool_defs(), body)), rep.mcp(q, st, now, th, body))

def plan(p: FM.Plan) -> Nat:
  match p:
    case FM.PHere{}:
      1n
    case _:
      2n

def rep.pre(+r: Nat, +st: M.State, +th: String, +body: String) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(Nat.add(String.length(RLS.Rules.gate(st, th, body)), plan(FM.Fm.plan.o(Maybe.default(&2, J.Json, J.Json.parse(body), J.Null{}), st, Nil{}, th))), rep.pre(q, st, th, body))

def per(+what: String, +reps: Nat, +a: Nat, +b: Nat, +x: Nat) -> IO(Unit):
  IO.print(what ++ ": " ++ Nat.show(Nat.div(Nat.mul(Nat.sub(b, a), 1000n), reps)) ++ " us each  (" ++ Nat.show(x) ++ ")")

def mcp(+reps: Nat, +st: M.State, +now: Nat, +th: String, +tool: String, +args: String) -> IO(Unit):
  do IO<Unit>:
    +body : String = call(tool, args)
    +a0 : Nat <- IO.now()
    +x0 : Nat = rep.pre(reps, st, th, body)
    +a1 : Nat <- IO.now()
    per("mcp " ++ tool ++ ": rules + plan", reps, a0, a1, x0)
    +x1 : Nat = rep.mcp(reps, st, now, th, body)
    +a2 : Nat <- IO.now()
    per("mcp " ++ tool ++ ": Bots.mcp", reps, a1, a2, x1)
    +s : B.Step = Out.step(BH.Bots.mcp(st, now, th, J.Null{}, "v", J.Json.arr(Nil{}), GG.Google.tool_defs(), body))
    +x2 : Nat = rep.locks(reps, st, now, s)
    +x3 : Nat = rep.wake(reps, st, s)
    +a3 : Nat <- IO.now()
    per("mcp " ++ tool ++ ": its step's locks + wakes", reps, a2, a3, Nat.add(x2, x3))

def count(s: M.State) -> Nat:
  match s:
    case M.State{_, _, es, _, _, _, _, _, _, _, _}:
      List.length(&2, M.Entry, es)

def num(s: String, n: Nat) -> Nat:
  match s:
    case SNil{}:
      n
    case SCon{Chr{c}, t}:
      num(t, Nat.add(Nat.mul(n, 10n), Nat.sub(U32.to_nat(c), 48n)))

def size(r: File & Result<&1, &1, U32 & String, U32>) -> File & U32:
  (f, res) = r
  (f, Result.default(&1, &1, U32 & String, U32, res, 0))

def lines(r: U.Lines & List<&2, String>) -> List<&2, String>:
  (_, ls) = r
  ls

def run(rb: File & Result<&1, &1, U32 & String, List<&2, U32>>, +th: String, +reps: Nat, +t0: Nat) -> IO(Unit):
  (f, res) = rb
  do IO<Unit>:
    File.close(f)
    bs : List<&2, U32> <- IO.pass(List<&2, U32>, res)
    +ld : ST.Loaded = ST.Store.load(lines(U.Lines.feed(U.Lines.new(), bs)), ST.Loaded{M.State.empty(), Nil{}})
    +now : Nat = 1792100000n
    +st : M.State = M.State.replay([M.TurnChanged{th, M.Running{}, now}], ST.Loaded.st(ld))
    +t1 : Nat <- IO.now()
    IO.print("loaded " ++ Nat.show(count(st)) ++ " entries, seq " ++ Nat.show(M.State.seq(st)) ++ " in " ++ Nat.show(Nat.sub(t1, t0)) ++ " ms")
    +line : String = delta()
    +o : J.Json = Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{})
    +s : B.Step = B.Hub.agent_line.vo(2n, st, now, th, line, o, "")
    +cs : List<&2, M.Change> = Step.cs(s)
    +os : List<&2, B.Out> = Step.os(s)
    +a0 : Nat <- IO.now()
    +x0 : Nat = rep.seq(reps, st)
    +a1 : Nat <- IO.now()
    per("seq (baseline)", reps, a0, a1, x0)
    +x1 : Nat = rep.vo(reps, st, now, th, line, o)
    +a2 : Nat <- IO.now()
    per("agent_line.vo 2 (delta, whole)", reps, a1, a2, x1)
    +x2 : Nat = rep.old(reps, st, now, th, line)
    +a3 : Nat <- IO.now()
    per("agent_line old rule (delta)", reps, a2, a3, x2)
    +x3 : Nat = rep.inner(reps, st, now, th, o)
    +a4 : Nat <- IO.now()
    per("Claude.line.v.k (inner)", reps, a3, a4, x3)
    +x4 : Nat = rep.ended(reps, st, now, th, s)
    +a5 : Nat <- IO.now()
    per("Ended.out", reps, a4, a5, x4)
    +x5 : Nat = rep.drain(reps, st, now, th, s)
    +a6 : Nat <- IO.now()
    per("Queue.drain", reps, a5, a6, x5)
    +x6 : Nat = rep.finish(reps, st, now, th, s)
    +a7 : Nat <- IO.now()
    per("Task.finish.w", reps, a6, a7, x6)
    +x7 : Nat = rep.tell(reps, st, now, th, s)
    +a8 : Nat <- IO.now()
    per("Launch.tell", reps, a7, a8, x7)
    +x8 : Nat = rep.wake(reps, st, s)
    +a9 : Nat <- IO.now()
    per("step: Wake.after", reps, a8, a9, x8)
    +x9 : Nat = rep.locks(reps, st, now, s)
    +b0 : Nat <- IO.now()
    per("step: Locks.after", reps, a9, b0, x9)
    +y0 : Nat = rep.alerts(reps, st, cs)
    +b1 : Nat <- IO.now()
    per("step: Alerts.changes", reps, b0, b1, y0)
    +y1 : Nat = rep.pend(reps, st, cs)
    +b2 : Nat <- IO.now()
    per("step: Pend.fix", reps, b1, b2, y1)
    +y2 : Nat = rep.outs(reps, cs, os)
    +b3 : Nat <- IO.now()
    per("step: Island/Alerts/Tools outs", reps, b2, b3, y2)
    +mline : String = message()
    +mo : J.Json = Maybe.default(&2, J.Json, J.Json.parse(mline), J.Null{})
    +ms : B.Step = B.Hub.agent_line.vo(2n, st, now, th, mline, mo, "")
    +y3 : Nat = rep.vo(reps, st, now, th, mline, mo)
    +b4 : Nat <- IO.now()
    per("agent_line.vo 2 (message)", reps, b3, b4, y3)
    +y4 : Nat = rep.locks(reps, st, now, ms)
    +b5 : Nat <- IO.now()
    per("message: Locks.after", reps, b4, b5, y4)
    +y5 : Nat = rep.alerts(reps, st, Step.cs(ms))
    +b6 : Nat <- IO.now()
    per("message: Alerts.changes", reps, b5, b6, y5)
    +y6 : Nat = rep.pend(reps, st, Step.cs(ms))
    +b7 : Nat <- IO.now()
    per("message: Pend.fix", reps, b6, b7, y6)
    +y7 : Nat = rep.fold(reps, st, Step.cs(ms))
    +b8 : Nat <- IO.now()
    per("message: fold + wire", reps, b7, b8, y7)
    +y8 : Nat = rep.wake(reps, st, ms)
    +b9 : Nat <- IO.now()
    per("message: Wake.after", reps, b8, b9, y8)
    +y9 : Nat = rep.busy(reps, st)
    +c0 : Nat <- IO.now()
    per("step: Hub.busy.set (Work.threads)", reps, b9, c0, y9)
    mcp(reps, st, now, th, "thread_list", "{}")
    mcp(reps, st, now, th, "thread_read", "{\"thread\":\"" ++ th ++ "\",\"limit\":40}")
    mcp(reps, st, now, th, "task_status", "{\"task\":\"k-none\"}")
    mcp(reps, st, now, th, "lock_list", "{}")
    mcp(reps, st, now, th, "work_list", "{}")
    mcp(reps, st, now, th, "wake_list", "{}")
    mcp(reps, st, now, th, "wake_on", "{\"kind\":\"thread\",\"target\":\"" ++ th ++ "\",\"events\":[\"end\"]}")
    mcp(reps, st, now, th, "lock_acquire", "{\"name\":\"x\",\"_lockReady\":true}")
    IO.print("message step: " ++ Nat.show(Step.size(ms)))

def go(r: File & Result<&1, &1, U32 & String, U32>, +file: String, +th: String, +reps: Nat, +t0: Nat) -> IO(Unit):
  (f, res) = r
  do IO<Unit>:
    rb : File & Result<&1, &1, U32 & String, List<&2, U32>> <- File.read_bytes(f, Result.default(&1, &1, U32 & String, U32, res, 1))
    run(rb, th, reps, t0)

def main() -> IO(Unit):
  do IO<Unit>:
    args0 : List<String> <- AV.Argv.args()
    +args : List<&2, String> = copy(args0)
    +file : String = arg(args, 0n, "/dev/null")
    +th : String = arg(args, 1n, "t1")
    +reps : Nat = num(arg(args, 2n, "20"), 0n)
    +t0 : Nat <- IO.now()
    f : File <- IO.try(File, File.open(file, "r"))
    r : File & Result<&1, &1, U32 & String, U32> <- File.size(f)
    IO.print("loading")
    go(r, file, th, reps, t0)
