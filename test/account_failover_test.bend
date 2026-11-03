import Base
import ./check.bend as C
import ../src/core/account-fallback.bend as F
import ../src/core/accounts.bend as A
import ../src/core/fallback.bend as FB
import ../src/core/hub.bend as H
import ../src/core/model.bend as M
import ../src/core/json.bend as J

# Only fictional public account metadata and synthetic protocol errors.
def model(+p: String) -> String:
  Bool.pick(String, String.eq(p, "claude"), "sonnet", "gpt-6.1-sol")

def profile(+p: String, +id: String, used: String) -> J.Json:
  J.Json.obj([J.KV{"id", J.Str{id}}, J.KV{"provider", J.Str{p}}, J.KV{"label", J.Str{"Account " ++ id}},
    J.KV{"epoch", J.Json.nat(1n)}, J.KV{"state", J.Str{"connected"}},
    J.KV{"catalog", J.Json.arr([J.Json.obj([J.KV{"id", J.Str{model(p)}}])])},
    J.KV{"allowance", J.Json.obj([J.KV{"state", J.Str{"ok"}}, J.KV{"at", J.Json.nat(100n)},
      J.KV{"windows", J.Json.arr([J.Json.obj([J.KV{"scope", J.Str{"provider-pool"}}, J.KV{"shared", J.Flag{True{}}},
        J.KV{"used_percent", A.Accounts.parse(used)}, J.KV{"resets_at", J.Json.nat(200n)}])])}])}])

def inventory(ps: List<&2, J.Json>) -> String:
  J.Json.show(J.Json.obj([J.KV{"profiles", J.Json.arr(ps)}]))

def base(+p: String) -> M.State:
  M.State.replay([M.ProjectCreated{"p", "Fixture", "/tmp/account-failover", 1n},
    M.ThreadCreated{"t", "p", "Fixture", M.Local{}, "", "/tmp/account-failover", p, model(p), 1n},
    M.SettingSet{"accounts.inventory", inventory([profile(p, "A", "95"), profile(p, "B", "80"), profile(p, "C", "40")]), 100n},
    M.SettingSet{A.Accounts.binding.key("t", p), A.Accounts.binding.show("A", 1n), 1n},
    M.SettingSet{"account.default." ++ p, A.Accounts.binding.show("A", 1n), 1n},
    M.SettingSet{A.Accounts.session.key("t", p, "C", 1n), "native-C", 1n},
    M.SessionSet{"t", "native-A", 1n}, M.MessagePosted{"t", "original", M.User{}, "Keep this exact request\nwith its second line", 2n},
    M.ActivityLogged{"t", "tool-result", "tool", "Bash", "Created the requested file", 3n},
    M.SettingSet{"fallback.input.t", "Keep this exact request\nwith its second line", 2n},
    M.TurnChanged{"t", M.Running{}, 3n}], M.State.empty())

def caps() -> J.Json:
  A.Accounts.parse("{\"claude\":[\"sonnet\",\"opus\"],\"codex\":[\"gpt-6.1-sol\"]}")

def limit() -> String:
  "{\"type\":\"turn.failed\",\"error\":{\"code\":\"quota_exceeded\",\"scope\":\"account\",\"message\":\"Usage limit reached\",\"resets_at\":200}}"

def fail(+s: M.State, +now: Nat, line: String) -> H.Step:
  H.Fallback.after(s, caps(), now, H.Hub.agent_line(s, now, "t", line))

def fold(+s: M.State, step: H.Step) -> M.State:
  M.State.replay(H.Queue.changes(step), s)

def one(+p: String) -> M.State:
  fold(base(p), fail(base(p), 120n, limit()))

def starts(os: List<&2, H.Out>) -> Nat:
  match os:
    case Nil{}: 0n
    case H.AgentStart{_, _, _, _, _, _, _} <> t: 1n+starts(t)
    case H.CodexTurn{_, _, _, _, _, _} <> t: 1n+starts(t)
    case _ <> t: starts(t)

