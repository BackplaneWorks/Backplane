import Base
import ./check.bend as C
import ./mailtriage_test.bend as T
import ../src/core/model.bend as M
import ../src/core/hub.bend as H
import ../src/core/mailrun.bend as R
import ../src/core/mailprivate.bend as P
import ../src/core/mail.bend as ML
import ../src/core/mcp.bend as MC
import ../src/core/bothub.bend as BH
import ../src/core/json.bend as J
import ../src/core/notice.bend as N
import ../src/core/runlife.bend as RL
import ../src/server/mailcontext.bend as CX

def lane(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}: ""
    case M.ThreadCreated{+id, _, _, _, _, _, _, _, _} <> _: id
    case _ <> rest: lane(rest)

def starts(os: List<&2, H.Out>, +thread: String) -> Nat:
  match os:
    case Nil{}: 0n
    case H.AgentStart{+th, _, _, _, _, _, _} <> rest: Nat.add(Bool.pick(Nat, String.eq(th, thread), 1n, 0n), starts(rest, thread))
    case H.CodexTurn{+th, _, _, _, _, _} <> rest: Nat.add(Bool.pick(Nat, String.eq(th, thread), 1n, 0n), starts(rest, thread))
    case _ <> rest: starts(rest, thread)

def spawn(+s: M.State) -> H.Step:
  H.Run.escalate(s, 20n, "b", "c", "bot", "Verified material outcome with backplane://thread/worker")

def state() -> M.State:
  +s = T.running()
  M.State.replay(H.Queue.changes(spawn(s)), s)

def privateid() -> String:
  lane(H.Queue.changes(spawn(T.running())))

def args(+text: String, +key: String) -> J.Json:
  J.Json.obj([J.KV{"text", J.Str{text}}, J.KV{"key", J.Str{key}}])

def empty_event() -> J.Json:
  J.Json.obj([J.KV{"item", J.Json.obj([J.KV{"type", J.Str{"agent_message"}}, J.KV{"text", J.Str{""}}])}])

def empty_checks() -> IO(Unit):
  do IO<Unit>:
    +before : M.State = T.running()
    +st : H.Step = spawn(before)
    +th : String = lane(H.Queue.changes(st))
    +s : M.State = M.State.replay(H.Queue.changes(st), before)
    T.check("escalation creates a separate private thread", Bool.and(Bool.not(String.is_empty(th)), Bool.not(String.eq(th, "foh"))))
    T.n("escalation starts no original-chat turn", starts(H.Step.outs_of(st), "foh"), 0n)
    T.n("escalation starts one private turn", starts(H.Step.outs_of(st), th), 1n)
    C.Check.str("original chat receives no private ride", H.Ride.of(s, "foh"), "")
    T.n("escalation posts no original-chat entry", T.posts(H.Queue.changes(st), "foh"), 0n)
    T.check("shared UI projection hides private lane", R.Run.is(s, th))
    T.check("existing clients have compatible RunSet privacy metadata", Bool.not(String.is_empty(ML.Run.thread(T.run_got(ML.Box.run_of(M.State.mail(s), th))))))
    T.check("coordinator lane is not another triage assessor", Bool.not(R.Run.is.got(R.Run.of(s, th))))
    C.Check.str("private owner is authenticated coordinator", P.Private.owner(s, th), "b")
    C.Check.str("context reads original owner history", CX.Context.owner(s, th), "foh")
    T.check("private turn permits empty success explicitly", String.contains(BH.Bots.prompt(s, th, "/tmp"), "allow_empty_output=\"true\""))
    +empty : H.Step = H.Codex.line.k(1n, s, th, 21n, empty_event())
    +s2 : M.State = M.State.replay(H.Queue.changes(empty), s)
    +end : H.Step = H.Run.after(s2, 22n, H.Codex.line.k(2n, s2, th, 22n, J.Null{}))
    +s3 : M.State = M.State.replay(H.Queue.changes(end), s2)
    T.n("EMPTY REPLY adds zero messages to user conversation", T.posts(H.Queue.changes(empty), "foh"), 0n)
    T.n("EMPTY COMPLETION adds zero messages to user conversation", T.posts(H.Queue.changes(end), "foh"), 0n)
    T.n("EMPTY COMPLETION schedules zero private re-prompts", starts(H.Step.outs_of(end), th), 0n)
    T.n("EMPTY COMPLETION schedules zero user re-prompts", starts(H.Step.outs_of(end), "foh"), 0n)
    T.check("empty private completion retires its SDK", RL.Life.finished(s3, th))
    T.check("completed private lane cannot tell later", Bool.not(P.Private.live(s3, th)))
    T.check("ordinary user turn still receives normal replies", Nat.is_eq(T.posts(H.Queue.changes(H.Codex.item.k(0n, "foh", 23n, 1n, J.Json.obj([J.KV{"text", J.Str{"Direct answer to person"}}]), False{})), "foh"), 1n))

