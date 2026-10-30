import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/core/hub.bend as H
import ../src/core/mcp.bend as MC
import ../src/core/subs.bend as SB

# Watched work (hub.bend's Watch.*, the work_* MCP tools): a work_run is
# recorded before it is launched and its end wakes the thread once with
# the outcome; a timeout or a vanished process is never success; a cancel
# shows until confirmed; unwatching says it may still run; the work counts
# as the thread's work (thread_wait, the Subagents panel with its Stop); a
# restart watches open work again, a mirror never does.

def base() -> M.State:
  M.State.replay([M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"t1", "p1", "x", M.Local{}, "", "", "claude", "", 1n},
    M.TurnChanged{"t1", M.Running{}, 2n}], M.State.empty())

def call(+name: String, +args: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse("{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"" ++ name ++ "\",\"arguments\":" ++ args ++ "}}"), J.Null{})

def out.step(o: MC.McpOut) -> H.Step:
  match o:
    case MC.McpOut{_, st, _}:
      st

def out.text(o: MC.McpOut) -> String:
  match o:
    case MC.McpOut{r, _, _}:
      r

def tool(+s: M.State, +now: Nat, +name: String, +args: String) -> MC.McpOut:
  MC.Mcp.handle(s, now, "t1", J.Json.arr(Nil{}), "1", call(name, args))

def apply(+s: M.State, st: H.Step) -> M.State:
  M.State.replay(H.Queue.changes(st), s)

def awaits(os: List<&2, H.Out>) -> String:
  match os:
    case Nil{}:
      ""
    case H.Await{_, +id, +kind, +target, +ident, +deadline, _} <> t:
      "await " ++ id ++ " " ++ kind ++ " " ++ target ++ " " ++ ident ++ " " ++ Nat.show(deadline) ++ ";" ++ awaits(t)
    case H.WatchStop{_, +id, +kind, _, _} <> t:
      "stop " ++ id ++ " " ++ kind ++ ";" ++ awaits(t)
    case _ <> t:
      awaits(t)

def rows(+s: M.State) -> String:
  String.join(M.Work.watch(s, "t1"), "|")

def words(rs: List<&2, SB.SubRow>) -> String:
  match rs:
    case Nil{}:
      ""
    case +r <> t:
      SB.Subs.word(r) ++ ":" ++ SB.Subs.cancel(r) ++ ";" ++ words(t)

def last(es: List<&2, M.Entry>) -> String:
  match es:
    case M.Msg{_, _, _, +text, _} <> _:
      text
    case _ <> t:
      last(t)
    case Nil{}:
      ""

