import Base
import ./json.bend as J
import ./model.bend as M
import ./codec.bend as C
import ./hub.bend as B
import ./far.bend as FR
import ./refs.bend as RF
import ./mcp.bend as MC
import ./xref.bend as XR
import ./handoff.bend as HD
import ./bothub.bend as BH
import ./plane.bend as PLN

# Agents across machines: the thread tools reaching another machine
# ------------------------------------------------------------------
# An agent's thread_send, thread_wait, thread_interrupt, thread_update,
# thread_list, thread_launch and project_list may name a machine of the
# owner's (a thread or project id elsewhere, "<link>~<id>", or a `machine`
# argument). The hub that hears the call does not run it: it sends it to
# the owning hub as the signed request `mcp.call` (POST /far/rpc, the path
# clients use for threads elsewhere) and answers the agent with what comes
# back, its ids made `<link>~<id>` so the agent can use them in its next
# call. The owner runs the same defs as for its own agents (mcp.bend), but
# only on what it shares with the owner's machines (Fr.thread.shared) and
# only the tools in Fm.names: a request from a machine that is not the
# owner's never gets this far (the route checks Far.own), and one for a
# thread this machine does not share is answered with an error and no
# change. thread_read asks the owner too (what it answers is current even
# before its push reaches the mirror; the mirror's (xref.bend) while the
# machine is away), thread_wait polls its owner through thread_wait_now,
# and thread_handoff is /give's (below; no agent calls it).

# the tools a remote agent may call here (thread_wait is its poll)
def Fm.names() -> List<&2, String>:
  ["thread_send", "thread_wait_now", "thread_interrupt", "thread_update", "thread_list", "thread_launch", "project_list", "machine_list", "thread_read", "thread_handoff", "plane_read", "launch_report"]

# a request from another machine: `mcp.call` of one of the tools the owner's
# machines may call here (machine_list is each hub's own answer)
def Fm.is(+o: J.Json) -> Bool:
  String.eq(J.Json.get_str(o, "m", ""), "mcp.call")

def Fm.remote.k(+i: Nat) -> Bool:
  Bool.or(Nat.is_lt(i, 7n), Bool.and(Nat.is_ge(i, 8n), Nat.is_lt(i, 12n)))

def Fm.remote(+tool: String) -> Bool:
  Fm.remote.k(C.Codec.pos(Fm.names(), tool))

def Fm.allowed(+o: J.Json) -> Bool:
  Bool.and(Fm.is(o), Fm.remote(J.Json.get_str(J.Json.get_obj(o, "p"), "tool", "")))

# what a hub takes at /far/rpc: a client's request about a shared thing, or
# a remote agent's tool call
def Fm.permit(+s: M.State, +o: J.Json) -> Bool:
  Bool.or(FR.Fr.allowed(s, o), Fm.allowed(o))

# On the owning hub
# -----------------

def Fm.say(ok: Bool, r: MC.Res) -> MC.Res:
  match ok:
    case True{}:
      r
    case False{}:
      MC.Res.fail("that thread is not shared with you (it is not on this machine)")

def Fm.keep(ok: Bool, j: J.Json, rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match ok:
    case True{}:
      j <> rest
    case False{}:
      rest

def Fm.projects(ps: List<&2, M.Project>) -> List<&2, J.Json>:
  match ps:
    case Nil{}:
      Nil{}
    case M.Project{+i, +t, +r, _, rm} <> rest:
      Fm.keep(Bool.not(rm), J.Json.obj([J.KV{"id", J.Str{i}}, J.KV{"name", J.Str{t}}, J.KV{"folder", J.Str{r}}]), Fm.projects(rest))

def Fm.proj.rows(+s: M.State) -> List<&2, J.Json>:
  match s:
    case M.State{ps, _, _, _, _, _, _, _, _, _, _}:
      Fm.projects(Map.values(&2, M.Project, ps))

def Fm.list.k(all: Bool, +s: M.State, +pid: String, ts: List<&2, M.Thread>) -> MC.Res:
  match all:
    case True{}:
      MC.Res.ok(J.Json.obj([J.KV{"threads", J.Json.arr(List.take(&2, J.Json, MC.Mcp.threads(ts, "*"), 200n))}]))
    case False{}:
      Bool.pick(MC.Res, String.is_empty(pid), MC.Res.fail("no such project here (project_list shows them)"),
        MC.Res.ok(J.Json.obj([J.KV{"threads", J.Json.arr(List.take(&2, J.Json, MC.Mcp.threads(ts, pid), 200n))}])))

# its threads, optionally those of the project the name or id picks
def Fm.list(+s: M.State, +project: String) -> MC.Res:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}:
      Fm.list.k(String.is_empty(String.trim(project)), s, MC.Mcp.project.find(s, project), Map.values(&2, M.Thread, ts))

