import Base
import ./check.bend as C
import ../src/core/fallback.bend as F
import ../src/core/hub.bend as H
import ../src/core/model.bend as M
import ../src/core/codec.bend as CD
import ../src/core/json.bend as J
import ../src/core/notice.bend as N
import ../src/mobile/notify.bend as PN
import ../src/server/push.bend as PUSH
import ../src/core/mail.bend as ML
import ../src/core/mailrun.bend as MR

# Fictional capability catalog: the test never invokes a real provider.
def caps() -> J.Json:
  J.Json.obj([J.KV{"claude", J.Json.arr([J.Str{"sonnet"}])}, J.KV{"codex", J.Json.arr([J.Str{"gpt-6.1-sol"}, J.Str{"test-next"}])}])

def chain() -> String:
  "claude/sonnet,codex/gpt-6.1-sol,codex/test-next"

def base() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "Test", "/tmp/fallback-project", 1n},
    M.ThreadCreated{"t1", "p", "Worker", M.Local{}, "", "/tmp/fallback-project", "claude", "sonnet", 1n},
    M.ThreadCreated{"bot", "p", "Bot", M.Local{}, "", "/tmp/fallback-bot", "claude", "sonnet", 1n},
    M.BotSet{"b", "Bot", "bot", 0n, "", 1n}, M.SessionSet{"t1", "claude-session", 2n},
    M.MessagePosted{"t1", "original-request", M.User{}, "Build once", 2n},
    M.ActivityLogged{"t1", "side-effect-1", "tool", "Bash", "Created the file", 3n},
    M.TurnChanged{"t1", M.Running{}, 3n}, M.TurnChanged{"bot", M.Running{}, 3n},
    M.SettingSet{"fallback.chain", chain(), 1n}, M.SettingSet{"fallback.on", "on", 1n},
    M.SettingSet{"fallback.input.t1", "Build once", 2n}], M.State.empty())

