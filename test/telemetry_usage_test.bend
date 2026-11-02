import Base
import ../src/core/model.bend as M
import ../src/core/storedb.bend as DB
import ../src/core/snapshot.bend as SN
import ../src/core/json.bend as J
import ../src/core/telemetry-usage.bend as U
import ../src/core/telemetry.bend as T
import ../src/core/usage-counts.bend as UC
import ./telemetry_usage_cases.bend as F
import ./check.bend as C

def check(name: String, yes: Bool) -> IO(Unit):
  C.Check.str(name, Bool.pick(String, yes, "yes", "no"), "yes")

def counts(s: M.State) -> String:
  M.Metrics.get(s, "1970-01-02")

def events() -> List<&2, J.Json>:
  U.Usage.snapshots(86400n, F.UsageCases.state(), F.UsageCases.id(), F.UsageCases.view())

def later(n: Nat, +at: Nat, s: M.State) -> M.State:
  match n:
    case 0n: s
    case 1n+k: later(k, Nat.add(at, 86400n), M.State.apply(s, M.BotWoke{"bot", 0n, at}))

def rows.k(xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> List<&2, J.KV>:
  match xs:
    case Nil{}: Nil{}
    case (k, v) <> tail: J.KV{k, J.Str{v}} <> rows.k(tail)

def rows(m: Map<&2, String>) -> String:
  J.Json.show(J.Json.obj(rows.k(Map.to_list(&2, String, m))))

def stored(log: List<&2, M.Change>) -> DB.Mat:
  DB.Mat.replay(DB.Demo.lines(log, 0n), DB.Mat.empty())

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("durable daily counters and token dedup", counts(F.UsageCases.state()), "1/1/1/2/1/100/20/30/40/1/0/0")
    C.Check.str("legacy history is never backfilled", counts(M.State.replay(F.UsageCases.work(), M.State.empty())), "")
    C.Check.str("legacy consent alone cannot enable new metrics", counts(M.State.replay(F.UsageCases.work(), M.State.replay(List.take(&2, M.Change, F.UsageCases.consent(), 2n), M.State.empty()))), "")
    C.Check.str("gate closed does not collect", counts(M.State.replay(F.UsageCases.work(), M.State.apply(M.State.replay(F.UsageCases.consent(), M.State.empty()), M.SettingSet{"telemetry.usage.enabled", "", 86400n}))), "")
    C.Check.str("opt out erases projection", counts(M.State.apply(F.UsageCases.state(), M.SettingSet{"telemetry.share", "off", 86400n})), "")
    C.Check.str("reset erases projection", counts(M.State.apply(F.UsageCases.state(), M.SettingSet{"telemetry.reset", "1", 86400n})), "")
    C.Check.str("revoked expanded consent erases projection", counts(M.State.apply(F.UsageCases.state(), M.SettingSet{"telemetry.usage.asked", "", 86400n})), "")
    check("privacy change invalidates worker snapshot", Bool.not(U.Usage.current(M.Setting.get(F.UsageCases.state(), "telemetry.usage.generation", ""), M.State.replay([M.SettingSet{"telemetry.share", "off", 86400n}, M.SettingSet{"telemetry.share", "on", 86400n}], F.UsageCases.state()))))
    check("off then on rotates identity even between worker ticks", Bool.not(String.eq(T.Tm.reset(F.UsageCases.state()), T.Tm.reset(M.State.replay([M.SettingSet{"telemetry.share", "off", 86400n}, M.SettingSet{"telemetry.share", "on", 86400n}], F.UsageCases.state())))))
    C.Check.str("opt-out never produces a usage snapshot", Nat.show(List.length(&2, U.UsageRow, U.Usage.rows(86400n, M.State.apply(F.UsageCases.state(), M.SettingSet{"telemetry.share", "off", 86400n})))), "0")
    C.Check.str("unknown provider tokens stay unknown", counts(M.State.apply(F.UsageCases.state(), F.UsageCases.turn("next", ""))), "1/1/1/2/1/100/20/30/40/1/0/0")
    C.Check.str("output-only report counts known output", counts(M.State.apply(F.UsageCases.state(), F.UsageCases.turn("next", "o"))), "1/1/1/2/1/100/20/30/80/2/0/0")
    C.Check.str("seven daily rows retained", Nat.show(List.length(&2, String, Map.keys(&2, String, M.Metrics.data(later(10n, 86400n, F.UsageCases.state()))))), "7")
    C.Check.str("old day expires from snapshots", Nat.show(List.length(&2, U.UsageRow, U.Usage.rows(864000n, F.UsageCases.state()))), "0")
    C.Check.str("one nonempty snapshot", Nat.show(List.length(&2, J.Json, events())), "1")
    C.Check.str("ack suppresses unchanged snapshot", Nat.show(List.length(&2, J.Json, U.Usage.pending(events(), events()))), "0")
    C.Check.str("changed counters send next snapshot", Nat.show(List.length(&2, J.Json, U.Usage.pending(U.Usage.snapshots(86400n, M.State.apply(F.UsageCases.state(), M.BotWoke{"bot", 0n, 86400n}), F.UsageCases.id(), F.UsageCases.view()), events()))), "1")
    C.Check.str("reset id cannot reuse old ack", Nat.show(List.length(&2, J.Json, U.Usage.pending(U.Usage.snapshots(86400n, F.UsageCases.state(), "00000000-0000-4000-8000-000000000000", F.UsageCases.view()), events()))), "1")
    check("payload has no prompt", Bool.not(String.contains(U.Usage.body(86400n, "0123456789abcdef", events()), "private")))
    check("no reasoning or cost fields", Bool.not(Bool.or(String.contains(U.Usage.body(86400n, "0123456789abcdef", events()), "reasoning"), String.contains(U.Usage.body(86400n, "0123456789abcdef", events()), "cost"))))
    C.Check.str("exactly ten allowlisted counters", Nat.show(List.length(&2, String, UC.Uc.names())), "10")
    C.Check.str("counter saturation", UC.Uc.show(UC.Uc.add(UC.Uc.read("281474976710655"), 0n, 1n)), "281474976710655/0/0/0/0/0/0/0/0/0/0/0")
    check("request stays below wire cap", Nat.is_lt(String.length(U.Usage.body(86400n, "0123456789abcdef", events())), 16384n))
    C.Check.str("incremental store keeps projected counters", rows(DB.Mat.recs(stored(F.UsageCases.log()))), rows(DB.Tab.dump(F.UsageCases.state())))
    C.Check.str("hot boot restores private daily counters", counts(SN.Snap.boot(stored(F.UsageCases.log()), Nil{})), counts(F.UsageCases.state()))
    C.Check.str("snapshot plus tail never double counts", counts(SN.Snap.boot(stored(List.take(&2, M.Change, F.UsageCases.log(), 8n)), List.drop(&2, M.Change, F.UsageCases.log(), 8n))), counts(F.UsageCases.state()))
    C.Check.str("incremental store erases counters on reset", rows(DB.Mat.recs(stored(List.append(&2, M.Change, F.UsageCases.log(), [M.SettingSet{"telemetry.reset", "1", 86400n}])))), rows(DB.Tab.dump(M.State.apply(F.UsageCases.state(), M.SettingSet{"telemetry.reset", "1", 86400n}))))