def stops(os: List<&2, H.Out>) -> Nat:
  match os:
    case Nil{}: 0n
    case H.AgentStop{_} <> t: 1n+stops(t)
    case _ <> t: stops(t)

def input(os: List<&2, H.Out>) -> String:
  match os:
    case Nil{}: ""
    case H.CodexTurn{_, _, _, _, _, text} <> t: text ++ input(t)
    case H.AgentSend{_, line} <> t: line ++ input(t)
    case _ <> t: input(t)

def notes(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}: ""
    case M.ActivityLogged{_, _, _, "account-switch", text, _} <> t: text ++ notes(t)
    case _ <> t: notes(t)

def selected(+s: M.State, +p: String) -> String:
  A.Accounts.id(s, "t", p)

def pick(x: Maybe<&2, J.Json>) -> String:
  match x:
    case None{}: "none"
    case Some{p}: J.Json.get_str(p, "id", "")

def held(+s: M.State, +p: String, +id: String, reset: Nat) -> M.State:
  M.State.replay([M.SettingSet{F.Hold.key(p, id, 1n), FB.Limit.json(FB.Limit{p, "Fixture limit", reset}, 120n), 120n}], s)

def allheld(+p: String, +reset: Nat) -> M.State:
  held(held(held(base(p), p, "A", reset), p, "B", reset), p, "C", reset)

def launch(+s: M.State, now: Nat) -> H.Step:
  H.Fallback.after(s, caps(), now, H.Step.outs(H.Hub.run(H.Hub.thread(s, "t"), "/tmp/account-failover", "New request", "uuid")))

def atrest(+s: M.State) -> M.State:
  M.State.apply(s, M.TurnChanged{"t", M.Completed{}, 150n})

def provider_roundtrip() -> M.State:
  M.State.replay([M.SettingSet{A.Accounts.binding.key("t", "codex"), A.Accounts.binding.show("B", 1n), 110n},
    M.SettingSet{A.Accounts.session.key("t", "codex", "B", 1n), "native-B-old", 110n},
    M.SessionSet{"t", "native-B-new", 111n}, M.ProviderSet{"t", "claude", "sonnet", 112n}], base("codex"))

def opus_accounts() -> M.State:
  +c = A.Accounts.put(profile("claude", "C", "80"), "catalog", J.Json.arr([J.Json.obj([J.KV{"id", J.Str{"opus"}}])]))
  M.State.setting(M.State.setting(M.State.setting(base("claude"), "accounts.inventory", inventory([profile("claude", "A", "95"), profile("claude", "B", "40"), c])),
    "fallback.chain", "claude/sonnet,claude/opus"), "fallback.on", "on")

def ask_open(xs: List<&2, M.Ask>, +id: String) -> Bool:
  match xs:
    case Nil{}: False{}
    case M.Ask{+k, _, _, _, _, _, op, _} <> t: (String.eq(k, id) && op) || ask_open(t, id)

def pending_exit() -> M.State:
  +s = M.State.replay([M.AskOpened{"old-ask", "t", "approval", "Bash", "old actor approval", "", 110n}, M.WorkSet{"t", "bg", "bg-task\tworking", 110n}], base("codex"))
  M.State.replay(H.AccountFallback.pending.save(s, 120n, "t", FB.Limit{"codex", "Usage limit reached", 200n}, J.Null{}), s)

# Incident reproduction: legacy Default has a cached weekly provider hold,
# while a connected Claude_Backplane profile has no allowance reading yet.
def legacy(+p: String) -> M.State:
  M.State.replay([M.SettingSet{A.Accounts.binding.key("t", p), A.Accounts.binding.show("", 0n), 110n},
    M.SettingSet{"accounts.inventory", inventory([A.Accounts.put(profile(p, "Claude_Backplane", "40"), "allowance", J.Null{})]), 110n},
    M.SettingSet{FB.Limit.key(p), FB.Limit.json(FB.Limit{p, "Weekly limit reached", 200n}, 110n), 110n},
    M.SettingSet{"fallback.on", "on", 110n}, M.SettingSet{"fallback.chain", p ++ "/" ++ model(p) ++ ",codex/gpt-6.1-sol", 110n}], base(p))

