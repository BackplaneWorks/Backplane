import Base
import ./check.bend as C
import ../src/core/settingsproposal.bend as P
import ../src/core/mcp.bend as MC
import ../src/core/hub.bend as H
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/codec.bend as CD
import ./mailtriage_test.bend as MT

def caps() -> J.Json:
  J.Json.obj([J.KV{"claude", J.Json.arr([J.Str{"sonnet"}])}, J.KV{"codex", J.Json.arr([J.Str{"model"}])}])

def base() -> M.State:
  M.State.replay([M.ThreadCreated{"t1", "p", "Test", M.Local{}, "", "/tmp", "claude", "sonnet", 1n}, M.SettingSet{"fallback.on", "off", 1n}, M.SettingSet{"fallback.chain", "claude/sonnet", 1n}], M.State.empty())

def propose(+s: M.State) -> MC.McpOut:
  MC.Mcp.handle(s, 10n, "t1", caps(), "test", J.Json.obj([J.KV{"id", J.Num{"1"}}, J.KV{"method", J.Str{"tools/call"}},
    J.KV{"params", J.Json.obj([J.KV{"name", J.Str{"settings_propose"}}, J.KV{"arguments", J.Json.obj([J.KV{"on", J.Str{"on"}}, J.KV{"chain", J.Str{"claude/sonnet,codex/model"}}])}])}]))

def proposal.changes(r: MC.McpOut) -> List<&2, M.Change>:
  match r:
    case MC.McpOut{_, step, _}: H.Queue.changes(step)

def unsafe.proposal() -> MC.McpOut:
  MC.Mcp.handle(base(), 10n, "t1", caps(), "test", J.Json.obj([J.KV{"id", J.Num{"2"}}, J.KV{"method", J.Str{"tools/call"}},
    J.KV{"params", J.Json.obj([J.KV{"name", J.Str{"settings_propose"}}, J.KV{"arguments", J.Json.obj([J.KV{"on", J.Str{"on"}}, J.KV{"chain", J.Str{"SECRET-FIXTURE-NOT-A-MODEL"}}])}])}]))

def starts_or_stops(os: List<&2, H.Out>) -> Nat:
  match os:
    case Nil{}: 0n
    case H.AgentStart{_, _, _, _, _, _, _} <> rest: 1n+starts_or_stops(rest)
    case H.AgentStop{_} <> rest: 1n+starts_or_stops(rest)
    case H.AgentSend{_, _} <> rest: 1n+starts_or_stops(rest)
    case H.CodexTurn{_, _, _, _, _, _} <> rest: 1n+starts_or_stops(rest)
    case H.GrokTurn{_, _, _, _, _, _} <> rest: 1n+starts_or_stops(rest)
    case H.MuseTurn{_, _, _, _, _, _} <> rest: 1n+starts_or_stops(rest)
    case H.AgyTurn{_, _, _, _, _, _} <> rest: 1n+starts_or_stops(rest)
    case H.OcTurn{_, _, _, _, _, _} <> rest: 1n+starts_or_stops(rest)
    case H.PiTurn{_, _, _, _, _, _} <> rest: 1n+starts_or_stops(rest)
    case H.OmpTurn{_, _, _, _, _, _} <> rest: 1n+starts_or_stops(rest)
    case _ <> rest: starts_or_stops(rest)

def writes(cs: List<&2, M.Change>, +key: String) -> Nat:
  match cs:
    case Nil{}: 0n
    case M.SettingSet{+k, _, _} <> rest: Nat.add(Bool.pick(Nat, String.eq(k, key), 1n, 0n), writes(rest, key))
    case _ <> rest: writes(rest, key)

def asked() -> M.State:
  M.State.replay(proposal.changes(propose(base())), base())

def two.asked() -> M.State:
  M.State.replay(proposal.changes(propose(asked())), asked())

def second.id() -> String:
  H.Hub.id("settings", 10n, H.Hub.seq(asked()))

def ask.answer.of(a: M.Ask) -> String:
  match a:
    case M.Ask{_, _, _, _, _, _, _, answer}: answer

def ask.answer(s: M.State, id: String) -> String:
  ask.answer.of(H.Ask.find(M.Asks.all(s), id))

def askid() -> String:
  H.Hub.id("settings", 10n, H.Hub.seq(base()))

