import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/hub.bend as H
import ../src/core/mail.bend as ML
import ../src/core/mailbody.bend as MB

def base(+provider: String, +model: String, uber: Bool) -> M.State:
  M.State.replay([M.ThreadCreated{"foh", "", "Beta", M.Local{}, "", "/tmp/mail-model", provider, model, 1n},
    M.BotSet{"beta", "Beta", "foh", 0n, "", 1n}, M.SettingSet{"mail.on", "on", 1n},
    M.SettingSet{"uber.bot", Bool.pick(String, uber, "beta", ""), 1n}], M.State.empty())

def override(s: M.State, model: String) -> M.State:
  M.State.apply(s, M.SettingSet{"run.model.beta", model, 2n})

def body(+id: String) -> MB.Body:
  MB.Body{ML.Conv{"conv", "Question", "pair", ["thread:worker", "bot:beta"], "", Nil{}, 1n, 2n, 1n},
    ML.Mail{id, "conv", "thread:worker", "bot:beta", "thread", "", "act", "question", 8n, 3n}, "question"}

def runstarts(os: List<&2, H.Out>) -> Nat:
  match os:
    case Nil{}: 0n
    case H.RunStart{_, _, _} <> t: 1n+runstarts(t)
    case _ <> t: runstarts(t)

def created(cs: List<&2, M.Change>) -> Nat:
  match cs:
    case Nil{}: 0n
    case M.ThreadCreated{_, _, _, _, _, _, _, _, _} <> t: 1n+created(t)
    case _ <> t: created(t)

def modes(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}: "missing"
    case M.ModesSet{_, _, _, model, _} <> _: model
    case _ <> t: modes(t)

def agent(os: List<&2, H.Out>) -> String:
  match os:
    case Nil{}: "missing"
    case H.AgentStart{_, _, provider, model, session, _, _} <> _: provider ++ "|" ++ model ++ "|" ++ session
    case H.CodexTurn{_, _, model, session, _, _} <> _: "codex|" ++ model ++ "|" ++ session
    case H.MuseTurn{_, _, model, session, _, _} <> _: "muse|" ++ model ++ "|" ++ session
    case H.GrokTurn{_, _, model, session, _, _} <> _: "grok|" ++ model ++ "|" ++ session
    case _ <> t: agent(t)

def check(+label: String, +s: M.State, +provider: String, +model: String) -> IO(Unit):
  do IO<Unit>:
    +step : H.Step = H.Mail.route_bot(s, 3n, body("m1"), "beta")
    +after : M.State = M.State.replay(H.Queue.changes(step), s)
    +th : String = ML.Box.live(M.State.mail(after), "beta", "conv")
    +start : H.Step = H.Run.start(after, 4n, th, "brief", [body("m1")])
    +live : M.State = M.State.replay(H.Queue.changes(start), after)
    +again : H.Step = H.Mail.route_bot(live, 5n, body("m2"), "beta")
    C.Check.str(label ++ " resolved", H.Run.model(s, "beta"), model)
    C.Check.str(label ++ " created provider", H.Hub.provider(H.Hub.thread(after, th)), provider)
    C.Check.str(label ++ " created model", H.Turn.model(H.Hub.thread(after, th)), model)
    C.Check.str(label ++ " modes same model", modes(H.Queue.changes(step)), model)
    C.Check.str(label ++ " actual fresh start", agent(H.Step.outs_of(start)), provider ++ "|" ++ model ++ "|")
    C.Check.str(label ++ " one start", Nat.show(runstarts(H.Step.outs_of(step))), "1")
    C.Check.str(label ++ " no duplicate start", Nat.show(runstarts(H.Step.outs_of(again))), "0")
    C.Check.str(label ++ " no duplicate thread", Nat.show(created(H.Queue.changes(again))), "0")
    C.Check.str(label ++ " one live conversation run", Nat.show(ML.Box.live_n(M.State.mail(M.State.replay(H.Queue.changes(again), live)), "beta")), "1")
    C.Check.str(label ++ " parent model unchanged", H.Turn.model(H.Hub.thread(after, "foh")), H.Turn.model(H.Hub.thread(s, "foh")))
    C.Check.str(label ++ " override unchanged", M.Setting.get(after, "run.model.beta", "unset"), M.Setting.get(s, "run.model.beta", "unset"))