# /give from another machine: the conversation queued ahead of the
# thread's next message, as a local /give queues it (bothub.bend's
# Give.thread.k), or a new thread in a project with it queued
# (Give.project.k); the project named by id, name or slug
def Fm.pslug.go(ps: List<&2, M.Project>, +w: String) -> String:
  match ps:
    case Nil{}:
      ""
    case M.Project{+i, +t, _, _, rm} <> rest:
      Bool.pick(String, Bool.and(Bool.not(rm), String.eq(RF.Slug.of(t), w)), i, Fm.pslug.go(rest, w))

def Fm.pslug(+s: M.State, +name: String) -> String:
  match s:
    case M.State{ps, _, _, _, _, _, _, _, _, _, _}:
      Fm.pslug.go(Map.values(&2, M.Project, ps), RF.Slug.of(name))

def Fm.pick.p(+s: M.State, +name: String) -> String:
  Bool.pick(String, String.is_empty(MC.Mcp.project.find(s, name)), Fm.pslug(s, name), MC.Mcp.project.find(s, name))

def Fm.hand.key(+from: String, +key: String) -> String:
  HD.Hand.prefix() ++ "give:" ++ from ++ ":" ++ key

def Fm.hand.thread(ok: Bool, +s: M.State, +now: Nat, +from: String, +to: String, +a: J.Json) -> MC.Res:
  match ok:
    case True{}:
      MC.Res{True{}, J.Json.obj([J.KV{"threadId", J.Str{to}}, J.KV{"state", J.Str{"handed"}}]),
        B.Step{[M.TurnQueued{to, Fm.hand.key(from, J.Json.get_str(a, "key", "")), J.Json.get_str(a, "hand", ""), now},
          M.ActivityLogged{to, "k" ++ J.Json.get_str(a, "key", "") ++ ".in", "info", "fork", "\"" ++ J.Json.get_str(a, "title", "") ++ "\" was handed over from another machine; it goes with the next message", now}], Nil{}}}
    case False{}:
      MC.Res.fail("that thread is not shared with you (it is not on this machine)")

def Fm.hand.project(ok: Bool, +s: M.State, +now: Nat, +from: String, +pid: String, +a: J.Json) -> MC.Res:
  match ok:
    case True{}:
      +th = B.Hub.id("t", now, B.Hub.seq(s))
      +mode = M.Setting.get(s, "thread.env", "auto")
      MC.Res{True{}, J.Json.obj([J.KV{"threadId", J.Str{th}}, J.KV{"state", J.Str{"handed"}}]),
        B.Step{[M.ThreadCreated{th, pid, J.Json.get_str(a, "title", "Handed over"), B.Hub.new.env(M.Scratch.is(pid), mode), "", B.Hub.new.wt(M.Scratch.is(pid), B.Hub.project.root(B.Hub.project(s, pid)), th),
            J.Json.get_str(a, "provider", "claude"), J.Json.get_str(a, "model", ""), now},
          M.TurnQueued{th, Fm.hand.key(from, J.Json.get_str(a, "key", "")), J.Json.get_str(a, "hand", ""), now}],
          B.Hub.new.outs(M.Scratch.is(pid), th, B.Hub.project.root(B.Hub.project(s, pid)), mode)}}
    case False{}:
      MC.Res.fail("no project called " ++ J.Json.get_str(a, "project", "") ++ " is shared on this machine")

def Fm.hand(+s: M.State, +now: Nat, +from: String, +a: J.Json) -> MC.Res:
  Bool.pick(MC.Res, String.is_empty(J.Json.get_str(a, "threadId", "")),
    Fm.hand.project(FR.Fr.project.shared(s, Fm.pick.p(s, J.Json.get_str(a, "project", ""))), s, now, from, Fm.pick.p(s, J.Json.get_str(a, "project", "")), a),
    Fm.hand.thread(FR.Fr.thread.shared(s, J.Json.get_str(a, "threadId", "")), s, now, from, J.Json.get_str(a, "threadId", ""), a))

# (the machine the plane is of: what it answers to the one that asked)
def Fm.plane.name(+name: String) -> String:
  Bool.pick(String, String.is_empty(name), "that machine", name)

