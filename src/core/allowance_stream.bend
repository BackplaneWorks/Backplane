import Base
import ./json.bend as J
import ./model.bend as M

# Passive Claude readings only. Missing/invalid windows are never zero usage.
# This leaves the legacy Usage-page limit settings and token totals intact.
def AllowanceStream.number(x: J.Json) -> F32:
  match x:
    case J.Num{s}: Maybe.default(&2, F32, F32.read(s), F32.neg(1.0))
    case _: F32.neg(1.0)

def AllowanceStream.window.k(valid: Bool, +w: J.Json, name: String, scope: String, minutes: Nat, +used: F32) -> List<&2, J.Json>:
  match valid:
    case True{}:
      [J.Json.obj([J.KV{"name", J.Str{name}}, J.KV{"scope", J.Str{scope}}, J.KV{"shared", J.Flag{True{}}},
        J.KV{"duration_minutes", J.Json.nat(minutes)}, J.KV{"used_permille", J.Json.nat(F32.to_nat(F32.ceil(F32.mul(used, 1000.0))))},
        J.KV{"reported_utilization", J.Json.get_obj(w, "utilization")},
        J.KV{"resets_at", Maybe.default(&2, J.Json, J.Json.get(w, "resetsAt"), J.Null{})}])]
    case False{}:
      [J.Json.obj([J.KV{"name", J.Str{name}}, J.KV{"scope", J.Str{scope}}, J.KV{"shared", J.Flag{True{}}},
        J.KV{"duration_minutes", J.Json.nat(minutes)}, J.KV{"used_permille", J.Null{}},
        J.KV{"resets_at", Maybe.default(&2, J.Json, J.Json.get(w, "resetsAt"), J.Null{})}])]

def AllowanceStream.window(+w: J.Json, name: String, scope: String, minutes: Nat) -> List<&2, J.Json>:
  +used = AllowanceStream.number(J.Json.get_obj(w, "utilization"))
  AllowanceStream.window.k(Bool.and(F32.is_ge(used, 0.0), F32.is_le(used, 1.0)), w, name, scope, minutes, used)

# Absence is unsupported/unreported; an explicitly present invalid window
# remains an unknown row and prevents a partial pool from showing a percentage.
def AllowanceStream.window.read(w: Maybe<&2, J.Json>, name: String, scope: String, minutes: Nat) -> List<&2, J.Json>:
  match w:
    case Some{v}: AllowanceStream.window(v, name, scope, minutes)
    case None{}: Nil{}

def AllowanceStream.windows(+w: J.Json) -> List<&2, J.Json>:
  List.concat(&2, J.Json, [
    AllowanceStream.window.read(J.Json.get(w, "five_hour"), "5 h", "shared", 300n),
    AllowanceStream.window.read(J.Json.get(w, "seven_day"), "7 d", "shared", 10080n),
    AllowanceStream.window.read(J.Json.get(w, "seven_day_opus"), "7 d · Opus", "opus", 10080n),
    AllowanceStream.window.read(J.Json.get(w, "seven_day_sonnet"), "7 d · Sonnet", "sonnet", 10080n)])

def AllowanceStream.snapshot(+now: Nat, ws: List<&2, J.Json>) -> String:
  J.Json.show(J.Json.obj([J.KV{"provider", J.Str{"claude"}}, J.KV{"source", J.Str{"Claude stream"}}, J.KV{"auth", J.Str{"unknown"}},
    J.KV{"state", J.Str{"ok"}}, J.KV{"at", J.Json.nat(now)}, J.KV{"windows", J.Json.arr(ws)}]))

def AllowanceStream.changed(has: Bool, +s: M.State, +now: Nat, ws: List<&2, J.Json>) -> List<&2, M.Change>:
  match has:
    case True{}:
      [M.SettingSet{"allowance.claude", AllowanceStream.snapshot(now, ws), now}]
    case False{}: Nil{}

# Changed reports publish immediately. Only identical normalized readings
# wait a minute before renewing their freshness timestamp.
def AllowanceStream.changes(+s: M.State, +now: Nat, +o: J.Json) -> List<&2, M.Change>:
  +ws = AllowanceStream.windows(J.Json.get_obj(J.Json.get_obj(o, "rate_limit_info"), "unifiedWindows"))
  +old = Maybe.default(&2, J.Json, J.Json.parse(M.Setting.get(s, "allowance.claude", "")), J.Null{})
  +at = J.Json.get_nat(old, "at", 0n)
  +different = Bool.not(String.eq(J.Json.show(J.Json.arr(ws)), J.Json.show(J.Json.get_obj(old, "windows"))))
  AllowanceStream.changed(Bool.and(Bool.not(List.is_empty(&2, J.Json, ws)), Bool.or(different,
    Bool.or(Nat.is_eq(at, 0n), Bool.or(Nat.is_lt(now, at), Nat.is_ge(Nat.sub(now, at), 60n))))), s, now, ws)

# The named-account collector owns its own profile+generation metadata.
def AllowanceStream.default(legacy: Bool, s: M.State, now: Nat, o: J.Json) -> List<&2, M.Change>:
  match legacy:
    case True{}: AllowanceStream.changes(s, now, o)
    case False{}: Nil{}