def live_preserved() -> IO(Unit):
  do IO<Unit>:
    +s : M.State = M.State.replay([M.ThreadCreated{"existing", "", "Already working", M.Local{}, "", "/tmp/mail-model", "codex", "gpt-6.1-sol", 2n},
      M.RunSet{"existing", "beta", "conv", "m0", 2n}, M.TurnChanged{"existing", M.Running{}, 2n},
      M.ProviderSet{"foh", "muse", "muse-wonder-full-1.0", 3n}, M.SettingSet{"run.model.beta", "opus", 3n}], base("codex", "gpt-6.1-sol", True{}))
    +step : H.Step = H.Mail.route_bot(s, 4n, body("m1"), "beta")
    +after : M.State = M.State.replay(H.Queue.changes(step), s)
    C.Check.str("live parent switch no new run", Nat.show(runstarts(H.Step.outs_of(step))), "0")
    C.Check.str("live parent switch no new thread", Nat.show(created(H.Queue.changes(step))), "0")
    C.Check.str("live provider unchanged", H.Hub.provider(H.Hub.thread(after, "existing")), "codex")
    C.Check.str("live model unchanged", H.Turn.model(H.Hub.thread(after, "existing")), "gpt-6.1-sol")
    C.Check.str("live conversation unchanged", ML.Box.live(M.State.mail(after), "beta", "conv"), "existing")

def main() -> IO(Unit):
  do IO<Unit>:
    check("codex uber unset", base("codex", "gpt-6.1-sol", True{}), "codex", "gpt-6.1-sol")
    check("codex worker unset", base("codex", "gpt-6.1-sol", False{}), "codex", "gpt-6.1-sol")
    check("codex stale opus", override(base("codex", "gpt-6.1-sol", True{}), "opus"), "codex", "gpt-6.1-sol")
    check("codex stale sonnet", override(base("codex", "gpt-6.1-sol", False{}), "sonnet"), "codex", "gpt-6.1-sol")
    check("codex stale muse", override(base("codex", "gpt-6.1-sol", True{}), "muse-wonder-full-1.0"), "codex", "gpt-6.1-sol")
    check("codex explicit", override(base("codex", "gpt-6.1-sol", True{}), "gpt-6-astra"), "codex", "gpt-6-astra")
    check("codex blank", override(base("codex", "gpt-6.1-sol", True{}), "  "), "codex", "gpt-6.1-sol")
    check("codex unknown custom", override(base("codex", "gpt-6.1-sol", True{}), "local-custom"), "codex", "local-custom")
    check("codex parent stale", base("codex", "opus", True{}), "codex", "")
    check("codex parent empty", base("codex", "", True{}), "codex", "")
    check("muse uber exact full", base("muse", "muse-wonder-full-1.0", True{}), "muse", "muse-wonder-full-1.0")
    check("muse worker unset", base("muse", "muse-wonder-1.0", False{}), "muse", "muse-wonder-1.0")
    check("muse stale opus", override(base("muse", "muse-wonder-full-1.0", True{}), "opus"), "muse", "muse-wonder-full-1.0")
    check("muse stale codex", override(base("muse", "muse-wonder-full-1.0", True{}), "gpt-6.1-sol"), "muse", "muse-wonder-full-1.0")
    check("muse explicit", override(base("muse", "muse-wonder-full-1.0", True{}), "muse-spark-1.3"), "muse", "muse-spark-1.3")
    check("muse whitespace", override(base("muse", "muse-wonder-full-1.0", True{}), " muse-wonder-full-1.0 "), "muse", "muse-wonder-full-1.0")
    check("muse parent stale", base("muse", "sonnet", False{}), "muse", "")
    check("claude uber keeps opus", base("claude", "claude-opus-5-5", True{}), "claude", "opus")
    check("claude worker keeps sonnet", base("claude", "opus", False{}), "claude", "sonnet")
    check("claude stale codex", override(base("claude", "opus", False{}), "gpt-6.1-sol"), "claude", "sonnet")
    check("claude stale muse", override(base("claude", "opus", True{}), "muse-wonder-full-1.0"), "claude", "opus")
    check("claude explicit", override(base("claude", "opus", False{}), "claude-opus-5-5"), "claude", "claude-opus-5-5")
    live_preserved()
