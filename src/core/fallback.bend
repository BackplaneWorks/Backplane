import Base
import ./json.bend as J
import ./model.bend as M

# One ordered, opt-in chain per hub. Explicit selections remain the starting
# point. Never wrap: an exhausted suffix blocks until a person intervenes.
type Target is Data:
  Target{provider: String, model: String}

def Target.provider(t: Target) -> String:
  match t:
    case Target{p, _}: p

def Target.model(t: Target) -> String:
  match t:
    case Target{_, m}: m

def Target.key(t: Target) -> String:
  match t:
    case Target{p, m}: p ++ "/" ++ m

def Target.parse(+s: String) -> Target:
  Target{Maybe.default(&2, String, List.head(&2, String, String.split(s, '/')), ""), String.join(List.drop(&2, String, String.split(s, '/'), 1n), "/")}

def Chain.parse.go(xs: List<&2, String>) -> List<&2, Target>:
  match xs:
    case Nil{}: Nil{}
    case +h <> t: Target.parse(h) <> Chain.parse.go(t)

def Chain.parse.empty(empty: Bool, s: String) -> List<&2, Target>:
  match empty:
    case True{}: Nil{}
    case False{}: Chain.parse.go(String.split(s, ','))

def Chain.parse(+s: String) -> List<&2, Target>:
  Chain.parse.empty(String.is_empty(s), s)

def Chain.of(s: M.State) -> List<&2, Target>:
  Chain.parse(M.Setting.get(s, "fallback.chain", ""))

def Chain.on(s: M.State) -> Bool:
  String.eq(M.Setting.get(s, "fallback.on", "off"), "on")

