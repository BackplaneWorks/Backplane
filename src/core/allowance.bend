import Base
import ./json.bend as J
import ./usage.bend as U
import ./tz.bend as TZ
import ./accounts.bend as ACCT
import ./model.bend as M

# Pure account-limit projection. at is the reading time; attempted_at is
# the cache's last attempt, including failures. Neither is inferred from now.
def Allowance.parse(snapshot: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(snapshot), J.Null{})

# Only JSON integer numbers qualify, never numeric strings or null defaults.
def Allowance.number(v: Maybe<&2, J.Json>) -> Maybe<&2, Nat>:
  match v:
    case Some{J.Num{s}}: Nat.read(s)
    case _: None{}

def Allowance.nat(+o: J.Json, key: String) -> Nat:
  Maybe.default(&2, Nat, Allowance.number(J.Json.get(o, key)), 0n)

def Allowance.used.k(v: Maybe<&2, Nat>) -> Maybe<&2, Nat>:
  match v:
    case Some{+n}: Bool.pick(Maybe<&2, Nat>, Nat.is_le(n, 1000n), Some{n}, None{})
    case None{}: None{}

def Allowance.used(o: J.Json) -> Maybe<&2, Nat>:
  Allowance.used.k(Allowance.number(J.Json.get(o, "used_permille")))

def Allowance.remaining(+used: Nat) -> Nat:
  Nat.div(Nat.sub(1000n, used), 10n)

def Allowance.provider(+provider: String) -> Bool:
  String.eq(provider, "codex") || String.eq(provider, "claude")

def Allowance.matches(+o: J.Json, +provider: String) -> Bool:
  Allowance.provider(provider) && String.eq(J.Json.get_str(o, "provider", ""), provider)

def Allowance.time.known(+at: Nat, +now: Nat) -> Bool:
  Nat.is_gt(at, 0n) && Nat.is_le(at, now)

def Allowance.ttl(+o: J.Json) -> Nat:
  Bool.pick(Nat, J.Json.get_bool(o, "managed_account", False{}), 300n, 900n)

def Allowance.fresh(+o: J.Json, +now: Nat) -> Bool:
  +at = Allowance.nat(o, "at")
  String.eq(J.Json.get_str(o, "state", ""), "ok") &&
    String.eq(J.Json.get_str(o, "phase", "complete"), "complete") &&
    Allowance.time.known(at, now) && Nat.is_le(Nat.sub(now, at), Allowance.ttl(o))

def Allowance.attempt(o: J.Json) -> Nat:
  match o:
    case J.Obj{+fs}:
      Maybe.default(&2, Nat,
        Allowance.number(J.Json.find(fs, "attempted_at")), Allowance.nat(J.Obj{fs}, "at"))
    case _: 0n

def Allowance.due.at(+at: Nat, +now: Nat) -> Bool:
  # A future attempt is held until its clock catches up, never a retry loop.
  Nat.is_eq(at, 0n) || (Nat.is_ge(now, at) && Nat.is_ge(Nat.sub(now, at), 300n))

def Allowance.due(snapshot: String, +now: Nat) -> Bool:
  Allowance.due.at(Allowance.attempt(Allowance.parse(snapshot)), now)

# Recognize explicit family tokens, not a substring in an unrelated name.
def Allowance.model.chars(s: String) -> String:
  match s:
    case SNil{}: SNil{}
    case SCon{+c, t}:
      SCon{Bool.pick(Char, Char.is_alpha(c) || Char.is_digit(c), Char.to_lower(c), '-'), Allowance.model.chars(t)}

def Allowance.model.has(ts: List<&2, String>, +family: String) -> Bool:
  match ts:
    case Nil{}: False{}
    case h <> t: String.eq(h, family) || Allowance.model.has(t, family)

def Allowance.model(+model: String, +family: String) -> Bool:
  +ts = String.split(Allowance.model.chars(model), '-')
  Allowance.model.has(ts, family) &&
    Bool.not(Allowance.model.has(ts, Bool.pick(String, String.eq(family, "opus"), "sonnet", "opus")))

