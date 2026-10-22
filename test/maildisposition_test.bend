import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/hub.bend as H
import ../src/core/mcp.bend as MC
import ../src/core/maildisposition.bend as D
import ../src/core/mail.bend as ML
import ../src/core/mailbody.bend as MB
import ../src/core/mailrun.bend as R
import ../src/core/charter.bend as CH
import ../src/core/json.bend as J

# Public incident identities, not user memory or prompts. The resource hold is
# intentionally NOT a permission grant or a completed work objective.
def run() -> String: "run1792519184-284989"
def conv() -> String: "wake:w1792461724-229122"
def input() -> String: "t:t1792182150-155310:lr-r1792519183-284979"
def worker() -> String: "t1792182150-155310"

def incoming(+id: String) -> M.Change:
  M.MailPosted{id, conv(), "thread:" ++ worker(), "bot:root", "wake", "w1792461724-229122", "act", "Resource remains held; no new action", 32n, 1792519183n}

def base() -> M.State:
  M.State.replay([
    M.ThreadCreated{"foh", "", "Root", M.Local{}, "", "/tmp/dispositions", "codex", "gpt-6.1-sol", 1n},
    M.ThreadCreated{worker(), "", "Hold owner", M.Local{}, "", "/tmp/dispositions", "codex", "", 1n},
    M.ThreadCreated{run(), "", "Mailroom", M.Local{}, "", "/tmp/dispositions", "codex", "", 1n},
    M.BotSet{"root", "Root", "foh", 0n, "", 1n}, M.SettingSet{"mail.on", "on", 1n},
    M.SettingSet{"hold.enabled", "off", 1n},
    M.ConvSet{conv(), "Resource hold", "wake", "thread:" ++ worker() ++ "\nbot:root", "", "", 1n},
    incoming(input()), M.RunSet{run(), "root", conv(), input(), 1792519184n},
    M.TurnChanged{run(), M.Running{}, 1792519184n}], M.State.empty())

def args(+id: String, +mode: String) -> J.Json:
  J.Json.obj([J.KV{"input", J.Str{id}}, J.KV{"mode", J.Str{mode}},
    J.KV{"reason", J.Str{"Already-known resource hold; root handled this notice, no permission or work completion."}},
    J.KV{"text", J.Str{"Resource hold acknowledged; awaiting the existing owner."}}])

def call(+s: M.State, +a: J.Json) -> MC.Res:
  MC.Mcp.call.k(44n, s, 1792519190n, run(), J.Null{}, a)

def step(r: MC.Res) -> H.Step:
  match r:
    case MC.Res{_, _, st}: st

def apply(+s: M.State, +r: MC.Res) -> M.State:
  M.State.replay(MC.Res.changes(r), s)

def append(os: List<&2, H.Out>) -> List<&2, M.Change>:
  match os:
    case Nil{}: Nil{}
    case H.MailAppend{_, _, cs} <> _: cs
    case _ <> rest: append(rest)

def body(os: List<&2, H.Out>) -> Maybe<&2, MB.Body>:
  match os:
    case Nil{}: None{}
    case H.MailAppend{line, _, _} <> _: MB.Body.parse(line)
    case _ <> rest: body(rest)

def posts(cs: List<&2, M.Change>) -> Nat:
  match cs:
    case Nil{}: 0n
    case M.MessagePosted{"foh", _, _, _, _} <> rest: 1n+posts(rest)
    case _ <> rest: posts(rest)

def fallback(+s: M.State) -> H.Step:
  H.Run.after(s, 1792519206n, H.Step{[M.TurnChanged{run(), M.Completed{}, 1792519206n}], Nil{}})

def owes(+s: M.State) -> Nat:
  posts(H.Queue.changes(fallback(s)))

def b(+label: String, got: Bool, expected: Bool) -> IO(Unit):
  C.Check.str(label, Bool.show(got), Bool.show(expected))

def n(+label: String, got: Nat, expected: Nat) -> IO(Unit):
  C.Check.str(label, Nat.show(got), Nat.show(expected))

def reply_checks(m: Maybe<&2, MB.Body>, +s: M.State) -> IO(Unit):
  match m:
    case None{}: C.Check.str("real reply body exists", "missing", "present")
    case Some{+body}:
      do IO<Unit>:
        C.Check.str("reply uses current input's local sender", ML.Mail.to(MB.Body.mail(body)), "thread:" ++ worker())
        C.Check.str("reply stays in exact wake conversation", ML.Mail.conv(MB.Body.mail(body)), conv())
        C.Check.str("reply uses actual run-thread identity", ML.Mail.from(MB.Body.mail(body)), "thread:" ++ run())
        C.Check.str("reply is FYI", ML.Mail.wanted(MB.Body.mail(body)), "fyi")
        C.Check.str("reply correlates input in durable header", ML.Mail.ref(MB.Body.mail(body)), input())
        n("FYI does not start another mail run", List.length(&2, H.Out, H.Step.outs_of(H.Mail.route(s, 1792519191n, body))), 0n)