def legacy_dated() -> M.State:
  M.State.replay([M.SettingSet{FB.Limit.key("claude"), FB.Limit.json(FB.Limit{"claude", "You've hit your weekly limit · resets Nov 4, 12pm (America/New_York)", 0n}, 1793635200n), 1793635200n},
    M.SettingSet{"tz.rule.America/New_York", "EST5EDT,M3.2.0,M11.1.0", 1793635200n}], legacy("claude"))

def permille() -> J.Json:
  A.Accounts.put(profile("claude", "normalized", "null"), "allowance", A.Accounts.parse("{\"provider\":\"claude\",\"state\":\"ok\",\"at\":100,\"windows\":[{\"scope\":\"shared\",\"used_permille\":125,\"resets_at\":200}]}"))

def main() -> IO(Unit):
  do IO<Unit>:
    +legacyStep : H.Step = launch(legacy("claude"), 120n)
    +legacyNext : M.State = fold(legacy("claude"), legacyStep)
    C.Check.str("legacy cached quota tries named Claude before Codex", M.Thread.provider(H.Hub.thread(legacyNext, "t")), "claude")
    C.Check.str("legacy cached quota selects connected unknown allowance account", selected(legacyNext, "claude"), "Claude_Backplane")
    C.Check.str("legacy original request launches exactly once", Nat.show(starts(H.Step.outs_of(legacyStep))), "1")
    C.Check.str("legacy native session never crosses account identities", M.Switch.sess(H.Hub.thread(legacyNext, "t")), "")
    C.Check.str("legacy imported hold retains reset for restoration", Nat.show(F.Hold.reset(legacyNext, "claude", "", 0n)), "200")
    C.Check.str("legacy restores default at reported reset", selected(fold(atrest(legacyNext), H.ProviderResume.tick(atrest(legacyNext), 200n)), "claude"), "")
    C.Check.str("cross-provider direct guard still tries available same-provider account", M.Thread.provider(H.Hub.thread(fold(legacy("claude"), H.AccountFallback.cross(True{}, legacy("claude"), caps(), 120n, "t", "/tmp/account-failover", "request", "uuid")), "t")), "claude")
    C.Check.str("normalized permille allowance ranks remaining correctly", Nat.show(U32.to_nat(Maybe.default(&2, U32, F.Usage.score(permille(), 120n, "sonnet"), 0))), "87500")
    +zero : M.State = M.State.setting(base("claude"), "accounts.inventory", inventory([profile("claude", "empty", "100"), profile("claude", "ready", "20")]))
    C.Check.str("fresh zero allowance account is excluded", pick(F.Pick.account(zero, 120n, "claude", "sonnet")), "ready")
    C.Check.str("provider fallback gate refuses while a same-provider account is available", Bool.show(F.Pick.may_cross(zero, 120n, "claude", "sonnet")), "False")
    C.Check.str("provider fallback gate opens after all reported allowances exhausted", Bool.show(F.Pick.may_cross(M.State.setting(zero, "accounts.inventory", inventory([profile("claude", "empty", "100")])), 120n, "claude", "sonnet")), "True")
    +dated : M.State = M.State.setting(base("claude"), "tz.rule.America/New_York", "EST5EDT,M3.2.0,M11.1.0")
    C.Check.str("weekly named-date reset keeps provider wall time", Nat.show(H.Reset.words(dated, 1793635200n, "You've hit your weekly limit · resets Nov 4, 12pm (America/New_York)")), "1793811600")
    +datedStep : H.Step = launch(legacy_dated(), 1793674000n)
    +datedNext : M.State = fold(legacy_dated(), datedStep)
    C.Check.str("cached legacy weekly words recover the account reset", Nat.show(F.Hold.reset(datedNext, "claude", "", 0n)), "1793811600")
    C.Check.str("cached legacy provider hold also records recovered reset", Nat.show(J.Json.get_nat(FB.Limit.record(datedNext, "claude"), "reset", 0n)), "1793811600")
    C.Check.str("cached weekly account restores after the dated reset", selected(fold(atrest(datedNext), H.ProviderResume.tick(atrest(datedNext), 1793811600n)), "claude"), "")
    +claude : H.Step = fail(base("claude"), 120n, limit())
    +codex : H.Step = fail(base("codex"), 120n, limit())
    C.Check.str("Claude retries most-remaining available connected account", selected(one("claude"), "claude"), "C")
    C.Check.str("Codex retries most-remaining available connected account", selected(one("codex"), "codex"), "C")
    C.Check.str("same provider retained", M.Thread.provider(H.Hub.thread(one("claude"), "t")), "claude")
    C.Check.str("one replacement launch", Nat.show(starts(H.Step.outs_of(claude))), "1")
    C.Check.str("one stop at the account boundary", Nat.show(stops(H.Step.outs_of(claude))), "1")
    C.Check.str("Codex replacement starts once", Nat.show(starts(H.Step.outs_of(codex))), "1")
    C.Check.str("source native session saved under its account", A.Accounts.session(one("claude"), "t", "claude", "A", 1n), "native-A")
    C.Check.str("destination resumes only its own native session", M.Switch.sess(H.Hub.thread(one("claude"), "t")), "native-C")
    C.Check.str("default preference is not changed", M.Setting.get(one("claude"), "account.default.claude", ""), A.Accounts.binding.show("A", 1n))
    C.Check.str("original request retained exactly", M.Setting.get(one("claude"), "fallback.input.t", ""), "Keep this exact request\nwith its second line")
    C.Check.str("unfinished request reaches replacement", Bool.show(String.contains(input(H.Step.outs_of(codex)), "Keep this exact request\nwith its second line")), "True")
    C.Check.str("completed tools reach replacement", Bool.show(String.contains(input(H.Step.outs_of(codex)), "Created the requested file")), "True")
    C.Check.str("visible note names account", Bool.show(String.contains(notes(H.Queue.changes(claude)), "Account C")), "True")
    C.Check.str("only failed account is held", Bool.show(F.Hold.active(one("claude"), "claude", "A", 1n, 121n)), "True")
    C.Check.str("shared pool metadata never holds sibling profile", Bool.show(F.Hold.active(one("claude"), "claude", "B", 1n, 121n)), "False")
    C.Check.str("hold ends exactly at provider reset", Bool.show(F.Hold.active(one("claude"), "claude", "A", 1n, 200n)), "False")
    C.Check.str("replacement identity never inherits old hold", Bool.show(F.Hold.active(one("claude"), "claude", "A", 2n, 121n)), "False")
    C.Check.str("no provider-wide hold for one exhausted account", Bool.show(FB.Limit.active(one("claude"), "claude", 121n)), "False")
    +unknown : M.State = M.State.setting(held(base("claude"), "claude", "A", 200n), "accounts.inventory", inventory([profile("claude", "B", "null"), profile("claude", "C", "80")]))
    C.Check.str("known remaining allowance precedes missing usage", pick(F.Pick.account(unknown, 120n, "claude", "sonnet")), "C")
    C.Check.str("stale utilization uses saved account hierarchy", pick(F.Pick.account(unknown, 500n, "claude", "sonnet")), "B")
    C.Check.str("wrong provider is excluded", pick(F.Pick.account(base("claude"), 120n, "codex", "gpt-6.1-sol")), "none")
    C.Check.str("unsupported model is excluded", pick(F.Pick.account(base("claude"), 120n, "claude", "unavailable-model")), "none")
    +unchecked : M.State = M.State.setting(base("claude"), "accounts.inventory", inventory([A.Accounts.put(profile("claude", "B", "80"), "state", J.Str{"unchecked"})]))
    C.Check.str("unverified sign-in is excluded", pick(F.Pick.account(unchecked, 120n, "claude", "sonnet")), "none")
    +ordinary : M.State = fold(base("codex"), fail(base("codex"), 120n, "{\"type\":\"turn.failed\",\"error\":{\"message\":\"connection reset\"}}"))
    C.Check.str("network error preserves explicit account", selected(ordinary, "codex"), "A")
    C.Check.str("network error creates no hold", Bool.show(F.Hold.active(ordinary, "codex", "A", 1n, 121n)), "False")
    +auth : M.State = fold(base("claude"), fail(base("claude"), 120n, "{\"type\":\"turn.failed\",\"error\":{\"code\":\"quota_exceeded\",\"message\":\"authentication login required\"}}"))
    C.Check.str("authentication errors never rotate accounts", selected(auth, "claude"), "A")
    +second : M.State = fold(one("claude"), fail(one("claude"), 125n, limit()))
    C.Check.str("second exhaustion skips held accounts", selected(second, "claude"), "B")
    +chain : M.State = M.State.setting(M.State.setting(second, "fallback.chain", "claude/sonnet,codex/gpt-6.1-sol"), "fallback.on", "on")
    +cross : M.State = fold(chain, fail(chain, 130n, limit()))
    C.Check.str("cross-provider chain after all accounts exhausted", M.Thread.provider(H.Hub.thread(cross, "t")), "codex")
    C.Check.str("cross-provider never resumes Claude native session", M.Switch.sess(H.Hub.thread(cross, "t")), "")
    +returnB : M.State = M.State.replay(H.AccountFallback.rebind(provider_roundtrip(), 140n, "t", "codex", "gpt-6.1-sol", A.Accounts.binding.show("B", 1n)), provider_roundtrip())
    C.Check.str("same destination binding retains newest provider session", M.Switch.sess(H.Hub.thread(returnB, "t")), "native-B-new")
    +opusFirst : M.State = fold(opus_accounts(), fail(opus_accounts(), 120n, limit()))
    +opusNext : M.State = fold(opusFirst, fail(opusFirst, 125n, limit()))
    C.Check.str("same-provider model fallback selects model-capable account", selected(opusNext, "claude"), "C")
    C.Check.str("same-provider model fallback changes actual launch model", FB.Target.model(H.Fallback.target(H.Hub.thread(opusNext, "t"))), "opus")
    +paused : M.State = fold(allheld("claude", 200n), launch(allheld("claude", 200n), 130n))
    C.Check.str("all limited without hierarchy preserves request", Bool.show(String.contains(M.Queue.of(paused, "t"), "New request")), "True")
    C.Check.str("all limited schedules actual reset", J.Json.get_str(H.ProviderResume.record(paused, "t"), "target", ""), "claude/sonnet")
    +restored : M.State = fold(atrest(one("claude")), H.ProviderResume.tick(atrest(one("claude")), 200n))
    C.Check.str("preferred account returns on reset between turns", selected(restored, "claude"), "A")
    C.Check.str("restoration resumes original account session", M.Switch.sess(H.Hub.thread(restored, "t")), "native-A")
    +cWork : M.State = atrest(M.State.apply(one("claude"), M.ActivityLogged{"t", "c-work", "tool", "Bash", "C completed after failover", 149n}))
    +tickReturned : M.State = fold(cWork, H.ProviderResume.tick(cWork, 200n))
    C.Check.str("idle restoration queues account-missed tool context", Bool.show(String.contains(M.Queue.of(tickReturned, "t"), "C completed after failover")), "True")
    +afterTick : H.Step = H.Fallback.after(tickReturned, caps(), 201n, H.Hub.post(tickReturned, 201n, "t", "after-reset", M.User{}, "Next task"))
    C.Check.str("next ordinary request receives reset handoff", Bool.show(String.contains(input(H.Step.outs_of(afterTick)), "C completed after failover")), "True")
    C.Check.str("running account not interrupted by reset", selected(fold(one("claude"), H.ProviderResume.tick(one("claude"), 200n)), "claude"), "C")
    C.Check.str("next request restores preferred account at safe boundary", selected(fold(one("claude"), launch(one("claude"), 200n)), "claude"), "A")
    +manual : M.State = M.State.replay(H.AccountFallback.forget("t", 150n), atrest(one("claude")))
    C.Check.str("new explicit account preference prevents automatic return", selected(fold(manual, H.ProviderResume.tick(manual, 200n)), "claude"), "C")
    +unknownHold : M.State = held(base("claude"), "claude", "A", 0n)
    C.Check.str("unknown reset stays held", Bool.show(F.Hold.active(unknownHold, "claude", "A", 1n, 500n)), "True")
    +exitStep : H.Step = H.Fallback.after(pending_exit(), caps(), 121n, H.Hub.agent_gone(pending_exit(), 121n, "t", 1))
    +afterExit : M.State = fold(pending_exit(), exitStep)
    C.Check.str("quota followed by process exit continues request", selected(afterExit, "codex"), "C")
    C.Check.str("replaced actor approval is retired", Bool.show(ask_open(M.Asks.all(afterExit), "old-ask")), "False")
    C.Check.str("replaced actor background rows are cleared", Nat.show(List.length(&2, String, M.Bg.of(afterExit, "t"))), "0")
    C.Check.str("quota process exit has one replacement launch", Nat.show(starts(H.Step.outs_of(exitStep))), "1")
    +archived : M.State = M.State.apply(atrest(one("claude")), M.ThreadArchived{"t", True{}, 150n})
    +deleted : M.State = M.State.apply(atrest(one("claude")), M.ThreadDeleted{"t", 150n})
    +configHeld : M.State = M.State.setting(atrest(one("claude")), M.Config.key("t"), "on")
    +dormant : M.State = M.State.setting(atrest(one("claude")), M.Dormant.key("p"), "on")
    C.Check.str("reset does not mutate archived account selection", selected(fold(archived, H.ProviderResume.tick(archived, 200n)), "claude"), "C")
    C.Check.str("reset does not mutate deleted account selection", selected(fold(deleted, H.ProviderResume.tick(deleted, 200n)), "claude"), "C")
    C.Check.str("reset respects configuration-held work", selected(fold(configHeld, H.ProviderResume.tick(configHeld, 200n)), "claude"), "C")
    C.Check.str("reset respects dormant project", selected(fold(dormant, H.ProviderResume.tick(dormant, 200n)), "claude"), "C")
    +modelHold : M.State = M.State.replay(F.Hold.mark(base("claude"), 120n, "t", FB.Limit{"claude/sonnet", "Model limit", 200n}), base("claude"))
    C.Check.str("model-only exhaustion holds selected model", Bool.show(F.Hold.current(modelHold, "claude", "A", 1n, "sonnet", 121n)), "True")
    C.Check.str("model-only exhaustion leaves another model available", Bool.show(F.Hold.current(modelHold, "claude", "A", 1n, "opus", 121n)), "False")
    C.Check.str("model-only exhaustion is not an account-wide hold", Bool.show(F.Hold.active(modelHold, "claude", "A", 1n, 121n)), "False")
    +unknownAll : M.State = allheld("codex", 0n)
    +unknownPaused : M.State = fold(unknownAll, launch(unknownAll, 130n))
    +cleared : M.State = fold(unknownPaused, H.AccountFallback.clear(unknownPaused, 140n, "codex", "B", 1n))
    C.Check.str("clear releases only selected profile generation", Bool.show(F.Hold.active(cleared, "codex", "B", 1n, 140n)), "False")
    C.Check.str("clear keeps other account holds", Bool.show(F.Hold.active(cleared, "codex", "A", 1n, 140n)), "True")
    +retry : H.Step = H.Fallback.after(cleared, caps(), 141n, H.ProviderResume.tick(cleared, 141n))
    C.Check.str("clearing unknown hold retries preserved request", Bool.show(String.contains(input(H.Step.outs_of(retry)), "New request")), "True")
    C.Check.str("cleared profile can receive retry", selected(fold(cleared, retry), "codex"), "B")
    +future : J.Json = profile("future-provider", "future", "50")
    C.Check.str("planner supports any provider with public connected profiles", pick(F.Pick.account(M.State.setting(base("claude"), "accounts.inventory", inventory([future])), 120n, "future-provider", "gpt-6.1-sol")), "future")