def Allowance.applies(+w: J.Json, +provider: String, +model: String) -> Bool:
  +scope = J.Json.get_str(w, "scope", "")
  String.eq(provider, "codex") ||
    (String.eq(provider, "claude") &&
      (String.eq(scope, "shared") ||
        (String.eq(scope, "opus") && Allowance.model(model, "opus")) ||
        (String.eq(scope, "sonnet") && Allowance.model(model, "sonnet"))))

def Allowance.windows.keep(keep: Bool, w: J.Json, rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match keep:
    case True{}: w <> rest
    case False{}: rest

def Allowance.windows.go(ws: List<&2, J.Json>, +provider: String, +model: String) -> List<&2, J.Json>:
  match ws:
    case Nil{}: Nil{}
    case +w <> t:
      Allowance.windows.keep(Allowance.applies(w, provider, model), w, Allowance.windows.go(t, provider, model))

def Allowance.windows(+o: J.Json, +provider: String, +model: String) -> List<&2, J.Json>:
  Allowance.windows.go(J.Json.elems(J.Json.get_obj(o, "windows")), provider, model)

def Allowance.reset.passed.at(at: Maybe<&2, Nat>, +now: Nat) -> Bool:
  match at:
    case Some{reset}: Nat.is_le(reset, now)
    case None{}: False{}

def Allowance.reset.passed(+w: J.Json, +now: Nat) -> Bool:
  Allowance.reset.passed.at(Allowance.number(J.Json.get(w, "resets_at")), now)

# A single pool can have several time windows. Use its most restrictive
# remaining value, only when every applicable window has a current reading.
# Distinct scopes are distinct pools; no model-to-Codex-pool mapping exists.
type Pool is Data:
  Pool{scope: String, left: Nat, valid: Bool, stale: Bool, multiple: Bool}

def Allowance.pool.used(v: Maybe<&2, Nat>, +w: J.Json, +scope: String, +now: Nat, p: Pool) -> Pool:
  match v p:
    case Some{+n} Pool{+s, +left, valid, stale, multiple}:
      Pool{s, Nat.min(left, Allowance.remaining(n)), valid && Bool.not(String.is_empty(scope)),
        stale || Allowance.reset.passed(w, now), multiple || Bool.not(String.eq(s, scope))}
    case None{} Pool{s, left, _, stale, multiple}:
      Pool{s, left, False{}, stale || Allowance.reset.passed(w, now), multiple}

def Allowance.pool.go(ws: List<&2, J.Json>, +now: Nat, p: Pool) -> Pool:
  match ws:
    case Nil{}: p
    case +w <> t:
      Allowance.pool.go(t, now, Allowance.pool.used(Allowance.used(w), w, J.Json.get_str(w, "scope", ""), now, p))

def Allowance.pool(ws: List<&2, J.Json>, +now: Nat) -> Pool:
  match ws:
    case Nil{}: Pool{"", 0n, False{}, False{}, False{}}
    case +w <> t:
      Allowance.pool.go(w <> t, now, Pool{J.Json.get_str(w, "scope", ""), 100n, True{}, False{}, False{}})

def Allowance.chip.pool(p: Pool, +fresh: Bool) -> String:
  match p:
    case Pool{+scope, left, valid, stale, multiple}:
      Bool.pick(String, Bool.not(fresh) || stale, "Limits stale",
        Bool.pick(String, Bool.not(valid), "Limits unknown",
          Bool.pick(String, multiple, "Shared limits", Nat.show(left) ++ "% left · shared")))

def Allowance.chip.given(ok: Bool, +o: J.Json, +provider: String, +model: String, +now: Nat) -> String:
  match ok:
    case False{}: "Limits unknown"
    case True{}:
      Allowance.chip.pool(Allowance.pool(Allowance.windows(o, provider, model), now), Allowance.fresh(o, now))

def Allowance.chip.parsed(+o: J.Json, +provider: String, +model: String, +now: Nat) -> String:
  Allowance.chip.given(Allowance.matches(o, provider) &&
    (String.eq(J.Json.get_str(o, "state", ""), "ok") ||
      ((String.eq(J.Json.get_str(o, "state", ""), "error") || String.eq(J.Json.get_str(o, "state", ""), "stale")) &&
        Nat.is_gt(List.length(&2, J.Json, Allowance.windows(o, provider, model)), 0n))), o, provider, model, now)

def Allowance.chip(+snapshot: String, +provider: String, +model: String, +now: Nat) -> String:
  Allowance.chip.parsed(Allowance.parse(snapshot), provider, model, now)

def Allowance.auth(+o: J.Json) -> String:
  +auth = J.Json.get_str(o, "auth", "")
  Bool.pick(String, String.eq(auth, "chatgpt"), "ChatGPT account",
    Bool.pick(String, String.eq(auth, "apikey"), "API key account",
      Bool.pick(String, String.eq(J.Json.get_str(o, "provider", ""), "claude"),
        "Claude CLI account on this machine · identity not reported", "Account auth unknown")))

def Allowance.relative(+seconds: Nat) -> String:
  Bool.pick(String, Nat.is_lt(seconds, 60n), Nat.show(seconds) ++ " s",
    Bool.pick(String, Nat.is_lt(seconds, 3600n), Nat.show(Nat.div(seconds, 60n)) ++ " min",
      Bool.pick(String, Nat.is_lt(seconds, 86400n), Nat.show(Nat.div(seconds, 3600n)) ++ " h",
        Nat.show(Nat.div(seconds, 86400n)) ++ " d")))

def Allowance.utc(+at: Nat) -> String:
  TZ.Tz.iso.at(at, "Z") ++ " UTC"

def Allowance.reset.at(at: Maybe<&2, Nat>, +now: Nat) -> String:
  match at:
    case None{}: "reset unknown"
    case Some{+at}:
      "reset " ++ Allowance.utc(at) ++
        Bool.pick(String, Nat.is_gt(at, now), " (in " ++ Allowance.relative(Nat.sub(at, now)) ++ ")",
          " (passed " ++ Allowance.relative(Nat.sub(now, at)) ++ " ago)")

def Allowance.reset(+w: J.Json, +now: Nat) -> String:
  Allowance.reset.at(Allowance.number(J.Json.get(w, "resets_at")), now)

def Allowance.duration.minutes(+mins: Nat) -> String:
  Bool.pick(String, Nat.is_eq(mins, 0n), "duration unknown",
    Bool.pick(String, Nat.is_eq(Nat.mod(mins, 1440n), 0n), Nat.show(Nat.div(mins, 1440n)) ++ " d window",
      Bool.pick(String, Nat.is_eq(Nat.mod(mins, 60n), 0n), Nat.show(Nat.div(mins, 60n)) ++ " h window", Nat.show(mins) ++ " min window")))

def Allowance.duration(+w: J.Json) -> String:
  Allowance.duration.minutes(Allowance.nat(w, "duration_minutes"))

def Allowance.window.left(v: Maybe<&2, Nat>, +stale: Bool) -> String:
  match v:
    case Some{n}:
      Bool.pick(String, stale, "Last reported ", "") ++ Nat.show(Allowance.remaining(n)) ++ "% left" ++
        Bool.pick(String, stale, " · stale", "")
    case None{}: "Remaining unknown · invalid or missing used_permille"

def Allowance.window.row(+w: J.Json, +provider: String, +fresh: Bool, +now: Nat) -> U.URow:
  U.URow{J.Json.get_str(w, "name", "Unnamed window"),
    Allowance.window.left(Allowance.used(w), Bool.not(fresh) || Allowance.reset.passed(w, now)) ++
      " · " ++ Allowance.duration(w) ++ " · " ++ Allowance.reset(w, now) ++
      " · scope: " ++ J.Json.get_str(w, "scope", "unknown") ++
      Bool.pick(String, String.eq(provider, "codex") || J.Json.get_bool(w, "shared", False{}) ||
        String.eq(J.Json.get_str(w, "scope", ""), "shared"), " · shared account pool", " · model-specific pool"), ""}

def Allowance.window.rows(ws: List<&2, J.Json>, +provider: String, +fresh: Bool, +now: Nat) -> List<&2, U.URow>:
  match ws:
    case Nil{}: [U.URow{"Windows", "No applicable reported windows; remaining unknown", ""}]
    case w <> Nil{}: [Allowance.window.row(w, provider, fresh, now)]
    case w <> t:
      row rest = Allowance.window.row(w, provider, fresh, now) Allowance.window.rows(t, provider, fresh, now)
      row <> rest

def Allowance.updated(+o: J.Json, +now: Nat) -> String:
  +at = Allowance.nat(o, "at")
  Bool.pick(String, Allowance.time.known(at, now),
    Allowance.utc(at) ++ " (" ++ Allowance.relative(Nat.sub(now, at)) ++ " ago)",
    "Update time unknown (missing, invalid, zero or future at)")

def Allowance.status.current(+o: J.Json, +now: Nat) -> String:
  Bool.pick(String, Allowance.fresh(o, now), "Reported limits", "Limits stale · reading age cannot establish a current percentage")

def Allowance.status(+o: J.Json, +now: Nat) -> String:
  +state = J.Json.get_str(o, "state", "")
  Bool.pick(String, String.eq(state, "error"), "Refresh error · last reported windows are stale",
    Bool.pick(String, String.eq(state, "unavailable"), "Limits unavailable · remaining unknown",
      Bool.pick(String, String.eq(state, "ok") || String.eq(state, "stale"),
        Allowance.status.current(o, now),
        "Limits unknown · invalid or legacy snapshot")))

def Allowance.rows.given(ok: Bool, +o: J.Json, +provider: String, +model: String, +now: Nat) -> List<&2, U.URow>:
  match ok:
    case False{}: [U.URow{"Limits unknown", "No matching valid provider snapshot; unavailable, invalid or legacy reading", ""}]
    case True{}:
      U.URow{"Account limits", Allowance.status(o, now), ""} <>
        U.URow{"Account scope", J.Json.get_str(o, "account_label", "Default CLI account") ++ " · " ++ Allowance.auth(o) ++
          Bool.pick(String, String.eq(provider, "codex"),
            " · shared account pools; allocation per model not reported",
            " · shared windows and explicitly reported model-specific pools"), ""} <>
        U.URow{"Last update", Allowance.updated(o, now), ""} <>
        U.URow{"Source", J.Json.get_str(o, "source", "unknown"), ""} <>
        Allowance.window.rows(Allowance.windows(o, provider, model), provider, Allowance.fresh(o, now), now)

def Allowance.rows.parsed(+o: J.Json, +provider: String, +model: String, +now: Nat) -> List<&2, U.URow>:
  U.URow{"Selected model", provider ++ " · " ++ Bool.pick(String, String.is_empty(model), "default model", model), ""} <>
    Allowance.rows.given(Allowance.matches(o, provider), o, provider, model, now)

def Allowance.rows(+snapshot: String, +provider: String, +model: String, +now: Nat) -> List<&2, U.URow>:
  Allowance.rows.parsed(Allowance.parse(snapshot), provider, model, now)

# Adapt only the authoritative selected named profile. Never borrow a default
# account, another provider or another identity generation. This is pure: no
# status/refresh action, auth read or settings mutation is performed here.
def Allowance.percent.got(valid: Bool, used: F32) -> J.Json:
  match valid:
    case True{}: J.Json.nat(F32.to_nat(F32.ceil(F32.mul(used, 10.0))))
    case False{}: J.Null{}

def Allowance.percent(v: J.Json) -> J.Json:
  match v:
    case J.Num{s}:
      +used = Maybe.default(&2, F32, F32.read(s), F32.neg(1.0))
      Allowance.percent.got(F32.is_ge(used, 0.0) && F32.is_le(used, 100.0), used)
    case _: J.Null{}

def Allowance.named.scope(+scope: String, +provider: String) -> String:
  Bool.pick(String, String.eq(provider, "claude"),
    Bool.pick(String, String.eq(scope, "five_hour") || String.eq(scope, "seven_day"), "shared",
      Bool.pick(String, String.eq(scope, "seven_day_opus"), "opus", Bool.pick(String, String.eq(scope, "seven_day_sonnet"), "sonnet", ""))), scope)

def Allowance.named.windows(ws: List<&2, J.Json>, +provider: String) -> List<&2, J.Json>:
  match ws:
    case Nil{}: Nil{}
    case +w <> t:
      J.Json.obj([J.KV{"name", J.Json.get_obj(w, "name")}, J.KV{"scope", J.Str{Allowance.named.scope(J.Json.get_str(w, "scope", ""), provider)}},
        J.KV{"shared", J.Flag{True{}}}, J.KV{"used_permille", Allowance.percent(J.Json.get_obj(w, "used_percent"))},
        J.KV{"resets_at", J.Json.get_obj(w, "resets_at")}, J.KV{"duration_minutes", J.Json.get_obj(w, "window_minutes")}]) <>
        Allowance.named.windows(t, provider)

def Allowance.named.snapshot(valid: Bool, +profile: J.Json, +provider: String) -> String:
  match valid:
    case False{}:
      J.Json.show(J.Json.obj([J.KV{"provider", J.Str{provider}}, J.KV{"managed_account", J.Flag{True{}}},
        J.KV{"state", J.Str{"unavailable"}}, J.KV{"account_label", J.Str{"Selected account unavailable"}}]))
    case True{}:
      +a = J.Json.get_obj(profile, "allowance")
      J.Json.show(J.Json.obj([J.KV{"provider", J.Str{provider}}, J.KV{"state", J.Str{J.Json.get_str(a, "state", "unavailable")}},
        J.KV{"managed_account", J.Flag{True{}}}, J.KV{"account_label", J.Str{J.Json.get_str(profile, "label", "Selected account")}},
        J.KV{"at", J.Json.get_obj(a, "at")}, J.KV{"source", J.Json.get_obj(a, "source")}, J.KV{"auth", J.Json.get_obj(a, "auth")},
        J.KV{"windows", J.Json.arr(Allowance.named.windows(J.Json.elems(J.Json.get_obj(a, "windows")), provider))}]))

def Allowance.selected.k(legacy: Bool, +st: M.State, +th: String, +provider: String) -> String:
  match legacy:
    case True{}: M.Setting.get(st, "allowance." ++ provider, "")
    case False{}:
      +p = ACCT.Accounts.profile(st, ACCT.Accounts.id(st, th, provider))
      Allowance.named.snapshot(ACCT.Accounts.ready(p, provider, ACCT.Accounts.epoch(st, th, provider)) &&
        String.eq(J.Json.get_str(J.Json.get_obj(p, "allowance"), "provider", provider), provider), p, provider)

def Allowance.selected(+st: M.State, +th: String, +provider: String) -> String:
  Allowance.selected.k(String.is_empty(ACCT.Accounts.id(st, th, provider)), st, th, provider)

# Exclusive presentation deadline. Retiring is not a new provider reading.
def Allowance.until.window(reset: Maybe<&2, Nat>, +until: Nat) -> Nat:
  match reset:
    case Some{+n}: Bool.pick(Nat, Nat.is_lt(n, until), n, until)
    case None{}: until

def Allowance.until.windows(ws: List<&2, J.Json>, until: Nat) -> Nat:
  match ws:
    case Nil{}: until
    case w <> t: Allowance.until.windows(t, Allowance.until.window(Allowance.number(J.Json.get(w, "resets_at")), until))

def Allowance.until.parsed(+o: J.Json, provider: String, model: String) -> Nat:
  Allowance.until.windows(Allowance.windows(o, provider, model), Nat.add(Allowance.nat(o, "at"), Nat.add(1n, Allowance.ttl(o))))

def Allowance.until(snapshot: String, provider: String, model: String) -> Nat:
  Allowance.until.parsed(Allowance.parse(snapshot), provider, model)

# Both explicit UI dispatch and the hub use the canonical thread's provider
# and binding; callers cannot supply an account or a model for this read.
def Allowance.read.allowed(+st: M.State, +th: String) -> Bool:
  String.eq(M.Thread.provider(M.State.thread.get(st, th)), "codex") && String.is_empty(ACCT.Accounts.id(st, th, "codex"))