# A launched thread's report from the machine it runs on, for its launcher
# here (hub.bend's Lr.far.*): posted once per key, so a line the other
# machine sends again posts nothing more (law launch_far_once); for the
# uberbot's own thread it is filed in its Mailroom (Operational.inbox)
def Fm.report.k(here: Bool, had: Bool, +s: M.State, +now: Nat, +from: String, +to: String, +key: String, +text: String) -> MC.Res:
  match here had:
    case False{} _:
      MC.Res.fail("no thread " ++ to ++ " here")
    case True{} True{}:
      MC.Res.ok(J.Json.obj([J.KV{"threadId", J.Str{to}}, J.KV{"state", J.Str{"had it"}}]))
    case True{} False{}:
      MC.Res{True{}, J.Json.obj([J.KV{"threadId", J.Str{to}}, J.KV{"state", J.Str{"sent"}}]), B.Operational.inbox(s, now, "launch", from, to, M.User{}, text, B.Lr.msg(from, key), Nil{})}

def Fm.report(+s: M.State, +now: Nat, +from: String, +to: String, +key: String, +text: String) -> MC.Res:
  Fm.report.k(B.Lr.exists(s, to), B.Hub.has_msg(M.State.mine(s, to), B.Lr.msg(from, key)), s, now, from, to, key, text)

# the same report taken twice: how many messages reach t0
def Fm.report.again(r: MC.Res) -> Nat:
  match r:
    case MC.Res{_, _, B.Step{+cs, _}}:
      B.Lr.count(cs, "t0")

def Fm.report.twice.k(r: MC.Res, +s: M.State) -> Nat:
  match r:
    case MC.Res{_, _, B.Step{+cs, _}}:
      Nat.add(B.Lr.count(cs, "t0"), Fm.report.again(Fm.report(M.State.replay(cs, s), 10n, "L~t1", "t0", "k1", "x")))

def Fm.report.twice() -> Nat:
  +s = B.Lr.demo(True{})
  Fm.report.twice.k(Fm.report(s, 9n, "L~t1", "t0", "k1", "x"), s)

def Fm.call.k(i: Nat, +s: M.State, +now: Nat, +from: String, +a: J.Json, +id: String, +ok: Bool) -> MC.Res:
  match i:
    case 0n:
      Fm.say(Bool.or(ok, B.Lr.may(s, from, id)), MC.Mcp.send.k(Bool.or(ok, B.Lr.may(s, from, id)), s, now, from, id, J.Json.get_str(a, "text", ""), J.Json.get_str(a, "mode", "")))
    case 1n:
      Fm.say(ok, MC.Mcp.wait.k(ok, s, id))
    case 2n:
      Fm.say(ok, MC.Mcp.interrupt.f(ok, B.Hub.running(B.Hub.thread(s, id)), s, now, from, id))
    case 3n:
      Fm.say(ok, MC.Mcp.update.k(ok, String.is_empty(String.trim(J.Json.get_str(a, "title", ""))), s, now, id, String.trim(J.Json.get_str(a, "title", ""))))
    case 4n:
      Fm.list(s, J.Json.get_str(a, "project", ""))
    case 5n:
      MC.Mcp.launch.by(s, now, "", from, a)
    case 6n:
      MC.Res.ok(J.Json.obj([J.KV{"projects", J.Json.arr(Fm.proj.rows(s))}]))
    case 8n:
      Fm.say(ok, MC.Mcp.read.k(ok, s, id, M.Switch.num(J.Json.as_raw(J.Json.get(a, "limit"), "40"), 40n)))
    case 9n:
      Fm.hand(s, now, from, a)
    case 11n:
      Fm.report(s, now, from, J.Json.get_str(a, "threadId", ""), J.Json.get_str(a, "key", ""), J.Json.get_str(a, "text", ""))
    case 10n:
      MC.Res.ok(J.Json.obj([J.KV{"text", J.Str{PLN.Plane.read(s, now, J.Json.get_str(a, "as", ""), Fm.plane.name(J.Json.get_str(a, "name", "")), J.Json.get_bool(a, "idle", False{}))}}]))
    case _:
      MC.Res.fail("unknown tool")

def Fm.call(+s: M.State, +now: Nat, +tool: String, +from: String, +a: J.Json) -> MC.Res:
  Fm.call.k(C.Codec.pos(Fm.names(), tool), s, now, from, a, J.Json.get_str(a, "threadId", ""), FR.Fr.thread.shared(s, J.Json.get_str(a, "threadId", "")))