def decide(+s: M.State, +now: Nat, +answer: String) -> H.Step:
  H.Fallback.validate(s, caps(), now, H.Hub.request(s, now, 1, J.Json.obj([J.KV{"id", J.Str{"1"}}, J.KV{"m", J.Str{"ask.answer"}},
    J.KV{"p", J.Json.obj([J.KV{"id", J.Str{askid()}}, J.KV{"answer", J.Str{answer}}])}])))

def applied(s: M.State, step: H.Step) -> M.State:
  M.State.replay(H.Queue.changes(step), s)

def write(+s: M.State, +value: String) -> M.State:
  applied(s, H.Fallback.validate(s, caps(), 20n, H.Hub.request(s, 20n, 2, J.Json.obj([J.KV{"id", J.Str{"2"}}, J.KV{"m", J.Str{"setting.set"}},
    J.KV{"p", J.Json.obj([J.KV{"key", J.Str{"fallback.chain"}}, J.KV{"value", J.Str{value}}])}]))))

def managed.alert() -> String:
  +s = MT.base()
  +cs = {[M.AskOpened{"settings-test", "worker", "approval", "settings_proposal", "private proposal marker", "", 20n}] : List<&2, M.Change>}
  M.Setting.get(M.State.replay(H.Alerts.changes(s, cs), s), "alert.worker", "")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("tool proposes without changing fallback", M.Setting.get(asked(), "fallback.on", "off"), "off")
    C.Check.str("approval applies proposed enable", M.Setting.get(applied(asked(), decide(asked(), 30n, "accept")), "fallback.on", "off"), "on")
    C.Check.str("approval applies proposed chain", M.Setting.get(applied(asked(), decide(asked(), 30n, "accept")), "fallback.chain", ""), "claude/sonnet,codex/model")
    C.Check.str("rejection preserves setting", M.Setting.get(applied(asked(), decide(asked(), 30n, "decline")), "fallback.on", "off"), "off")
    C.Check.str("session-wide approval cannot apply", M.Setting.get(applied(asked(), decide(asked(), 30n, "session")), "fallback.on", "off"), "off")
    C.Check.str("expired approval preserves setting", M.Setting.get(applied(asked(), decide(asked(), 610n, "accept")), "fallback.on", "off"), "off")
    C.Check.str("write and restore cannot approve", M.Setting.get(applied(write(write(asked(), "codex/model"), "claude/sonnet"), decide(write(write(asked(), "codex/model"), "claude/sonnet"), 30n, "accept")), "fallback.on", "off"), "off")
    C.Check.str("replayed answer emits no changes", Nat.show(List.length(&2, M.Change, H.Queue.changes(decide(applied(asked(), decide(asked(), 30n, "accept")), 31n, "accept")))), "0")
    C.Check.str("managed proposal does not spawn mail triage", Nat.show(List.length(&2, H.Out, H.Step.outs_of(H.Run.after(MT.base(), 20n, H.Step{[M.AskOpened{"settings-test", "worker", "approval", "settings_proposal", "private proposal", "", 20n}], Nil{}})))), "0")
    C.Check.str("managed proposal alerts at source", Bool.show(String.contains(managed.alert(), "A settings proposal needs your review.")), "True")
    C.Check.str("alert omits proposal details", Bool.show(String.contains(managed.alert(), "private proposal marker")), "False")
    C.Check.str("invalid secret-like input opens no ask", Nat.show(List.length(&2, M.Change, proposal.changes(unsafe.proposal()))), "0")
    C.Check.str("invalid secret-like input is absent from tool reply", Bool.show(String.contains(MC.McpOut.reply(unsafe.proposal()), "SECRET-FIXTURE-NOT-A-MODEL")), "False")
    C.Check.str("approval emits no start or stop", Nat.show(starts_or_stops(H.Step.outs_of(decide(asked(), 30n, "accept")))), "0")
    C.Check.str("unchanged setting is not written", Nat.show(writes(H.Queue.changes(H.Fallback.validate(asked(), caps(), 30n, H.Settings.answer.apply(True{}, P.Proposal{"off", "claude/sonnet", "on", "claude/sonnet", 610n}, 30n, 1, "x", askid(), "t1"))), "fallback.chain")), "0")
    C.Check.str("first approval invalidates concurrent proposal", ask.answer(applied(two.asked(), decide(two.asked(), 30n, "accept")), second.id()), "stale")