def main() -> IO(Unit):
  do IO<Unit>:
    +s : M.State = base()
    n("actual denied-tools incident still exposes unresolved input", owes(s), 1n)
    +done : MC.Res = call(s, args(input(), "done"))
    b("known hold can be durably handled", MC.Res.good(done), True{})
    +handled : M.State = apply(s, done)
    n("handled notice avoids repeating known hold escalation", owes(handled), 0n)
    C.Check.str("Hold remains disabled", M.Setting.get(handled, "hold.enabled", ""), "off")
    C.Check.str("conversation is not marked done", ML.Mark.status(ML.Box.mark_of(M.State.mail(handled), "root", conv())), ML.Mark.status(ML.Box.mark_of(M.State.mail(s), "root", conv())))
    b("notice does not fake a running or completed turn", H.Hub.running(H.Hub.thread(handled, run())), True{})
    n("notice bookkeeping starts no effects", List.length(&2, H.Out, H.Step.outs_of(step(done))), 0n)
    n("duplicate exact input adds no durable changes", List.length(&2, M.Change, MC.Res.changes(call(handled, args(input(), "done")))), 0n)
    n("duplicate exact input emits no effects", List.length(&2, H.Out, H.Step.outs_of(step(call(handled, args(input(), "reply"))))), 0n)
    n("replayed disposition still settles this notice", owes(M.State.replay(MC.Res.changes(done), handled)), 0n)
    n("explicit notice defer avoids duplicate fallback", owes(apply(s, call(s, args(input(), "defer")))), 0n)
    +m2 : M.State = M.State.apply(handled, incoming("m2"))
    n("new same-second input is NOT handled by old receipt", owes(m2), 1n)
    b("stale m1 caller cannot dispose m2", MC.Res.good(call(m2, args(input(), "done"))), False{})
    n("stale input makes no changes", List.length(&2, M.Change, MC.Res.changes(call(m2, args(input(), "done")))), 0n)
    +reported : M.State = M.State.replay(H.Queue.changes(fallback(m2)), m2)
    n("new actionable fallback remains exactly once", owes(reported), 0n)
    C.Check.str("fallback still names new input", M.Setting.get(reported, "run.forwarded." ++ run(), ""), "m2")
    n("read/delivery alone is not disposition", owes(M.State.apply(s, M.MailMarked{"root", conv(), "open", 1792519183n, 0n, 1792519185n})), 1n)
    b("ordinary FOH cannot use the endpoint", MC.Res.good(MC.Mcp.call.k(44n, s, 9n, "foh", J.Null{}, args(input(), "done"))), False{})
    b("mail off refuses disposition", MC.Res.good(call(M.State.apply(s, M.SettingSet{"mail.on", "off", 9n}), args(input(), "done"))), False{})
    b("ended run refuses writes", MC.Res.good(call(M.State.apply(s, M.TurnChanged{run(), M.Completed{}, 9n}), args(input(), "done"))), False{})
    b("provider action is not a notice mode", MC.Res.good(call(s, args(input(), "deploy"))), False{})
    b("extra target is rejected rather than honored", MC.Res.good(call(s, J.Json.obj([J.KV{"input", J.Str{input()}}, J.KV{"mode", J.Str{"reply"}}, J.KV{"reason", J.Str{"status"}}, J.KV{"text", J.Str{"hello"}}, J.KV{"to", J.Str{"https://outside.invalid"}}]))), False{})
    b("other-conversation argument is rejected", MC.Res.good(call(s, J.Json.obj([J.KV{"input", J.Str{input()}}, J.KV{"mode", J.Str{"done"}}, J.KV{"reason", J.Str{"status"}}, J.KV{"conv", J.Str{"other"}}]))), False{})
    b("empty reason is refused", MC.Res.good(call(s, J.Json.obj([J.KV{"input", J.Str{input()}}, J.KV{"mode", J.Str{"done"}}, J.KV{"reason", J.Str{" "}}]))), False{})
    b("empty reply is refused", MC.Res.good(call(s, J.Json.obj([J.KV{"input", J.Str{input()}}, J.KV{"mode", J.Str{"reply"}}, J.KV{"reason", J.Str{"status"}}]))), False{})
    +reply : MC.Res = call(s, args(input(), "reply"))
    b("bounded local reply prepares one append", MC.Res.good(reply), True{})
    n("reply has one append effect", List.length(&2, H.Out, H.Step.outs_of(step(reply))), 1n)
    +pending : M.State = apply(s, reply)
    n("uncommitted or failed journal still requires fallback", owes(pending), 1n)
    n("unknown pending reply is not automatically resent", List.length(&2, H.Out, H.Step.outs_of(step(call(pending, args(input(), "reply"))))), 0n)
    n("pending alone cannot complete by switching mode", owes(apply(pending, call(pending, args(input(), "done")))), 1n)
    +cs : List<&2, M.Change> = append(H.Step.outs_of(step(reply)))
    +committed : M.State = M.State.replay(cs, pending)
    n("actual journal commit records reply and disposition", owes(committed), 0n)
    n("post-commit duplicate emits no append", List.length(&2, H.Out, H.Step.outs_of(step(call(committed, args(input(), "reply"))))), 0n)
    reply_checks(body(H.Step.outs_of(step(reply))), committed)
    +late : M.State = M.State.replay(cs, M.State.apply(pending, incoming("m2")))
    n("late m1 reply CANNOT swallow newer m2", owes(late), 1n)
    +m2done : M.State = apply(M.State.apply(pending, incoming("m2")), call(M.State.apply(pending, incoming("m2")), args("m2", "done")))
    n("late m1 append cannot overwrite m2 disposition", owes(M.State.replay(cs, m2done)), 0n)
    +external : M.State = M.State.apply(s, M.MailPosted{"external", conv(), "thread:peer~remote", "bot:root", "thread", "", "act", "status", 6n, 9n})
    b("remote sender cannot receive automatic internal reply", MC.Res.good(call(external, args("external", "reply"))), False{})
    +room : M.State = M.State.apply(s, M.MailPosted{"room", conv(), "room:shared", "bot:root", "thread", "", "act", "status", 6n, 9n})
    b("room reply cannot trigger mentions or fanout", MC.Res.good(call(room, args("room", "reply"))), False{})
    b("Codex grant applies to a real live mail run", D.Disposition.launch(s, run()), True{})
    b("Codex grant never applies to ordinary FOH", D.Disposition.launch(s, "foh"), False{})
    b("live run advertises bounded endpoint", String.contains(J.Json.show(MC.Mcp.tools.for(s, run())), "\"name\":\"mail_dispose\""), True{})
    b("ordinary tools do not advertise the endpoint", String.contains(J.Json.show(MC.Mcp.tools.for(s, "foh")), "\"name\":\"mail_dispose\""), False{})
    b("ended run does not advertise the endpoint", String.contains(J.Json.show(MC.Mcp.tools.for(M.State.apply(s, M.TurnChanged{run(), M.Completed{}, 9n}), run())), "\"name\":\"mail_dispose\""), False{})
    n("ordinary Codex receives no additional flags", List.length(&2, String, D.Disposition.codex(False{})), 0n)
    C.Check.str("only mail_dispose is preauthorized", String.join(D.Disposition.codex(True{}), " "), "-c mcp_servers.backplane.tools.mail_dispose.approval_mode=\"approve\"")
    b("new tool remains a WRITE for generation checks", H.Gen.changes(J.Json.obj([J.KV{"method", J.Str{"tools/call"}}, J.KV{"params", J.Json.obj([J.KV{"name", J.Str{"mail_dispose"}}])}])), True{})
    b("stale provider generation still refused", H.Gen.refused("old", "current", J.Json.obj([J.KV{"method", J.Str{"tools/call"}}, J.KV{"params", J.Json.obj([J.KV{"name", J.Str{"mail_dispose"}}])}])), True{})
    b("external Gmail still risky", CH.Charter.risky("gmail_send", J.Null{}), True{})
    b("deploy still risky", CH.Charter.risky("deploy_main", J.Null{}), True{})
    b("shell remains reviewed", CH.Charter.risky("exec_command", J.Json.obj([J.KV{"cmd", J.Str{"rm -rf /tmp/test"}}])), True{})
    b("brief exposes exact input token", String.contains(R.Run.brief(s, "root", conv()), "Current actionable input: " ++ input()), True{})
    b("brief distinguishes notice from objective completion", String.contains(R.Run.brief(s, "root", conv()), "never permission or completion of the underlying objective"), True{})