def Fm.serve.k(r: MC.Res, +client: U32) -> B.Step:
  match r:
    case MC.Res{ok, body, B.Step{cs, os}}:
      B.Step{cs, B.Send{client, J.Json.show(J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{"0"}}, J.KV{"ok", J.Flag{True{}}},
        J.KV{"mcp", J.Json.obj([J.KV{"ok", J.Flag{ok}}, J.KV{"body", body}])}]))} <> os}

# the step answering a remote agent's call (its reply first, which is the
# one the route sends back)
def Fm.serve(+s: M.State, +now: Nat, +client: U32, +o: J.Json) -> B.Step:
  Fm.serve.k(Fm.call(s, now, J.Json.get_str(J.Json.get_obj(o, "p"), "tool", ""), J.Json.get_str(J.Json.get_obj(o, "p"), "from", ""),
    J.Json.get_obj(J.Json.get_obj(o, "p"), "args")), client)

# On the calling hub
# ------------------

# ids that come back from a machine, named as here
def Fm.id.key(+k: String) -> Bool:
  Bool.or(String.eq(k, "id"), Bool.or(String.eq(k, "threadId"), String.eq(k, "project")))

def Fm.far.json(j: J.Json, +link: String) -> J.Json:
  match j:
    case J.Obj{fs}:
      J.Obj{Fm.far.json(fs, link)}
    case J.Arr{xs}:
      J.Arr{Fm.far.json(xs, link)}
    case J.Item{h, t}:
      J.Item{Fm.far.json(h, link), Fm.far.json(t, link)}
    case J.Field{+k, J.Str{+v}, t}:
      J.Field{k, J.Str{Bool.pick(String, Fm.id.key(k), FR.Fr.opt(link, v), v)}, Fm.far.json(t, link)}
    case J.Field{k, v, t}:
      J.Field{k, Fm.far.json(v, link), Fm.far.json(t, link)}
    case x:
      x

# the call's arguments with the ids of that machine made its own
def Fm.args.local(fs: J.Json, +link: String) -> J.Json:
  match fs:
    case J.Field{+k, J.Str{+v}, t}:
      J.Field{k, J.Str{Bool.pick(String, Bool.and(Bool.or(String.eq(k, "threadId"), String.eq(k, "project")), String.eq(FR.Fr.link(v), link)), FR.Fr.local(v), v)}, Fm.args.local(t, link)}
    case J.Field{k, v, t}:
      J.Field{k, v, Fm.args.local(t, link)}
    case x:
      x

def Fm.request(+link: String, +tool: String, +from: String, a: J.Json) -> String:
  J.Json.show(J.Json.obj([J.KV{"id", J.Num{"0"}}, J.KV{"m", J.Str{"mcp.call"}},
    J.KV{"p", J.Json.obj([J.KV{"tool", J.Str{tool}}, J.KV{"from", J.Str{from}}, J.KV{"args", J.Obj{Fm.args.local(FR.Fr.obj.fields(a), link)}}])}]))

# what the hub does with a tool call
type Plan is Data:
  PHere{}
  PAnswer{r: MC.Res}
  PFar{link: String, body: String}
  # for a machine that is away: queued (hub.bend's Fq.*), sent when it is back
  PLater{link: String, name: String, tool: String, body: String}

# what the agent is told when its call waits for a machine
# (its step keeps the call in the queue: B.Fq.queue)
def Fm.later.res(+s: M.State, +now: Nat, +link: String, +name: String, +tool: String, +thread: String, +rq: String) -> MC.Res:
  MC.Res{True{}, J.Json.obj([J.KV{"queued", J.Json.bool(True{})}, J.KV{"machine", J.Str{name}},
    J.KV{"note", J.Str{name ++ " is away: this " ++ tool ++ " is queued and goes there, in order, when it is back (kept a day). You will be told what came of it."}}]),
    B.Fq.queue(s, now, link, thread, tool, rq)}

# one machine as the agent sees it
# (a mirror's projects, from its fold kept with it: FR.In.view)
def Fm.machine(+i: FR.In) -> J.Json:
  J.Json.obj([J.KV{"machine", J.Str{FR.In.name.of(i)}}, J.KV{"link", J.Str{FR.In.link(i)}},
    J.KV{"state", J.Str{Bool.pick(String, FR.In.away(i), "away", "here")}},
    J.KV{"projects", J.Json.arr(Fm.proj.rows(FR.In.view(i)))}])

def Fm.machines.go(ins: List<&2, FR.In>) -> List<&2, J.Json>:
  match ins:
    case Nil{}:
      Nil{}
    case +i <> t:
      Fm.machine(i) <> Fm.machines.go(t)