def json(s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def limit() -> String:
  "{\"type\":\"result\",\"is_error\":true,\"errors\":[\"Weekly limit reached\"],\"resets_at\":1000}"

def selected(s: M.State, th: String) -> String:
  F.Target.key(H.Fallback.target(H.Hub.thread(s, th)))

def fold(+s: M.State, st: H.Step) -> M.State:
  M.State.replay(H.Queue.changes(st), s)

def fail(+s: M.State, +now: Nat, +th: String, line: String) -> H.Step:
  H.Fallback.after(s, caps(), now, H.Hub.agent_line(s, now, th, line))

def first() -> H.Step:
  fail(base(), 10n, "t1", limit())

def post() -> M.State:
  fold(base(), first())

def starts(os: List<&2, H.Out>) -> String:
  match os:
    case Nil{}: ""
    case H.CodexTurn{_, _, m, se, _, +text} <> t:
      m ++ "/session=" ++ se ++ "/" ++ text ++ starts(t)
    case H.AgentStart{_, _, p, m, se, _, _} <> t: p ++ "/" ++ m ++ se ++ starts(t)
    case _ <> t: starts(t)

def alerts(cs: List<&2, M.Change>) -> Nat:
  match cs:
    case Nil{}: 0n
    case M.SettingSet{+k, _, _} <> t:
      Nat.add(Bool.pick(Nat, String.starts_with(k, "alert."), 1n, 0n), alerts(t))
    case _ <> t: alerts(t)

def completed(cs: List<&2, M.Change>) -> Bool:
  match cs:
    case Nil{}: False{}
    case M.TurnChanged{_, M.Completed{}, _} <> _: True{}
    case _ <> t: completed(t)

def quota(s: String) -> String:
  Bool.show(Maybe.is_some(&2, F.Limit, F.Error.classify("claude", "sonnet", json(s))))

def codex_limit() -> String:
  "{\"type\":\"turn.failed\",\"error\":{\"code\":\"quota_exceeded\",\"scope\":\"model\",\"message\":\"model quota exhausted\",\"resets_at\":2000}}"

def showpick(x: Maybe<&2, F.Target>) -> String:
  match x:
    case None{}: "blocked"
    case Some{t}: F.Target.key(t)

def codec(cs: List<&2, M.Change>) -> List<&2, M.Change>:
  match cs:
    case Nil{}: Nil{}
    case +h <> t: Maybe.default(&2, M.Change, CD.Change.decode(CD.Change.encode(h)), h) <> codec(t)

def run_identity(r: Maybe<&2, ML.Run>) -> String:
  match r:
    case None{}: "missing"
    case Some{+r}: ML.Run.thread(r) ++ "/" ++ ML.Run.bot(r) ++ "/" ++ ML.Run.conv(r) ++ "/" ++ ML.Run.mail(r)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("weekly result is quota", quota(limit()), "True")
    C.Check.str("Codex message-only usage limit is quota", quota("{\"type\":\"turn.failed\",\"error\":{\"message\":\"You’ve hit your usage limit. Try again later.\"}}"), "True")
    C.Check.str("Codex ASCII usage limit is quota", quota("{\"type\":\"turn.failed\",\"error\":{\"message\":\"You've hit your usage limit for gpt-6.1-sol. Switch to another model now.\"}}"), "True")
    C.Check.str("Codex named-model limit leaves other models available", F.Error.scope("codex", "gpt-6.1-sol", Maybe.default(&2, J.Json, J.Json.parse("{\"type\":\"turn.failed\",\"error\":{\"message\":\"You’ve hit your usage limit for gpt-6.1-sol. Switch to another model now.\"}}"), J.Null{})), "codex/gpt-6.1-sol")
    C.Check.str("Codex account usage limit holds all models", F.Error.scope("codex", "gpt-6.1-sol", Maybe.default(&2, J.Json, J.Json.parse("{\"type\":\"turn.failed\",\"error\":{\"message\":\"You’ve hit your usage limit.\"}}"), J.Null{})), "codex")
    C.Check.str("explicit model scope wins over limit wording", F.Error.scope("codex", "gpt-6.1-sol", Maybe.default(&2, J.Json, J.Json.parse("{\"type\":\"turn.failed\",\"error\":{\"scope\":\"model\",\"message\":\"usage limit reached\"}}"), J.Null{})), "codex/gpt-6.1-sol")
    C.Check.str("tool text is not quota", quota("{\"type\":\"assistant\",\"message\":\"weekly limit reached\"}"), "False")
    C.Check.str("network error not quota", quota("{\"type\":\"turn.failed\",\"message\":\"connection reset\"}"), "False")
    C.Check.str("keyring error not quota", quota("{\"type\":\"turn.failed\",\"message\":\"login keyring is locked\"}"), "False")
    C.Check.str("login error not quota", quota("{\"type\":\"turn.failed\",\"error\":{\"code\":\"rate_limit_error\",\"message\":\"authentication login required\"}}"), "False")
    C.Check.str("cancelled error not quota", quota("{\"type\":\"turn.failed\",\"message\":\"cancelled; weekly limit\"}"), "False")
    C.Check.str("allowed plan with rejected overage is not quota", quota("{\"type\":\"rate_limit_event\",\"rate_limit_info\":{\"status\":\"allowed\",\"overageStatus\":\"rejected\"}}"), "False")
    C.Check.str("rejected plan is quota", quota("{\"type\":\"rate_limit_event\",\"rate_limit_info\":{\"status\":\"rejected\",\"rateLimitType\":\"seven_day\",\"resetsAt\":1000}}"), "True")
    C.Check.str("catalog rejects uninstalled model", Bool.show(F.Chain.valid(F.Chain.parse("codex/missing"), caps())), "False")
    C.Check.str("catalog rejects duplicates", Bool.show(F.Chain.valid(F.Chain.parse("codex/gpt-6.1-sol,codex/gpt-6.1-sol"), caps())), "False")
    C.Check.str("catalog accepts selected models", Bool.show(F.Chain.valid(F.Chain.parse(chain()), caps())), "True")
    C.Check.str("chain moves up", F.Chain.show(F.Chain.up(F.Chain.parse(chain()), "codex/test-next")), "claude/sonnet,codex/test-next,codex/gpt-6.1-sol")
    C.Check.str("chain removes", F.Chain.show(F.Chain.remove(F.Chain.parse(chain()), "claude/sonnet")), "codex/gpt-6.1-sol,codex/test-next")
    C.Check.str("never wraps at end", showpick(F.Chain.pick(F.Chain.suffix(F.Chain.parse(chain()), "codex/test-next"), base(), caps(), 10n)), "blocked")
    C.Check.str("absent explicit choice starts at head", F.Chain.show(F.Chain.suffix(F.Chain.parse(chain()), "manual/model")), chain())
    C.Check.str("Claude canonical text-block input preserved", H.Fallback.claude.text(H.Claude.send("t1", "/tmp", "exact input", "uuid")), "exact input")
    C.Check.str("original request reaches fallback", Bool.show(String.contains(starts(H.Step.outs_of(first())), "Original request (context, not a new task):\nBuild once")), "True")
    C.Check.str("first fallback exact requested model", selected(post(), "t1"), "codex/gpt-6.1-sol")
    C.Check.str("original intent retained", M.Setting.get(post(), "fallback.intent.t1", ""), "claude/sonnet")
    C.Check.str("workspace retained", H.Hub.cwd(post(), H.Hub.thread(post(), "t1")), "/tmp/fallback-project")
    C.Check.str("new model clean session", M.Switch.sess(H.Hub.thread(post(), "t1")), "")
    C.Check.str("old model session retained", M.Setting.get(post(), "fallback.session.t1.claude/sonnet", ""), "claude-session")
    C.Check.str("no false task completion", Bool.show(completed(H.Queue.changes(first()))), "False")
    C.Check.str("handoff contains completed effect", Bool.show(String.contains(starts(H.Step.outs_of(first())), "Created the file")), "True")
    C.Check.str("continuation forbids repeating actions", Bool.show(String.contains(starts(H.Step.outs_of(first())), "Do not repeat completed tool actions")), "True")
    C.Check.str("one persistent fallback alert", Nat.show(alerts(H.Queue.changes(first()))), "1")
    C.Check.str("persistent alert names from and to", Bool.show(String.contains(N.Alert.rtext(N.Alert.get(post(), "t1")), "claude/sonnet → codex/gpt-6.1-sol")), "True")
    C.Check.str("persistent alert includes affected count", Bool.show(String.contains(N.Alert.rtext(N.Alert.get(post(), "t1")), "2 active threads affected")), "True")
    C.Check.str("known reset cached", Nat.show(J.Json.get_nat(F.Limit.record(post(), "claude"), "reset", 0n)), "1000")
    C.Check.str("known reset held before", Bool.show(F.Target.held(post(), F.Target{"claude", "sonnet"}, 999n)), "True")
    C.Check.str("known reset eligible at boundary", Bool.show(F.Target.held(post(), F.Target{"claude", "sonnet"}, 1000n)), "False")
    C.Check.str("working model stays after reset", selected(post(), "t1"), "codex/gpt-6.1-sol")
    +bot : H.Step = fail(post(), 11n, "bot", limit())
    C.Check.str("second bot same outage switches", selected(fold(post(), bot), "bot"), "codex/gpt-6.1-sol")
    C.Check.str("second bot same outage no alert storm", Nat.show(alerts(H.Queue.changes(bot))), "0")
    C.Check.str("second bot keeps first outage identity", M.Setting.get(fold(post(), bot), "fallback.outage.claude", ""), M.Setting.get(post(), "fallback.outage.claude", ""))
    +second : H.Step = fail(post(), 20n, "t1", codex_limit())
    +s2 : M.State = fold(post(), second)
    C.Check.str("next configured step used", selected(s2, "t1"), "codex/test-next")
    C.Check.str("model scope leaves other codex model available", Bool.show(F.Target.held(s2, F.Target{"codex", "test-next"}, 21n)), "False")
    +third : H.Step = fail(s2, 21n, "t1", codex_limit())
    +s3 : M.State = fold(s2, third)
    C.Check.str("exhaustion blocks without wrap", Bool.show(H.Hub.running(H.Hub.thread(s3, "t1"))), "False")
    C.Check.str("exhaustion preserves pending work", Bool.show(M.Queue.waiting(s3, "t1")), "True")
    C.Check.str("exhaustion actionable notification", N.Alert.kind(N.Alert.get(s3, "t1")), "fallback-blocked")
    C.Check.str("exhausted chain no spawn", starts(H.Step.outs_of(third)), "")
    C.Check.str("restart event replay keeps choice", selected(M.State.replay(codec(H.Queue.changes(first())), base()), "t1"), "codex/gpt-6.1-sol")
    C.Check.str("desktop push carries exact alert body", PUSH.Push.alert.body(post(), "t1", N.Alert.get(post(), "t1")), N.Alert.rtext(N.Alert.get(post(), "t1")))
    C.Check.str("mobile alert carries thread deeplink", J.Json.get_str(Maybe.default(&2, J.Json, List.head(&2, J.Json, PN.Note.ralert.q(N.Alert.get(post(), "t1"), "t1", "Worker", Nil{}, "", Nil{})), J.Null{}), "thread", ""), "t1")
    +auth : H.Step = H.Hub.agent_line(M.State.replay([M.ProviderSet{"t1", "codex", "gpt-6.1-sol", 4n}], base()), 30n, "t1", "{\"type\":\"turn.failed\",\"message\":\"codex was not started: the login keyring is locked\"}")
    +as : M.State = fold(M.State.replay([M.ProviderSet{"t1", "codex", "gpt-6.1-sol", 4n}], base()), auth)
    C.Check.str("keyring holds not fails", N.Alert.kind(N.Alert.get(as, "t1")), "auth-blocked")
    C.Check.str("keyring never changes provider", selected(as, "t1"), "codex/gpt-6.1-sol")
    C.Check.str("keyring saves exact unsent prompt", M.Queue.of(as, "t1"), "Build once")
    C.Check.str("locked probe never resumes", starts(H.Step.outs_of(H.Auth.recover(False{}, as, 31n))), "")
    +res : H.Step = H.Auth.recover(True{}, as, 32n)
    +rs : M.State = fold(as, res)
    C.Check.str("unlock resumes once", Bool.show(String.contains(starts(H.Step.outs_of(res)), "Build once")), "True")
    C.Check.str("second unlock probe cannot duplicate", starts(H.Step.outs_of(H.Auth.recover(True{}, rs, 33n))), "")
    +authbot : H.Step = H.Auth.hold(as, 31n, "bot", "bot input", "login keyring is locked")
    +both : M.State = fold(as, authbot)
    C.Check.str("one account unlock alert for many threads", Nat.show(alerts(H.Queue.changes(authbot))), "0")
    +one : H.Step = H.Auth.recover(True{}, both, 35n)
    +left : M.State = fold(both, one)
    C.Check.str("unlock releases only one queued recovery", Bool.show(H.Auth.any(H.Rec.threads(left), left)), "True")
    C.Check.str("unlock episode remains deduped while backlog held", M.Setting.get(left, "auth.alerted.keyring", ""), "held")
    +cancelled : M.State = fold(as, H.Hub.interrupt(as, 34n, 1, "stop", "t1"))
    C.Check.str("Stop cancels automatic unlock recovery", starts(H.Step.outs_of(H.Auth.recover(True{}, cancelled, 35n))), "")
    +manual : M.State = fold(post(), H.Fallback.after(post(), caps(), 40n, H.Step{[M.ProviderSet{"t1", "codex", "test-next", 40n}], Nil{}}))
    C.Check.str("explicit model choice remains unchanged by hierarchy", selected(manual, "t1"), "codex/test-next")
    C.Check.str("manual choice clears original fallback intent", M.Setting.get(manual, "fallback.intent.t1", "missing"), "")
    +mailstate : M.State = M.State.replay([M.SettingSet{"mail.on", "on", 1n}, M.RunSet{"t1", "b", "conv-id", "mail-id", 1n}], base())
    +mailpost : M.State = fold(mailstate, fail(mailstate, 15n, "t1", limit()))
    C.Check.str("fallback preserves run/thread/mail identity", run_identity(MR.Run.of(mailpost, "t1")), "t1/b/conv-id/mail-id")
    C.Check.str("fallback keeps mail run live", Bool.show(Maybe.is_some(&2, ML.Run, MR.Run.of(mailpost, "t1"))), "True")
