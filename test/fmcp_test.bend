import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/codec.bend as K
import ../src/core/json.bend as J
import ../src/core/far.bend as FR
import ../src/core/hub.bend as B
import ../src/core/mcp.bend as MC
import ../src/core/fmcp.bend as F

# Agents across machines (fmcp.bend): a tool call naming another machine
# becomes a signed request there (the plan), the owning hub runs it only
# for what it shares (serve), and what comes back is named by its link.

def numbered(cs: List<&2, M.Change>, +n: Nat) -> List<&2, FR.Item>:
  match cs:
    case Nil{}:
      Nil{}
    case c <> t:
      FR.Item{n, K.Change.wire(c)} <> numbered(t, 1n+n)

# the owning hub's state: a project with an idle thread
def owner() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "board", "/b", 1n}, M.ThreadCreated{"pt", "p", "layout", M.Local{}, "", "", "claude", "", 2n},
    M.ThreadCreated{"bt1", "", "Kit", M.Local{}, "", "", "claude", "", 3n}], M.State.empty())

# its mirror here, named "box" behind the link p1
def mirror() -> FR.In:
  FR.In.put(FR.In.new("p1", "box"), 0n, 2n, numbered([M.ProjectCreated{"p", "board", "/b", 1n}, M.ThreadCreated{"pt", "p", "layout", M.Local{}, "", "", "claude", "", 2n}], 1n))

def asked(+tool: String, +a: List<&2, J.KV>) -> J.Json:
  J.Json.obj([J.KV{"id", J.Num{"0"}}, J.KV{"m", J.Str{"mcp.call"}},
    J.KV{"p", J.Json.obj([J.KV{"tool", J.Str{tool}}, J.KV{"from", J.Str{"p1~t0"}}, J.KV{"args", J.Json.obj(a)}])}])

def said(st: B.Step) -> String:
  match st:
    case B.Step{_, B.Send{_, +t} <> _}:
      t
    case _:
      "no reply"

def changed(st: B.Step) -> Nat:
  match st:
    case B.Step{cs, _}:
      List.length(&2, M.Change, cs)

def call(+tool: String, +a: List<&2, J.KV>) -> J.Json:
  J.Json.obj([J.KV{"method", J.Str{"tools/call"}}, J.KV{"params", J.Json.obj([J.KV{"name", J.Str{tool}}, J.KV{"arguments", J.Json.obj(a)}])}])

def plan(+tool: String, +a: List<&2, J.KV>, ins: List<&2, FR.In>) -> F.Plan:
  F.Fm.plan.o(call(tool, a), M.State.empty(), ins, "t0")

def rb(r: MC.Res) -> J.Json:
  match r:
    case MC.Res{_, b, _}:
      b

def body(p: F.Plan) -> String:
  match p:
    case F.PFar{l, +b}:
      l ++ " " ++ b
    case F.PAnswer{r}:
      "answer " ++ J.Json.show(rb(r))
    case F.PHere{}:
      "here"
    case F.PLater{l, n, t, _}:
      "later " ++ l ++ " " ++ n ++ " " ++ t

