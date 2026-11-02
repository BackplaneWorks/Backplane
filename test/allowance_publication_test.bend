import Base
import ./check.bend as C
import ../src/core/hub.bend as H
import ../src/core/allowance.bend as A
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/server/server.bend as S

def js(s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def report(five: String, extra: String) -> J.Json:
  js("{\"rate_limit_info\":{\"unifiedWindows\":{\"five_hour\":{\"utilization\":" ++ five ++ ",\"resetsAt\":10000}" ++ extra ++ "}}}")

def apply.step(st: M.State, step: H.Step) -> M.State:
  match step:
    case H.Step{cs, _}: M.State.replay(cs, st)

def apply(+st: M.State, +now: Nat, o: J.Json) -> M.State:
  apply.step(st, H.Claude.line.k(5n, st, "t", now, o))

def chip(+st: M.State, now: Nat) -> String:
  A.Allowance.chip(M.Setting.get(st, "allowance.claude", ""), "claude", "claude-opus-5-5", now)

def snapshot(attempt: Nat, at: Nat, phase: String, state: String) -> String:
  J.Json.show(J.Json.obj([J.KV{"provider", J.Str{"codex"}}, J.KV{"source", J.Str{"codex-app-server"}}, J.KV{"attempted_at", J.Json.nat(attempt)},
    J.KV{"at", J.Json.nat(at)}, J.KV{"phase", J.Str{phase}}, J.KV{"state", J.Str{state}}, J.KV{"windows", J.Json.arr(Nil{})}]))

def publish(+st: M.State, snapshot: String) -> M.State:
  M.State.replay(S.Hub.allowance.changes(st, snapshot, 1000n), st)

def count(+st: M.State, snapshot: String) -> String:
  Nat.show(List.length(&2, M.Change, S.Hub.allowance.changes(st, snapshot, 1000n)))

def main() -> IO(Unit):
  do IO<Unit>:
    +first : M.State = apply(M.State.empty(), 1000n, report("0.28", ""))
    C.Check.str("actual stream initial remaining", chip(first, 1000n), "72% left · shared")
    C.Check.str("changed stream before minute publishes exhausted state", chip(apply(first, 1020n, report("1.0", "")), 1020n), "0% left · shared")
    C.Check.str("identical stream before minute keeps sample age", Nat.show(J.Json.get_nat(js(M.Setting.get(apply(first, 1020n, report("0.28", "")), "allowance.claude", "")), "at", 0n)), "1000")
    C.Check.str("mixed numeric and null window never asserts current percentage", chip(apply(first, 1020n, report("0.1", ",\"seven_day\":{\"utilization\":null}")), 1020n), "Limits unknown")
    C.Check.str("mixed numeric and string window stays unknown", chip(apply(first, 1020n, report("0.1", ",\"seven_day\":{\"utilization\":\"0.2\"}")), 1020n), "Limits unknown")
    C.Check.str("present null window stays unknown", chip(apply(first, 1020n, report("0.1", ",\"seven_day\":null")), 1020n), "Limits unknown")
    C.Check.str("absent unsupported window remains absent", chip(apply(first, 1020n, report("0.1", "")), 1020n), "90% left · shared")
    +newest : M.State = publish(M.State.empty(), snapshot(200n, 200n, "complete", "ok"))
    C.Check.str("actual hub rejects older completed callback", count(newest, snapshot(100n, 100n, "complete", "error")), "0")
    C.Check.str("actual hub rejects delayed pending at same attempt", count(newest, snapshot(200n, 0n, "pending", "error")), "0")
    C.Check.str("actual hub complete result is idempotent", count(newest, snapshot(200n, 200n, "complete", "ok")), "0")
    C.Check.str("same attempt conflicting complete cannot replace outcome", count(newest, snapshot(200n, 0n, "complete", "error")), "0")
    C.Check.str("newer account clear accepted despite lower sample at", count(newest, snapshot(300n, 0n, "complete", "unavailable")), "1")
    C.Check.str("pending first is not published", count(M.State.empty(), snapshot(300n, 0n, "pending", "error")), "0")
    C.Check.str("old pending upgraded by final equal attempt", count(M.State.apply(M.State.empty(), M.SettingSet{"allowance.codex", snapshot(300n, 0n, "pending", "error"), 300n}), snapshot(300n, 300n, "complete", "ok")), "1")
    C.Check.str("future attempt cannot overwrite current reading", count(newest, snapshot(1001n, 1001n, "complete", "ok")), "0")
    C.Check.str("untyped string attempt is not a publication clock", count(newest, "{\"provider\":\"codex\",\"source\":\"codex-app-server\",\"attempted_at\":\"300\",\"phase\":\"complete\",\"state\":\"ok\"}"), "0")
    C.Check.str("missing phase cannot masquerade as completed callback", count(newest, "{\"provider\":\"codex\",\"source\":\"codex-app-server\",\"attempted_at\":300,\"state\":\"ok\"}"), "0")