# this machine and the owner's others, each with its projects (ids a
# thread_launch `project` takes, threads in each by thread_list `machine`)
def Fm.machines(+s: M.State, ins: List<&2, FR.In>) -> MC.Res:
  MC.Res.ok(J.Json.obj([J.KV{"machines", J.Json.arr(J.Json.obj([J.KV{"machine", J.Str{"this"}}, J.KV{"state", J.Str{"here"}}, J.KV{"projects", J.Json.arr(Fm.proj.rows(s))}]) <> Fm.machines.go(ins))},
    J.KV{"note", J.Str{"Name a machine in thread_list and thread_launch (`machine`), or use the ids shown here (<link>~<id>) with thread_send, thread_read, thread_wait and thread_interrupt."}}]))

# the machine word a call names ("" for none: it is this hub's)
def Fm.word(+i: Nat, +a: J.Json) -> String:
  Bool.pick(String, Bool.or(Nat.is_lt(i, 4n), Nat.is_eq(i, 8n)),
    Bool.pick(String, FR.Fr.is(J.Json.get_str(a, "threadId", "")), FR.Fr.link(J.Json.get_str(a, "threadId", "")), ""),
    Bool.pick(String, FR.Fr.is(J.Json.get_str(a, "project", "")), FR.Fr.link(J.Json.get_str(a, "project", "")), J.Json.get_str(a, "machine", "")))

# a machine that is away: thread_send and thread_launch wait for it
# (B.Fq.*), anything else is refused
def Fm.plan.away(later: Bool, +i: FR.In, +tool: String, +rq: String) -> Plan:
  match later:
    case True{}:
      PLater{FR.In.link(i), FR.In.name.of(i), tool, rq}
    case False{}:
      PAnswer{MC.Res.fail("the machine " ++ FR.In.name.of(i) ++ " is away; try again when it is back")}

def Fm.plan.i(m: Maybe<&2, FR.In>, +word: String, +tool: String, +thread: String, a: J.Json) -> Plan:
  match m:
    case Some{+i}:
      +rq = Fm.request(FR.In.link(i), tool, FR.Fr.id(FR.In.link(i), thread), a)
      Bool.pick(Plan, FR.In.away(i), Fm.plan.away(B.Fq.tool(tool), i, tool, rq), PFar{FR.In.link(i), rq})
    case None{}:
      PAnswer{MC.Res.fail("no machine of yours called " ++ word ++ " is linked here (machine_list shows them)")}

def Fm.plan.w(+word: String, +ins: List<&2, FR.In>, +tool: String, +thread: String, a: J.Json) -> Plan:
  Bool.pick(Plan, String.is_empty(word), PHere{}, Fm.plan.i(XR.Xref.pick(ins, RF.Slug.of(word)), word, tool, thread, a))

# (thread_read on a machine that is away reads its mirror here)
def Fm.plan.read.i(m: Maybe<&2, FR.In>, +word: String, +tool: String, +thread: String, a: J.Json) -> Plan:
  match m:
    case Some{+i}:
      Bool.pick(Plan, FR.In.away(i), PHere{}, PFar{FR.In.link(i), Fm.request(FR.In.link(i), tool, FR.Fr.id(FR.In.link(i), thread), a)})
    case None{}:
      PHere{}

def Fm.plan.read(+word: String, +ins: List<&2, FR.In>, +tool: String, +thread: String, a: J.Json) -> Plan:
  Bool.pick(Plan, String.is_empty(word), PHere{}, Fm.plan.read.i(XR.Xref.pick(ins, RF.Slug.of(word)), word, tool, thread, a))

# plane_read of another machine: its plane, ids made its link's, under its name here
def Fm.plane.args(a: J.Json, +link: String, +name: String) -> J.Json:
  J.Obj{J.Field{"as", J.Str{link}, J.Field{"name", J.Str{name}, FR.Fr.obj.fields(a)}}}

def Fm.plan.pi(m: Maybe<&2, FR.In>, +word: String, +tool: String, +thread: String, a: J.Json) -> Plan:
  match m:
    case Some{+i}:
      Bool.pick(Plan, FR.In.away(i), PAnswer{MC.Res.fail("the machine " ++ FR.In.name.of(i) ++ " is away; try again when it is back")},
        PFar{FR.In.link(i), Fm.request(FR.In.link(i), tool, FR.Fr.id(FR.In.link(i), thread), Fm.plane.args(a, FR.In.link(i), FR.In.name.of(i)))})
    case None{}:
      PAnswer{MC.Res.fail("no machine of yours called " ++ word ++ " is linked here (machine_list shows them)")}