def Chain.keys(xs: List<&2, Target>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case h <> t: Target.key(h) <> Chain.keys(t)

def Chain.show(xs: List<&2, Target>) -> String:
  String.join(Chain.keys(xs), ",")

def Chain.has(xs: List<&2, Target>, +key: String) -> Bool:
  match xs:
    case Nil{}: False{}
    case h <> t: Bool.or(String.eq(Target.key(h), key), Chain.has(t, key))

def Chain.after.k(hit: Bool, t: List<&2, Target>, other: List<&2, Target>) -> List<&2, Target>:
  match hit:
    case True{}: t
    case False{}: other

def Chain.after(xs: List<&2, Target>, +key: String) -> List<&2, Target>:
  match xs:
    case Nil{}: Nil{}
    case h <> +t: Chain.after.k(String.eq(Target.key(h), key), t, Chain.after(t, key))

def Chain.suffix.k(has: Bool, +xs: List<&2, Target>, key: String) -> List<&2, Target>:
  match has:
    case True{}: Chain.after(xs, key)
    case False{}: xs

def Chain.suffix(+xs: List<&2, Target>, +key: String) -> List<&2, Target>:
  Chain.suffix.k(Chain.has(xs, key), xs, key)

def Caps.has(xs: List<&2, J.Json>, +m: String) -> Bool:
  match xs:
    case Nil{}: False{}
    case h <> t: Bool.or(String.eq(J.Json.as_str(Some{h}, ""), m), Caps.has(t, m))

def Caps.valid(+caps: J.Json, t: Target) -> Bool:
  match t:
    case Target{p, +m}:
      +xs = J.Json.elems(J.Json.get_obj(caps, p))
      Bool.and(Bool.not(List.is_empty(&2, J.Json, xs)), Bool.or(String.is_empty(m), Caps.has(xs, m)))

def Chain.valid(xs: List<&2, Target>, +caps: J.Json) -> Bool:
  match xs:
    case Nil{}: True{}
    case +h <> +t: Bool.and(Bool.and(Caps.valid(caps, h), Bool.not(Chain.has(t, Target.key(h)))), Chain.valid(t, caps))

# Outages are scoped to this hub's provider login, or to a model when the
# protocol names a model limit. No credential/account identifiers are stored.
# Unknown reset stays held until explicitly cleared; no invented retry time.
type Limit is Data:
  Limit{scope: String, reason: String, reset: Nat}

def Limit.scope(l: Limit) -> String:
  match l:
    case Limit{s, _, _}: s

def Limit.reason(l: Limit) -> String:
  match l:
    case Limit{_, r, _}: r

def Limit.reset(l: Limit) -> Nat:
  match l:
    case Limit{_, _, r}: r

def Limit.key(+scope: String) -> String:
  "fallback.outage." ++ scope

def Limit.json(l: Limit, +now: Nat) -> String:
  match l:
    case Limit{sc, why, reset}:
      J.Json.show(J.Json.obj([J.KV{"scope", J.Str{sc}}, J.KV{"reason", J.Str{String.take(why, 400n)}}, J.KV{"reset", J.Json.nat(reset)}, J.KV{"at", J.Json.nat(now)}]))

def Limit.record(s: M.State, +scope: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(M.Setting.get(s, Limit.key(scope), "")), J.Null{})

def Limit.active.o(+o: J.Json, +now: Nat) -> Bool:
  Bool.and(Bool.not(String.is_empty(J.Json.get_str(o, "scope", ""))), Bool.or(Nat.is_eq(J.Json.get_nat(o, "reset", 0n), 0n), Nat.is_lt(now, J.Json.get_nat(o, "reset", 0n))))

def Limit.active(+s: M.State, +scope: String, now: Nat) -> Bool:
  Limit.active.o(Limit.record(s, scope), now)

def Target.held(+s: M.State, +t: Target, +now: Nat) -> Bool:
  Bool.or(Limit.active(s, Target.provider(t), now), Limit.active(s, Target.key(t), now))

def Chain.pick.k(ok: Bool, h: Target, rest: Maybe<&2, Target>) -> Maybe<&2, Target>:
  match ok:
    case True{}: Some{h}
    case False{}: rest

def Chain.pick(xs: List<&2, Target>, +s: M.State, +caps: J.Json, +now: Nat) -> Maybe<&2, Target>:
  match xs:
    case Nil{}: None{}
    case +h <> t: Chain.pick.k(Bool.and(Caps.valid(caps, h), Bool.not(Target.held(s, h, now))), h, Chain.pick(t, s, caps, now))

# Deliberately narrow: only provider error envelopes are passed here, never
# arbitrary assistant/tool text. Auth, cancellations and network failures are
# not capacity signals. Protocol codes are preferred; known plan-limit wording
# covers older CLIs (including Claude's weekly limit).
def Error.code(+c: String) -> Bool:
  Bool.or(Bool.or(String.eq(c, "rate_limit_error"), String.eq(c, "rate_limit_exceeded")),
    Bool.or(Bool.or(String.eq(c, "insufficient_quota"), String.eq(c, "usage_limit_reached")),
      Bool.or(String.eq(c, "budget_exceeded"), String.eq(c, "quota_exceeded"))))

def Error.words(+s: String) -> Bool:
  Bool.or(String.contains(s, "hit your usage limit"), Bool.or(Bool.or(String.contains(s, "you've hit your limit"), String.contains(s, "you have hit your limit")),
    Bool.or(Bool.or(String.contains(s, "weekly limit"), String.contains(s, "usage limit reached")),
      Bool.or(Bool.or(String.contains(s, "usage limit has been reached"), String.contains(s, "insufficient quota")),
        Bool.or(String.contains(s, "rate limit exceeded"), String.contains(s, "budget exceeded"))))))

def Error.no(+s: String) -> Bool:
  Bool.or(Bool.or(String.contains(s, "authentication"), String.contains(s, "unauthorized")),
    Bool.or(Bool.or(String.contains(s, "log in"), String.contains(s, "login required")),
      Bool.or(Bool.or(String.contains(s, "cancelled"), String.contains(s, "canceled")), Bool.or(String.contains(s, "invalid api key"), String.contains(s, "keyring")))))

def Error.envelope(+o: J.Json) -> Bool:
  Bool.or(String.eq(J.Json.get_str(o, "type", ""), "turn.failed"),
    Bool.or(String.eq(J.Json.get_str(o, "type", ""), "error"), Bool.and(String.eq(J.Json.get_str(o, "type", ""), "result"), J.Json.get_bool(o, "is_error", False{}))))

def Error.reset(+o: J.Json) -> Nat:
  J.Json.get_nat(J.Json.get_obj(o, "error"), "resets_at", J.Json.get_nat(o, "resets_at", J.Json.get_nat(J.Json.get_obj(o, "rate_limit_info"), "resetsAt", 0n)))

def Chain.error.strs(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case h <> t: J.Json.as_str(Some{h}, "") <> Chain.error.strs(t)

def Chain.remove.k(hit: Bool, h: Target, rest: List<&2, Target>) -> List<&2, Target>:
  match hit:
    case True{}: rest
    case False{}: h <> rest

# List editing is shared by the four Settings renderers.
def Chain.remove(xs: List<&2, Target>, +key: String) -> List<&2, Target>:
  match xs:
    case Nil{}: Nil{}
    case +h <> t: Chain.remove.k(String.eq(Target.key(h), key), h, Chain.remove(t, key))

def Chain.up.k(hit: Bool, a: Target, b: Target, rest: List<&2, Target>, other: List<&2, Target>) -> List<&2, Target>:
  match hit:
    case True{}: b <> a <> rest
    case False{}: a <> other

def Chain.up.rest(xs: List<&2, Target>, +a: Target, +key: String, other: List<&2, Target>) -> List<&2, Target>:
  match xs:
    case +b <> +t: Chain.up.k(String.eq(Target.key(b), key), a, b, t, other)
    case Nil{}: [a]

def Chain.up(xs: List<&2, Target>, +key: String) -> List<&2, Target>:
  match xs:
    case a <> +rest: Chain.up.rest(rest, a, key, Chain.up(rest, key))
    case Nil{}: Nil{}

def Error.assistant.text(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case h <> t: J.Json.get_str(h, "text", "") <> Error.assistant.text(t)

def Error.rejected(+o: J.Json) -> Bool:
  Bool.and(String.eq(J.Json.get_str(o, "type", ""), "rate_limit_event"), String.eq(J.Json.get_str(J.Json.get_obj(o, "rate_limit_info"), "status", ""), "rejected"))

def Error.reason(+o: J.Json) -> String:
  Bool.pick(String, Error.rejected(o), "Provider usage limit (" ++ J.Json.get_str(J.Json.get_obj(o, "rate_limit_info"), "rateLimitType", "account") ++ ")",
    J.Json.get_str(J.Json.get_obj(o, "error"), "message", J.Json.get_str(o, "message", J.Json.get_str(o, "result", String.join(List.append(&2, String, Chain.error.strs(J.Json.elems(J.Json.get_obj(o, "errors"))), Error.assistant.text(J.Json.elems(J.Json.get_obj(J.Json.get_obj(o, "message"), "content")))), "; ")))))

def Error.assistant(+o: J.Json) -> Bool:
  Bool.and(String.eq(J.Json.get_str(o, "type", ""), "assistant"), String.eq(J.Json.get_str(o, "error", ""), "rate_limit"))

# Codex's JSON turn.failed may carry only the displayed message. Its
# "usage limit for <model>" form is model-specific; the bare form is account
# capacity. An explicit protocol scope takes precedence over wording.
def Error.account.scope(+scope: String, +o: J.Json) -> Bool:
  Bool.and(Bool.not(String.eq(scope, "model")), Bool.or(String.eq(scope, "account"),
    Bool.or(Bool.or(Error.rejected(o), Error.assistant(o)),
      Bool.and(Bool.not(String.contains(String.to_lower(Error.reason(o)), "hit your usage limit for ")),
        Bool.or(String.contains(String.to_lower(Error.reason(o)), "weekly limit"), String.contains(String.to_lower(Error.reason(o)), "usage limit"))))))

def Error.scope(+p: String, m: String, +o: J.Json) -> String:
  Bool.pick(String, Error.account.scope(J.Json.get_str(J.Json.get_obj(o, "error"), "scope", J.Json.get_str(o, "scope", "")), o), p, p ++ "/" ++ m)

def Error.classify.k(yes: Bool, p: String, m: String, +o: J.Json) -> Maybe<&2, Limit>:
  match yes:
    case True{}: Some{Limit{Error.scope(p, m, o), Error.reason(o), Error.reset(o)}}
    case False{}: None{}

def Error.classify(+p: String, m: String, +o: J.Json) -> Maybe<&2, Limit>:
  Error.classify.k(Bool.and(Bool.or(Bool.or(Error.rejected(o), Error.assistant(o)), Error.envelope(o)), Bool.and(Bool.not(Error.no(String.to_lower(Error.reason(o)))),
    Bool.or(Bool.or(Error.rejected(o), Error.assistant(o)), Bool.or(Error.code(J.Json.get_str(J.Json.get_obj(o, "error"), "code", J.Json.get_str(J.Json.get_obj(o, "error"), "type", ""))), Error.words(String.to_lower(Error.reason(o))))))), p, m, o)