def main() -> IO(Unit):
  do IO<Unit>:
    # the owning hub
    +s : M.State = owner()
    C.Check.str("owner lists its threads (not a bot's)", said(F.Fm.serve(s, 100n, 1, asked("thread_list", Nil{}))),
      "{\"t\":\"reply\",\"id\":0,\"ok\":true,\"mcp\":{\"ok\":true,\"body\":{\"threads\":[{\"id\":\"pt\",\"project\":\"p\",\"title\":\"layout\",\"provider\":\"claude\",\"model\":\"\",\"turn\":\"idle\",\"activity\":2}]}}}")
    +sd : B.Step = F.Fm.serve(s, 100n, 1, asked("thread_send", [J.KV{"threadId", J.Str{"pt"}}, J.KV{"text", J.Str{"hello from alpha"}}]))
    C.Check.str("a send to a shared thread is stored", Bool.show(String.contains(said(sd), "\"ok\":true,\"body\":{\"threadId\":\"pt\",\"state\":\"sent\"")) ++ " " ++ Bool.show(Nat.is_gt(changed(sd), 0n)), "True True")
    +un : B.Step = F.Fm.serve(s, 100n, 1, asked("thread_send", [J.KV{"threadId", J.Str{"nope"}}, J.KV{"text", J.Str{"x"}}]))
    C.Check.str("a send to a thread not here is refused, nothing changes", Bool.show(String.contains(said(un), "\"mcp\":{\"ok\":false")) ++ " " ++ Nat.show(changed(un)), "True 0")
    C.Check.str("a thread's turn read for waiting", Bool.show(String.contains(said(F.Fm.serve(s, 100n, 1, asked("thread_wait_now", [J.KV{"threadId", J.Str{"pt"}}]))), "\"done\":true")), "True")
    +ln : B.Step = F.Fm.serve(s, 100n, 1, asked("thread_launch", [J.KV{"project", J.Str{"Board"}}, J.KV{"prompt", J.Str{"route the clock"}}, J.KV{"title", J.Str{"clock"}}]))
    C.Check.str("a launch in a project by name makes its thread", Bool.show(String.contains(said(ln), "\"state\":\"started\"")) ++ " " ++ Bool.show(Nat.is_gt(changed(ln), 1n)), "True True")
    C.Check.str("a launch in no project is refused", Bool.show(String.contains(said(F.Fm.serve(s, 100n, 1, asked("thread_launch", [J.KV{"project", J.Str{"nope"}}, J.KV{"prompt", J.Str{"x"}}]))), "\"mcp\":{\"ok\":false")), "True")
    C.Check.str("projects", said(F.Fm.serve(s, 100n, 1, asked("project_list", Nil{}))),
      "{\"t\":\"reply\",\"id\":0,\"ok\":true,\"mcp\":{\"ok\":true,\"body\":{\"projects\":[{\"id\":\"p\",\"name\":\"board\",\"folder\":\"/b\"}]}}}")
    C.Check.str("other tools are not taken", Bool.show(F.Fm.permit(s, asked("work_run", Nil{}))) ++ Bool.show(F.Fm.permit(s, asked("thread_send", Nil{}))), "FalseTrue")
    # the calling hub
    +ins : List<&2, FR.In> = [mirror()]
    C.Check.str("a local thread stays", body(plan("thread_send", [J.KV{"threadId", J.Str{"t1"}}], ins)), "here")
    C.Check.str("a thread elsewhere goes to its link in its own id", body(plan("thread_send", [J.KV{"threadId", J.Str{"p1~pt"}}, J.KV{"text", J.Str{"hi"}}], ins)),
      "p1 {\"id\":0,\"m\":\"mcp.call\",\"p\":{\"tool\":\"thread_send\",\"from\":\"p1~t0\",\"args\":{\"threadId\":\"pt\",\"text\":\"hi\"}}}")
    C.Check.str("a machine by name", body(plan("thread_list", [J.KV{"machine", J.Str{"Box"}}], ins)),
      "p1 {\"id\":0,\"m\":\"mcp.call\",\"p\":{\"tool\":\"thread_list\",\"from\":\"p1~t0\",\"args\":{\"machine\":\"Box\"}}}")
    C.Check.str("a project elsewhere by its id", body(plan("thread_launch", [J.KV{"project", J.Str{"p1~p"}}, J.KV{"prompt", J.Str{"go"}}], ins)),
      "p1 {\"id\":0,\"m\":\"mcp.call\",\"p\":{\"tool\":\"thread_launch\",\"from\":\"p1~t0\",\"args\":{\"project\":\"p\",\"prompt\":\"go\"}}}")
    C.Check.str("a machine not linked here is an error, nothing sent", body(plan("thread_list", [J.KV{"machine", J.Str{"gamma"}}], ins)),
      "answer {\"error\":\"no machine of yours called gamma is linked here (machine_list shows them)\"}")
    C.Check.str("a send to a machine away waits for it (the far queue)", body(plan("thread_send", [J.KV{"threadId", J.Str{"p1~pt"}}], [FR.In.away.set(mirror(), True{})])),
      "later p1 box thread_send")
    C.Check.str("an interrupt to a machine away is still an error", body(plan("thread_interrupt", [J.KV{"threadId", J.Str{"p1~pt"}}], [FR.In.away.set(mirror(), True{})])),
      "answer {\"error\":\"the machine box is away; try again when it is back\"}")
    C.Check.str("machine_list names the machines and their projects", body(plan("machine_list", Nil{}, ins)),
      "answer {\"machines\":[{\"machine\":\"this\",\"state\":\"here\",\"projects\":[]},{\"machine\":\"box\",\"link\":\"p1\",\"state\":\"here\",\"projects\":[{\"id\":\"p1~p\",\"name\":\"board\",\"folder\":\"/b\"}]}],\"note\":\"Name a machine in thread_list and thread_launch (`machine`), or use the ids shown here (<link>~<id>) with thread_send, thread_read, thread_wait and thread_interrupt.\"}")
    # plane_read: this machine's plane here, another machine's from its owner
    C.Check.str("plane_read without a machine is read here", body(plan("plane_read", Nil{}, ins)), "here")
    C.Check.str("plane_read of a machine asks it, with its link and name for the ids", body(plan("plane_read", [J.KV{"machine", J.Str{"Box"}}, J.KV{"idle", J.Flag{True{}}}], ins)),
      "p1 {\"id\":0,\"m\":\"mcp.call\",\"p\":{\"tool\":\"plane_read\",\"from\":\"p1~t0\",\"args\":{\"as\":\"p1\",\"name\":\"box\",\"machine\":\"Box\",\"idle\":true}}}")
    C.Check.str("plane_read of a machine away is an error", body(plan("plane_read", [J.KV{"machine", J.Str{"Box"}}], [FR.In.away.set(mirror(), True{})])),
      "answer {\"error\":\"the machine box is away; try again when it is back\"}")
    C.Check.str("the owner answers plane_read for its machines", Bool.show(F.Fm.permit(s, asked("plane_read", Nil{}))), "True")
    +pr : String = said(F.Fm.serve(s, 100n, 1, asked("plane_read", [J.KV{"as", J.Str{"p1"}}, J.KV{"name", J.Str{"box"}}])))
    C.Check.str("... in the caller's ids", pr, "{\"t\":\"reply\",\"id\":0,\"ok\":true,\"mcp\":{\"ok\":true,\"body\":{\"text\":\"Status plane of box\\nNothing needs you and nothing is running.\\nIdle in the last day: 1 (plane_read with idle: true lists them).\\n\"}}}")
    +pi : String = said(F.Fm.serve(s, 100n, 1, asked("plane_read", [J.KV{"as", J.Str{"p1"}}, J.KV{"idle", J.Flag{True{}}}])))
    C.Check.str("... with idle threads and the link's ids", Bool.show(String.contains(pi, "- p1~pt ")) ++ Bool.show(String.contains(pi, " layout")), "TrueTrue")
    # the answer, in the caller's ids
    +reply : String = said(F.Fm.serve(s, 100n, 1, asked("thread_list", Nil{})))
    C.Check.str("an answer's ids carry the link", Bool.show(String.contains(F.Fm.answer(J.Num{"4"}, "p1", 200n, reply), "\\\"id\\\":\\\"p1~pt\\\"")) ++ Bool.show(String.contains(F.Fm.answer(J.Num{"4"}, "p1", 200n, reply), "\"id\":4")), "TrueTrue")
    C.Check.str("a refusal is an error result", Bool.show(String.contains(F.Fm.answer(J.Num{"4"}, "p1", 403n, "not something this machine shares"), "\"isError\":true")), "True")
    C.Check.str("no answer says away", Bool.show(String.contains(F.Fm.answer(J.Num{"4"}, "p1", 0n, ""), "away or did not answer")), "True")
    C.Check.str("the prompt lists machines", F.Fm.prompt(ins) ++ "|" ++ F.Fm.prompt(Nil{}) ++ "|" ++ F.Fm.prompt([FR.In.away.set(mirror(), True{})]),
      "<machines>The person's other machines linked to this one: box. machine_list shows their projects and ids.</machines>||<machines>The person's other machines linked to this one: box (away). machine_list shows their projects and ids.</machines>")
    # /give elsewhere
    +here : M.State = M.State.replay([M.ProjectCreated{"q", "mine", "/m", 1n}, M.ThreadCreated{"t0", "q", "chat", M.Local{}, "", "", "claude", "", 2n}], M.State.empty())
    +gv : J.Json = J.Json.obj([J.KV{"m", J.Str{"bots.give"}}, J.KV{"p", J.Json.obj([J.KV{"thread", J.Str{"t0"}}, J.KV{"text", J.Str{"/give >box:layout please"}}])}])
    C.Check.str("/give to a thread elsewhere hands it over queued", Nat.show(List.length(&2, F.Plan, F.Fm.give(here, [mirror()], gv, 100n))) ++ " "
      ++ Bool.show(String.contains(body(Maybe.default(&2, F.Plan, List.head(&2, F.Plan, F.Fm.give(here, [mirror()], gv, 100n)), F.PHere{})), "\"threadId\":\"pt\""))
      ++ Bool.show(String.contains(body(Maybe.default(&2, F.Plan, List.head(&2, F.Plan, F.Fm.give(here, [mirror()], gv, 100n)), F.PHere{})), "\"tool\":\"thread_handoff\"")), "1 TrueTrue")
    +gp : J.Json = J.Json.obj([J.KV{"m", J.Str{"bots.give"}}, J.KV{"p", J.Json.obj([J.KV{"thread", J.Str{"t0"}}, J.KV{"text", J.Str{"/give %box:board"}}])}])
    C.Check.str("/give to a project elsewhere hands it to a new thread there", Bool.show(String.contains(body(Maybe.default(&2, F.Plan, List.head(&2, F.Plan, F.Fm.give(here, [mirror()], gp, 100n)), F.PHere{})), "\"project\":\"board\"")), "True")
    C.Check.str("/give naming no machine of yours is an error", F.Fm.plan.name(Maybe.default(&2, F.Plan, List.head(&2, F.Plan, F.Fm.give(here, Nil{}, gv, 100n)), F.PHere{})), "answer")
    +gf : J.Json = J.Json.obj([J.KV{"m", J.Str{"bots.give"}}, J.KV{"p", J.Json.obj([J.KV{"thread", J.Str{"p1~pt"}}, J.KV{"text", J.Str{"/give >box:layout"}}])}])
    C.Check.str("/give from a thread elsewhere is refused here", F.Fm.plan.name(Maybe.default(&2, F.Plan, List.head(&2, F.Plan, F.Fm.give(here, [mirror()], gf, 100n)), F.PHere{})), "answer")
    C.Check.str("a machine word on its own line is left out too", F.Fm.give.text("/give note\n%box:board\t%mine"), "/give note\n\t%mine")
    C.Check.str("the hub's own /give reads no machine words", F.Fm.give.text("/give >box:layout %mine %box:board please"), "/give %mine please")
    C.Check.str("a handoff queues on a shared thread", Nat.show(changed(F.Fm.serve(s, 100n, 1, asked("thread_handoff", [J.KV{"threadId", J.Str{"pt"}}, J.KV{"hand", J.Str{"ctx"}}, J.KV{"key", J.Str{"k1"}}])))), "2")
    C.Check.str("a handoff to a project makes a thread", Nat.show(changed(F.Fm.serve(s, 100n, 1, asked("thread_handoff", [J.KV{"project", J.Str{"board"}}, J.KV{"hand", J.Str{"ctx"}}, J.KV{"key", J.Str{"k2"}}])))), "2")
    C.Check.str("a handoff to nothing shared changes nothing", Nat.show(changed(F.Fm.serve(s, 100n, 1, asked("thread_handoff", [J.KV{"threadId", J.Str{"nope"}}, J.KV{"hand", J.Str{"ctx"}}])))), "0")
    C.Check.str("thread_read elsewhere asks its machine", F.Fm.plan.name(plan("thread_read", [J.KV{"threadId", J.Str{"p1~pt"}}], ins)), "far")
    C.Check.str("thread_read on a machine away reads the mirror", F.Fm.plan.name(plan("thread_read", [J.KV{"threadId", J.Str{"p1~pt"}}], [FR.In.away.set(mirror(), True{})])), "here")
    C.Check.str("a sender of this machine's comes back as its own", FR.Fr.sent("p1", M.Link.id("p1~ta", "m5")) ++ " " ++ FR.Fr.sent("p1", M.Link.id("tb", "m6")) ++ " " ++ FR.Fr.sent("p1", "m7"), "t:ta:m5 t:p1~tb:m6 m7")