def main() -> IO(Unit):
  do IO<Unit>:
    +r1 : MC.McpOut = tool(base(), 10n, "work_run", "{\"command\":\"sleep 3; exit 4\",\"note\":\"slow build\",\"timeoutSec\":60}")
    +id : String = H.Watch.id(base(), "t1")
    C.Check.str("work_run answers its id", Bool.show(String.contains(out.text(r1), id)), "True")
    C.Check.str("the row is written, launching", Bool.show(String.contains(rows(apply(base(), out.step(r1))), id ++ "\tlaunching\t\tslow build\twatch\trun\t\\esleep 3; exit 4\t-\t70")), "True")
    C.Check.str("and the launch goes with it", awaits(H.Step.outs_of(out.step(r1))), "await " ++ id ++ " run sleep 3; exit 4 - 70;")
    +s1 : M.State = apply(base(), out.step(r1))
    C.Check.str("it is the thread's work", Nat.show(M.Work.busy(s1, "t1")), "1")
    C.Check.str("thread_wait is not done", Bool.show(MC.Mcp.done.w(M.State.replay([M.TurnChanged{"t1", M.Completed{}, 11n}], s1), "t1")), "False")
    C.Check.str("the panel shows it, with Stop", words(SB.Subs.rows(s1, Map.new(&2, String), "t1", 11n)), "launching:work-cancel;")
    +s2 : M.State = apply(s1, H.Watch.launched(s1, 11n, "t1", id))
    C.Check.str("launched: watching", Bool.show(String.contains(rows(s2), "\twatching\t")), "True")
    # the turn ends; the work ends after
    +s3 : M.State = M.State.replay([M.TurnChanged{"t1", M.Completed{}, 12n}], s2)
    +e1 : H.Step = H.Watch.end(s3, 20n, "t1", id, "exit 4", "exit code 4\noutput: /h/work/" ++ id ++ "/out\nits end:\nboom")
    +s4 : M.State = apply(s3, e1)
    C.Check.str("its end wakes the thread", Bool.show(H.Hub.running(H.Hub.thread(s4, "t1"))), "True")
    C.Check.str("with the outcome", Bool.show(String.contains(last(M.State.mine(s4, "t1")), "Watched work \"slow build\" ended: exit 4")) ++ Bool.show(String.contains(last(M.State.mine(s4, "t1")), "boom")), "TrueTrue")
    C.Check.str("once", Nat.show(List.length(&2, M.Change, H.Queue.changes(H.Watch.end(s4, 21n, "t1", id, "exit 4", "")))), "0")
    C.Check.str("the row is gone", rows(s4), "")
    # a timeout: the row stays, outcome unknown
    +t1 : M.State = apply(s3, H.Watch.end(s3, 80n, "t1", id, "timeout", ""))
    C.Check.str("timeout: told, never success", Bool.show(String.contains(last(M.State.mine(t1, "t1")), "timed out, still running, outcome unknown")), "True")
    C.Check.str("timeout: the row stays, unknown", M.Bg.status(rows(t1)), "unknown")
    C.Check.str("timeout: no Stop on it", words(SB.Subs.rows(t1, Map.new(&2, String), "t1", 11n)), "unknown:;")
    C.Check.str("timeout: final outcome still delivered", rows(apply(t1, H.Watch.end(t1, 90n, "t1", id, "exit 0", "finished"))), "")
    C.Check.str("timeout: restarted observer has no repeated deadline", awaits(H.Watch.rearm(t1)), "await " ++ id ++ " run sleep 3; exit 4 - 0;")
    C.Check.str("timeout: repeated deadline report suppressed", Nat.show(List.length(&2, M.Change, H.Queue.changes(H.Watch.end(t1, 81n, "t1", id, "timeout", "")))), "0")
    # gone without an outcome
    +g1 : M.State = apply(s3, H.Watch.end(s3, 30n, "t1", id, "gone", ""))
    C.Check.str("gone: outcome unknown", Bool.show(String.contains(last(M.State.mine(g1, "t1")), "outcome unknown")), "True")
    # cancel
    +c1 : MC.McpOut = tool(s3, 30n, "work_cancel", "{\"id\":\"" ++ id ++ "\"}")
    +cs : M.State = apply(s3, out.step(c1))
    C.Check.str("cancel: cancelling until confirmed", Bool.show(String.contains(rows(cs), "\tcancelling\t")) ++ " " ++ awaits(H.Step.outs_of(out.step(c1))), "True stop " ++ id ++ " run;")
    C.Check.str("cancel: then cancelled", Bool.show(String.contains(last(M.State.mine(apply(cs, H.Watch.end(cs, 31n, "t1", id, "gone", "")), "t1")), "ended: cancelled")), "True")
    C.Check.str("cancel: nothing to cancel twice", Bool.show(String.contains(out.text(tool(apply(cs, H.Watch.end(cs, 31n, "t1", id, "gone", "")), 32n, "work_cancel", "{\"id\":\"" ++ id ++ "\"}")), "no such work")), "True")
    # unwatch
    +u1 : MC.McpOut = tool(s3, 30n, "work_unwatch", "{\"id\":\"" ++ id ++ "\"}")
    C.Check.str("unwatch: may still run", Bool.show(String.contains(out.text(u1), "may still run")) ++ " " ++ rows(apply(s3, out.step(u1))) ++ awaits(H.Step.outs_of(out.step(u1))), "True stop " ++ id ++ " unwatch;")
    # a file watched: its token
    +f1 : MC.McpOut = tool(base(), 10n, "work_watch", "{\"kind\":\"file\",\"target\":\"/tmp/done.txt\",\"note\":\"remote job\"}")
    C.Check.str("work_watch file: the token is its ident", awaits(H.Step.outs_of(out.step(f1))), "await " ++ id ++ " file /tmp/done.txt " ++ id ++ " 1210;")
    C.Check.str("work_watch: a kind it knows", Bool.show(String.contains(out.text(tool(base(), 10n, "work_watch", "{\"kind\":\"socket\",\"target\":\"x\"}")), "kind must be")), "True")
    C.Check.str("work_list", Bool.show(String.contains(out.text(tool(s1, 12n, "work_list", "{}")), "slow build")), "True")
    # a restart watches it again; a mirror never does
    C.Check.str("restart: watched again", awaits(H.Step.outs_of(H.Hub.recover(s2, 40n))), "await " ++ id ++ " run sleep 3; exit 4 - 70;")
    C.Check.str("a mirror never watches", awaits(H.Watch.rearm(H.Watch.demo.at(False{}, "p1~t1"))), "")