def Fm.plan.pw(+word: String, +ins: List<&2, FR.In>, +tool: String, +thread: String, a: J.Json) -> Plan:
  Bool.pick(Plan, String.is_empty(word), PHere{}, Fm.plan.pi(XR.Xref.pick(ins, RF.Slug.of(word)), word, tool, thread, a))

def Fm.plan.k(i: Nat, +s: M.State, +ins: List<&2, FR.In>, +tool: String, +thread: String, +a: J.Json) -> Plan:
  match i:
    case 12n:
      PHere{}
    case 11n:
      PHere{}
    case 10n:
      Fm.plan.pw(Fm.word(i, a), ins, tool, thread, a)
    case 9n:
      PHere{}
    case 8n:
      Fm.plan.read(Fm.word(i, a), ins, tool, thread, a)
    case 7n:
      PAnswer{Fm.machines(s, ins)}
    case _:
      Fm.plan.w(Fm.word(i, a), ins, tool, thread, a)

# where a tools/call goes: here (the usual run), answered by the hub, or
# to the machine it names
def Fm.plan.o(+o: J.Json, +s: M.State, +ins: List<&2, FR.In>, +thread: String) -> Plan:
  Bool.pick(Plan, String.eq(J.Json.get_str(o, "method", ""), "tools/call"),
    Fm.plan.k(C.Codec.pos(Fm.names(), J.Json.get_str(J.Json.get_obj(o, "params"), "name", "")), s, ins, J.Json.get_str(J.Json.get_obj(o, "params"), "name", ""), thread, J.Json.get_obj(J.Json.get_obj(o, "params"), "arguments")),
    PHere{})

def Fm.answer.r(m: Maybe<&2, J.Json>, +ok: Bool, +err: String, +link: String) -> MC.Res:
  match m:
    case Some{+b}:
      MC.Res{ok, Fm.far.json(b, link), B.Step.none()}
    case None{}:
      MC.Res.fail(err)

# the JSON-RPC answer from a machine's reply (code 0: no answer at all)
def Fm.answer.b(m: Maybe<&2, J.Json>, +id: J.Json, +link: String) -> String:
  match m:
    case Some{+r}:
      XR.Xref.out(MC.Mcp.reply(id, Fm.answer.r(J.Json.get(J.Json.get_obj(r, "mcp"), "body"), J.Json.get_bool(J.Json.get_obj(r, "mcp"), "ok", False{}), J.Json.get_str(r, "error", "that machine gave no answer"), link)))
    case None{}:
      XR.Xref.out(MC.Mcp.reply(id, MC.Res.fail("that machine's answer was not understood")))

def Fm.answer(+id: J.Json, +link: String, +code: Nat, +body: String) -> String:
  Bool.pick(String, Nat.is_eq(code, 200n), Fm.answer.b(J.Json.parse(body), id, link),
    XR.Xref.out(MC.Mcp.reply(id, MC.Res.fail(Bool.pick(String, Nat.is_eq(code, 0n), "that machine is away or did not answer", "that machine did not take it: " ++ String.take(body, 160n))))))

# The agent's prompt
# ------------------
# which machines are linked as the agent starts ("" for none; how to use
# them is in the orchestration paragraph)

def Fm.nm(+i: FR.In) -> String:
  Bool.pick(String, String.is_empty(FR.In.name.of(i)), FR.In.link(i), FR.In.name.of(i)) ++ Bool.pick(String, FR.In.away(i), " (away)", "")

def Fm.nms(ins: List<&2, FR.In>) -> List<&2, String>:
  match ins:
    case Nil{}:
      Nil{}
    case +i <> t:
      Fm.nm(i) <> Fm.nms(t)

def Fm.prompt.k(ns: List<&2, String>) -> String:
  match ns:
    case Nil{}:
      ""
    case +x <> +t:
      "<machines>The person's other machines linked to this one: " ++ String.join(x <> t, ", ") ++ ". machine_list shows their projects and ids.</machines>"

def Fm.prompt(ins: List<&2, FR.In>) -> String:
  Fm.prompt.k(Fm.nms(ins))

# /give elsewhere
# ---------------
# `/give >machine:slug` hands the conversation to a thread on another
# machine, and `/give %machine:project` to a new thread in that machine's
# project: thread_handoff there queues it ahead of the next message, as a
# local /give does (Fm.hand), and the person hears back only once each
# machine took it (the server's Far.give). Words naming something here are
# the hub's own (bothub.bend's Give.*), read from the text with the machine
# words left out (Fm.give.local), so `%beta:board` is never also a local
# `%beta`. A thread elsewhere gives from its own machine.