def output_checks() -> IO(Unit):
  do IO<Unit>:
    +s : M.State = state()
    +th : String = privateid()
    +out : H.Step = H.Codex.item.k(0n, th, 21n, 1n, J.Json.obj([J.KV{"text", J.Str{"Internal acknowledgement stays private"}}]), False{})
    T.n("nonempty final also stays out of original chat", T.posts(H.Queue.changes(out), "foh"), 0n)
    T.n("private final remains inspectable in source lane", T.posts(H.Queue.changes(out), th), 1n)
    T.check("private coordinator retains cross-project thread_send scope", MC.Mcp.same_project(s, th, "other"))
    T.check("worker cannot access private coordinator", Bool.not(MC.Mcp.same_project(s, "worker", th)))
    T.check("owning Bub can inspect private coordinator", MC.Mcp.same_project(s, "foh", th))
    T.check("private prompt retains Bub identity", String.contains(BH.Bots.prompt(s, th, "/tmp"), "# You are the uberbot"))
    T.check("private prompt retains current original conversation context", String.contains(BH.Bots.prompt(s, th, "/tmp"), "UDP address width unresolved"))
    T.check("private turn exposes explicit publication", String.contains(J.Json.show(MC.Mcp.tools.for(s, th)), "tell_person"))
    T.check("private coordinator retains thread_send tool", String.contains(J.Json.show(MC.Mcp.tools.for(s, th)), "thread_send"))
    +launched : MC.Res = MC.Mcp.launch(s, 23n, th, J.Json.obj([J.KV{"project", J.Str{"p"}}, J.KV{"prompt", J.Str{"Coordinate within existing scope"}}]))
    +child : String = J.Json.get_str(T.res_json(launched), "threadId", "")
    +with_child : M.State = M.State.replay(H.Queue.changes(T.res_step(launched)), s)
    T.check("private coordinator can dispatch work", Bool.and(MC.Res.good(launched), Bool.not(String.is_empty(child))))
    C.Check.str("dispatched reports keep original owner's triage pipeline", M.Launch.by(with_child, child), "foh")
    +reported : H.Step = H.Lr.send(with_child, 24n, child, M.Launch.by(with_child, child), "Routine source report")
    T.n("dispatched report files one private mail input", T.appends(H.Step.outs_of(reported)), 1n)
    T.n("dispatched report never posts original-chat entry", T.posts(H.Queue.changes(reported), "foh"), 0n)

    T.check("triage assessor cannot publish", Bool.not(String.contains(J.Json.show(MC.Mcp.tools.for(s, T.runid(s))), "tell_person")))
    T.check("private lane alerts no one by ending", String.is_empty(N.Notice.key(N.Notice.of(s, th, False{}, True{}))))
    +ask : M.State = M.State.apply(s, M.AskOpened{"pending", th, "approval", "Bash", "Existing runtime approval", "", 21n})
    T.check("genuine private approval remains live", RL.Life.ask(M.Asks.all(ask), th))
    +notice : MC.Res = BH.Bots.call.private(True{}, s, 22n, J.Null{}, th, J.Json.obj([J.KV{"name", J.Str{"notify_person"}}]))
    T.check("notify_person points to explicit publication instead of recursion", Bool.not(MC.Res.good(notice)))
    T.n("private notify refusal produces no new triage work", List.length(&2, H.Out, H.Step.outs_of(T.res_step(notice))), 0n)

def tell_checks() -> IO(Unit):
  do IO<Unit>:
    +s : M.State = state()
    +th : String = privateid()
    +a : J.Json = args("A fresh linked status: backplane://thread/worker", "artifact-v1")
    +r : MC.Res = MC.Mcp.tell_person(s, 23n, th, a)
    T.check("explicit tell accepted from live owning coordinator", MC.Res.good(r))
    T.n("explicit tell posts exactly one user-chat message", T.posts(H.Queue.changes(T.res_step(r)), "foh"), 1n)
    +post : M.State = M.State.replay(H.Queue.changes(T.res_step(r)), s)
    T.n("same tell key cannot post twice", T.posts(H.Queue.changes(T.res_step(MC.Mcp.tell_person(post, 24n, th, a))), "foh"), 0n)
    T.check("ordinary worker publication refused", Bool.not(MC.Res.good(MC.Mcp.tell_person(s, 24n, "worker", a))))
    T.check("triage publication refused", Bool.not(MC.Res.good(MC.Mcp.tell_person(s, 24n, T.runid(s), a))))
    T.check("target spoof refused", Bool.not(MC.Res.good(MC.Mcp.tell_person(s, 24n, th, J.Json.obj([J.KV{"text", J.Str{"wrong owner"}}, J.KV{"key", J.Str{"v2"}}, J.KV{"target", J.Str{"bot-thread"}}])))))
    +again : H.Step = H.Private.spawn(post, 25n, "b", "other-route", "Same artifact through another room")
    +th2 : String = lane(H.Queue.changes(again))
    +s2 : M.State = M.State.replay(H.Queue.changes(again), post)
    T.n("dedup spans independent private turns", T.posts(H.Queue.changes(T.res_step(MC.Mcp.tell_person(s2, 26n, th2, a))), "foh"), 0n)
    T.n("new exact artifact can be published", T.posts(H.Queue.changes(T.res_step(MC.Mcp.tell_person(s2, 26n, th2, args("Changed result", "artifact-v2")))), "foh"), 1n)
    +pending : M.State = M.State.apply(s2, M.AskOpened{"source-approval", "worker", "approval", "flash", "Hardware hold", "", 26n})
    +told : M.State = M.State.replay(H.Queue.changes(T.res_step(MC.Mcp.tell_person(pending, 27n, th2, args("Person decision needed: backplane://thread/worker", "source-approval")))), pending)
    T.check("explicit tell never answers real source approval", RL.Life.ask(M.Asks.all(told), "worker"))

def main() -> IO(Unit):
  do IO<Unit>:
    empty_checks()
    output_checks()
    tell_checks()