# a project on another machine: `%machine:name`
def Fm.pword(+w: String) -> Bool:
  Bool.and(String.eq(RF.Refs.kind(w), "project"), Bool.and(String.contains(w, ":"), Bool.not(String.is_empty(RF.Slug.of(RF.Refs.x.m(String.drop(w, 1n)))))))

def Fm.far.word(+w: String) -> Bool:
  Bool.or(XR.Xref.named(w), Fm.pword(w))

# what the source thread gives: its conversation, title, provider and model
type Src is Data:
  Src{id: String, hand: String, title: String, provider: String, model: String, key: String}

def Fm.src.of(t: M.Thread, +id: String, +hand: String, +key: String) -> Src:
  match t:
    case M.Thread{_, _, +ti, _, _, _, _, _, _, _, _, _, _, _, _, +pr, +mo, _, _, _, _}:
      Src{id, hand, ti, pr, mo, key}

def Fm.hand.req(+link: String, +i: Nat, src: Src, +target: J.KV) -> Plan:
  match src:
    case Src{+id, +hand, +ti, +pr, +mo, +key}:
      PFar{link, Fm.request(link, "thread_handoff", FR.Fr.id(link, id), J.Json.obj([target, J.KV{"hand", J.Str{hand}}, J.KV{"key", J.Str{key ++ "." ++ Nat.show(i)}},
        J.KV{"title", J.Str{ti}}, J.KV{"provider", J.Str{pr}}, J.KV{"model", J.Str{mo}}]))}

def Fm.give.thread(m: Maybe<&2, RF.Cand>, +link: String, +word: String, +i: Nat, src: Src) -> Plan:
  match m:
    case Some{RF.Cand{+id, _, _, _}}:
      Fm.hand.req(link, i, src, J.KV{"threadId", J.Str{id}})
    case None{}:
      PAnswer{MC.Res.fail("no thread " ++ word ++ " on that machine")}

def Fm.give.t(m: Maybe<&2, FR.In>, +slug: String, +word: String, +i: Nat, src: Src) -> Plan:
  match m:
    case Some{+in}:
      Fm.give.thread(List.head(&2, RF.Cand, List.sort(~RF.Cand, ~RF.Cand.before, RF.Refs.t.hits(XR.Xref.threads(FR.In.view(in)), slug, ""))), FR.In.link(in), word, i, src)
    case None{}:
      PAnswer{MC.Res.fail("no machine of yours called " ++ RF.Refs.x.m(String.drop(word, 1n)) ++ " is linked here")}

def Fm.give.p(m: Maybe<&2, FR.In>, +name: String, +word: String, +i: Nat, src: Src) -> Plan:
  match m:
    case Some{+in}:
      Fm.hand.req(FR.In.link(in), i, src, J.KV{"project", J.Str{name}})
    case None{}:
      PAnswer{MC.Res.fail("no machine of yours called " ++ RF.Refs.x.m(String.drop(word, 1n)) ++ " is linked here")}

# (match on the word's kind, so only its own lookup runs)
def Fm.give.word.k(named: Bool, proj: Bool, +w: String, +ins: List<&2, FR.In>, +i: Nat, src: Src) -> List<&2, Plan>:
  match named proj:
    case True{} _:
      [Fm.give.t(XR.Xref.pick(ins, RF.Slug.of(RF.Refs.x.m(String.drop(w, 1n)))), RF.Slug.of(RF.Slug.take(RF.Refs.x.q(String.drop(w, 1n)))), w, i, src)]
    case False{} True{}:
      [Fm.give.p(XR.Xref.pick(ins, RF.Slug.of(RF.Refs.x.m(String.drop(w, 1n)))), RF.Refs.x.q(String.drop(w, 1n)), w, i, src)]
    case False{} False{}:
      Nil{}

def Fm.give.words(ws: List<&2, String>, +ins: List<&2, FR.In>, +i: Nat, +src: Src) -> List<&2, Plan>:
  match ws:
    case Nil{}:
      Nil{}
    case +w <> t:
      List.append(&2, Plan, Fm.give.word.k(XR.Xref.named(w), Fm.pword(w), w, ins, i, src), Fm.give.words(t, ins, 1n+i, src))

def Fm.give.any(ws: List<&2, String>) -> Bool:
  match ws:
    case Nil{}:
      False{}
    case +w <> t:
      Bool.or(Fm.far.word(w), Fm.give.any(t))

# the text without its machine words (what the hub's own Give.* reads)
def Fm.give.drop.k(far: Bool, +w: String, rest: List<&2, String>) -> List<&2, String>:
  match far:
    case True{}:
      rest
    case False{}:
      w <> rest

def Fm.give.drop(ws: List<&2, String>) -> List<&2, String>:
  match ws:
    case Nil{}:
      Nil{}
    case +w <> t:
      Fm.give.drop.k(Fm.far.word(w), w, Fm.give.drop(t))

# (words are cut at spaces, tabs and new lines alike, as Refs.sigil.words
# cuts them, and each kept where it was)
def Fm.give.spaces(xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      String.join(Fm.give.drop(String.split(x, ' ')), " ") <> Fm.give.spaces(t)

def Fm.give.tabs(xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      String.join(Fm.give.spaces(String.split(x, '\t')), "\t") <> Fm.give.tabs(t)

def Fm.give.text(+text: String) -> String:
  String.join(Fm.give.tabs(String.split(text, '\n')), "\n")

def Fm.give.go(far: Bool, +s: M.State, +ins: List<&2, FR.In>, +src: String, +text: String, +now: Nat) -> List<&2, Plan>:
  match far:
    case True{}:
      [PAnswer{MC.Res.fail("give this thread from its own machine")}]
    case False{}:
      Fm.give.words(RF.Refs.sigil.words(text), ins, 0n, Fm.src.of(B.Hub.thread(s, src), src,
        String.take(HD.Hand.give(B.Hub.entries(s), src, HD.Hand.title(B.Hub.thread(s, src)), BH.Give.note(Fm.give.text(text))), 100000n), Nat.show(now) ++ "-" ++ Nat.show(B.Hub.seq(s))))

def Fm.give.k(any: Bool, +s: M.State, +ins: List<&2, FR.In>, +src: String, +text: String, +now: Nat) -> List<&2, Plan>:
  match any:
    case True{}:
      Fm.give.go(FR.Fr.is(src), s, ins, src, text, now)
    case False{}:
      Nil{}

# the requests a `bots.give` makes of other machines (none for any other
# request, or a text that names nothing elsewhere: a cheap test first)
def Fm.give(+s: M.State, +ins: List<&2, FR.In>, +o: J.Json, +now: Nat) -> List<&2, Plan>:
  Bool.pick(List<&2, Plan>, Bool.and(String.eq(J.Json.get_str(o, "m", ""), "bots.give"), String.contains(J.Json.get_str(J.Json.get_obj(o, "p"), "text", ""), ":")),
    Fm.give.k(Fm.give.any(RF.Refs.sigil.words(J.Json.get_str(J.Json.get_obj(o, "p"), "text", ""))), s, ins, J.Json.get_str(J.Json.get_obj(o, "p"), "thread", ""), J.Json.get_str(J.Json.get_obj(o, "p"), "text", ""), now), Nil{})

# the request as the hub runs it here: the same, its text without the
# machine words
def Fm.give.local(+o: J.Json) -> J.Json:
  J.Json.obj([J.KV{"id", Maybe.default(&2, J.Json, J.Json.get(o, "id"), J.Num{"0"})}, J.KV{"m", J.Str{"bots.give"}},
    J.KV{"p", J.Json.obj([J.KV{"thread", J.Str{J.Json.get_str(J.Json.get_obj(o, "p"), "thread", "")}}, J.KV{"text", J.Str{Fm.give.text(J.Json.get_str(J.Json.get_obj(o, "p"), "text", ""))}}])}])

# whether the text also names something here (bothub.bend's Give.*)
def Fm.give.here(+s: M.State, +o: J.Json) -> Bool:
  Bool.not(List.is_empty(&2, RF.Hit, RF.Refs.found(s, J.Json.get_str(J.Json.get_obj(o, "p"), "thread", ""), J.Json.get_str(J.Json.get_obj(o, "p"), "text", ""))))

# what a plan that failed says
def Fm.res.msg(r: MC.Res) -> String:
  match r:
    case MC.Res{_, +b, _}:
      J.Json.get_str(b, "error", "it could not be given")

# a plan's name (laws and logs)
def Fm.plan.name(p: Plan) -> String:
  match p:
    case PHere{}:
      "here"
    case PAnswer{_}:
      "answer"
    case PFar{_, _}:
      "far"
    case PLater{_, _, _, _}:
      "later"

# the plans' names, in order (laws)
def Fm.give.names(ps: List<&2, Plan>) -> String:
  match ps:
    case Nil{}:
      ""
    case p <> t:
      Fm.plan.name(p) ++ " " ++ Fm.give.names(t)
