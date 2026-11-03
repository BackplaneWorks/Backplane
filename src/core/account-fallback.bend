import Base
import ./accounts.bend as A
import ./fallback.bend as F
import ./json.bend as J
import ./model.bend as M

# Only public profile metadata enters this planner. Login stores and their
# contents remain owned by the existing provider runner.
def Thread.model(t: M.Thread) -> String:
  match t:
    case M.Thread{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, model, _, _, _, _}: model

def Hold.key(+p: String, +id: String, epoch: Nat) -> String:
  "account.limit." ++ A.Accounts.part(p) ++ A.Accounts.part(id) ++ Nat.show(epoch) ++ ":"

def Hold.model.key(p: String, id: String, epoch: Nat, model: String) -> String:
  Hold.key(p, id, epoch) ++ "model:" ++ A.Accounts.part(model)

def Hold.record(s: M.State, p: String, id: String, epoch: Nat) -> J.Json:
  A.Accounts.parse(M.Setting.get(s, Hold.key(p, id, epoch), ""))

def Hold.active(+s: M.State, p: String, id: String, epoch: Nat, now: Nat) -> Bool:
  F.Limit.active.o(Hold.record(s, p, id, epoch), now)

def Hold.reset(s: M.State, p: String, id: String, epoch: Nat) -> Nat:
  J.Json.get_nat(Hold.record(s, p, id, epoch), "reset", 0n)

def Hold.model.record(s: M.State, p: String, id: String, epoch: Nat, model: String) -> J.Json:
  A.Accounts.parse(M.Setting.get(s, Hold.model.key(p, id, epoch, model), ""))

def Hold.current(+s: M.State, +p: String, +id: String, +epoch: Nat, model: String, +now: Nat) -> Bool:
  Hold.active(s, p, id, epoch, now) || F.Limit.active.o(Hold.model.record(s, p, id, epoch, model), now)

def Hold.until.records(+a: J.Json, +b: J.Json) -> Nat:
  +unknown = (Bool.not(String.is_empty(J.Json.get_str(a, "scope", ""))) && Nat.is_eq(J.Json.get_nat(a, "reset", 0n), 0n)) ||
    (Bool.not(String.is_empty(J.Json.get_str(b, "scope", ""))) && Nat.is_eq(J.Json.get_nat(b, "reset", 0n), 0n))
  Bool.pick(Nat, unknown, 0n, Nat.max(J.Json.get_nat(a, "reset", 0n), J.Json.get_nat(b, "reset", 0n)))

def Hold.until(+s: M.State, +p: String, +id: String, +epoch: Nat, model: String) -> Nat:
  Hold.until.records(Hold.record(s, p, id, epoch), Hold.model.record(s, p, id, epoch, model))

def Hold.mark(+s: M.State, +now: Nat, +th: String, +lim: F.Limit) -> List<&2, M.Change>:
  +p = M.Thread.provider(M.State.thread.get(s, th))
  +id = A.Accounts.id(s, th, p)
  +epoch = A.Accounts.epoch(s, th, p)
  +key = Bool.pick(String, String.eq(F.Limit.scope(lim), p), Hold.key(p, id, epoch), Hold.model.key(p, id, epoch, F.Target.model(F.Target.parse(F.Limit.scope(lim)))))
  [M.SettingSet{key, F.Limit.json(lim, now), now}]

def Hold.clear.keys(xs: List<&2, String>, +now: Nat) -> List<&2, M.Change>:
  match xs:
    case Nil{}: Nil{}
    case k <> t: M.SettingSet{k, "", now} <> Hold.clear.keys(t, now)

def Hold.clear(+s: M.State, +now: Nat, p: String, id: String, epoch: Nat) -> List<&2, M.Change>:
  Hold.clear.keys(M.Setting.keys.pre(s, Hold.key(p, id, epoch)), now)

def Hold.profile.keys(xs: List<&2, String>, +s: M.State, +now: Nat) -> Bool:
  match xs:
    case Nil{}: False{}
    case k <> t: F.Limit.active.o(A.Accounts.parse(M.Setting.get(s, k, "")), now) || Hold.profile.keys(t, s, now)

def Hold.profile.active(+s: M.State, +now: Nat, p: String, id: String, epoch: Nat) -> Bool:
  Hold.profile.keys(M.Setting.keys.pre(s, Hold.key(p, id, epoch)), s, now)

def Profile.supports(xs: List<&2, J.Json>, +model: String) -> Bool:
  match xs:
    case Nil{}: False{}
    case h <> t: String.eq(J.Json.get_str(h, "id", ""), model) || Profile.supports(t, model)

def Profile.model(+profile: J.Json, +model: String) -> Bool:
  +xs = J.Json.elems(J.Json.get_obj(profile, "catalog"))
  String.is_empty(model) || List.is_empty(&2, J.Json, xs) || Profile.supports(xs, model)

# Rank by the most remaining allowance across applicable active windows.
# Public raw percentages and #694/#757 permille snapshots share this path.
# Missing, stale and expired readings stay unknown, never zero allowance.
def Usage.number(x: Maybe<&2, F32>) -> Maybe<&2, U32>:
  match x:
    case None{}: None{}
    case Some{+x}:
      Bool.pick(Maybe<&2, U32>, F32.is_ge(x, 0.0) && F32.is_le(x, 100.0), Some{F32.to_u32(F32.mul(x, 1000.0))}, None{})

def Usage.permille(x: Maybe<&2, F32>) -> Maybe<&2, U32>:
  match x:
    case None{}: None{}
    case Some{x}: Usage.number(Some{F32.div(x, 10.0)})

def Usage.first(a: Maybe<&2, U32>, b: Maybe<&2, U32>) -> Maybe<&2, U32>:
  match a:
    case Some{x}: Some{x}
    case None{}: b

def Usage.used(+w: J.Json) -> Maybe<&2, U32>:
  Usage.first(Usage.number(F32.read(J.Json.as_raw(J.Json.get(w, "used_percent"), ""))),
    Usage.permille(F32.read(J.Json.as_raw(J.Json.get(w, "used_permille"), ""))))

def Usage.max(a: Maybe<&2, U32>, b: Maybe<&2, U32>) -> Maybe<&2, U32>:
  match a b:
    case Some{a} Some{b}: Some{U32.max(a, b)}
    case None{} other: other
    case other None{}: other

def Usage.applies(+w: J.Json, +model: String) -> Bool:
  +scope = J.Json.get_str(w, "scope", "")
  (Bool.not(String.eq(scope, "seven_day_opus") || String.eq(scope, "opus")) || String.contains(model, "opus")) &&
    (Bool.not(String.eq(scope, "seven_day_sonnet") || String.eq(scope, "sonnet")) || String.contains(model, "sonnet"))

def Usage.windows(xs: List<&2, J.Json>, +now: Nat, +model: String) -> Maybe<&2, U32>:
  match xs:
    case Nil{}: None{}
    case +h <> t:
      +reset = J.Json.get_nat(h, "resets_at", 0n)
      Usage.max(Bool.pick(Maybe<&2, U32>, Usage.applies(h, model) && (Nat.is_eq(reset, 0n) || Nat.is_gt(reset, now)),
        Usage.used(h), None{}), Usage.windows(t, now, model))

def Usage.left(x: Maybe<&2, U32>) -> Maybe<&2, U32>:
  match x:
    case None{}: None{}
    case Some{x}: Some{U32.sub(100000, x)}

def Usage.score(+profile: J.Json, +now: Nat, model: String) -> Maybe<&2, U32>:
  +a = J.Json.get_obj(profile, "allowance")
  +at = J.Json.get_nat(a, "at", 0n)
  Bool.pick(Maybe<&2, U32>, String.eq(J.Json.get_str(a, "state", ""), "ok") && String.eq(J.Json.get_str(a, "phase", "complete"), "complete") && String.eq(J.Json.get_str(a, "provider", J.Json.get_str(profile, "provider", "")), J.Json.get_str(profile, "provider", "")) && Nat.is_gt(at, 0n) && Nat.is_le(at, now) && Nat.is_le(Nat.sub(now, at), 300n),
    Usage.left(Usage.windows(J.Json.elems(J.Json.get_obj(a, "windows")), now, model)), None{})


def Usage.exhausted(score: Maybe<&2, U32>) -> Bool:
  match score:
    case None{}: False{}
    case Some{left}: U32.is_eq(left, 0)

def Profile.eligible(+s: M.State, +now: Nat, +p: String, +model: String, +profile: J.Json) -> Bool:
  String.eq(J.Json.get_str(profile, "provider", ""), p) && String.eq(J.Json.get_str(profile, "state", ""), "connected") &&
    Bool.not(String.is_empty(J.Json.get_str(profile, "id", ""))) && Profile.model(profile, model) && Bool.not(Usage.exhausted(Usage.score(profile, now, model))) &&
    Bool.not(Hold.current(s, p, J.Json.get_str(profile, "id", ""), J.Json.get_nat(profile, "epoch", 0n), model, now))

def Profile.filter(xs: List<&2, J.Json>, +s: M.State, +now: Nat, +p: String, +model: String) -> List<&2, J.Json>:
  match xs:
    case Nil{}: Nil{}
    case +h <> t:
      +rest = Profile.filter(t, s, now, p, model)
      Bool.pick(List<&2, J.Json>, Profile.eligible(s, now, p, model, h), h <> rest, rest)

def Provider.has(xs: List<&2, J.Json>, +p: String) -> Bool:
  match xs:
    case Nil{}: False{}
    case h <> t: String.eq(J.Json.get_str(h, "provider", ""), p) || Provider.has(t, p)

def Provider.managed(+s: M.State, +th: String, +p: String) -> Bool:
  Bool.not(String.is_empty(A.Accounts.id(s, th, p))) || Provider.has(A.Accounts.profiles(s), p)

def Hold.inherit(+s: M.State, +now: Nat, +th: String, +lim: F.Limit) -> List<&2, M.Change>:
  +p = M.Thread.provider(M.State.thread.get(s, th))
  +model = Thread.model(M.State.thread.get(s, th))
  +target = {F.Target{p, model} : F.Target}
  +scope = F.Limit.scope(lim)
  +o = F.Limit.record(s, scope)
  +normalized = Bool.pick(List<&2, M.Change>, Nat.is_eq(J.Json.get_nat(o, "reset", 0n), 0n) && Nat.is_gt(F.Limit.reset(lim), 0n),
    [M.SettingSet{F.Limit.key(scope), F.Limit.json(lim, J.Json.get_nat(o, "at", now)), now}], Nil{})
  Bool.pick(List<&2, M.Change>, Provider.managed(s, th, p) && String.is_empty(A.Accounts.id(s, th, p)) && F.Target.held(s, target, now) &&
    (Bool.not(Hold.current(s, p, "", 0n, model, now)) || (Nat.is_eq(Hold.until(s, p, "", 0n, model), 0n) && Nat.is_gt(F.Limit.reset(lim), 0n))),
    List.append(&2, M.Change, normalized, Hold.mark(s, now, th, lim)), Nil{})

def Usage.known(xs: List<&2, J.Json>, +now: Nat, +model: String) -> Bool:
  match xs:
    case Nil{}: True{}
    case h <> t: Maybe.is_some(&2, U32, Usage.score(h, now, model)) && Usage.known(t, now, model)

def Usage.best(xs: List<&2, J.Json>, +now: Nat, +model: String, +best: J.Json, +score: U32) -> J.Json:
  match xs:
    case Nil{}: best
    case +h <> t:
      +v = Maybe.default(&2, U32, Usage.score(h, now, model), 0)
      Usage.best(t, now, model, Bool.pick(J.Json, U32.is_gt(v, score), h, best), U32.max(v, score))

# Prefer the largest known remaining allowance; unknown readings are eligible
# after known available accounts. With no readings, retain saved profile order.
def Pick.ranked(known: Bool, +xs: List<&2, J.Json>, +now: Nat, +model: String) -> Maybe<&2, J.Json>:
  match xs:
    case Nil{}: None{}
    case +h <> t:
      Bool.pick(Maybe<&2, J.Json>, known, Some{Usage.best(t, now, model, h, Maybe.default(&2, U32, Usage.score(h, now, model), 0))}, Some{h})

def Pick.account(+s: M.State, +now: Nat, p: String, +model: String) -> Maybe<&2, J.Json>:
  +xs = Profile.filter(A.Accounts.profiles(s), s, now, p, model)
  Pick.ranked(True{}, xs, now, model)

def Pick.none(choice: Maybe<&2, J.Json>) -> Bool:
  match choice:
    case Some{_}: False{}
    case None{}: True{}

# The hub gates every cross-provider account route on this actual selector.
def Pick.may_cross(s: M.State, now: Nat, p: String, model: String) -> Bool:
  Pick.none(Pick.account(s, now, p, model))

def Pick.binding(+profile: J.Json) -> String:
  A.Accounts.binding.show(J.Json.get_str(profile, "id", ""), J.Json.get_nat(profile, "epoch", 0n))

def Preference.key(+th: String) -> String:
  "account.failover.preference." ++ th

def Preference.record(s: M.State, th: String) -> J.Json:
  A.Accounts.parse(M.Setting.get(s, Preference.key(th), ""))

def Preference.forget(th: String, now: Nat) -> List<&2, M.Change>:
  [M.SettingSet{Preference.key(th), "", now}]

def Preference.matches(+s: M.State, +th: String, +o: J.Json) -> Bool:
  +p = M.Thread.provider(M.State.thread.get(s, th))
  String.eq(J.Json.get_str(o, "activeProvider", "!missing"), p) &&
    String.eq(J.Json.get_str(o, "activeBinding", "!missing"), A.Accounts.binding.show(A.Accounts.id(s, th, p), A.Accounts.epoch(s, th, p)))

def Preference.save(+s: M.State, +now: Nat, +th: String, +p: String, +binding: String) -> List<&2, M.Change>:
  +old = Preference.record(s, th)
  +from = M.Thread.provider(M.State.thread.get(s, th))
  +keep = Preference.matches(s, th, old)
  +o = Bool.pick(J.Json, keep, old, J.Json.obj([
    J.KV{"provider", J.Str{from}}, J.KV{"model", J.Str{Thread.model(M.State.thread.get(s, th))}}, J.KV{"effort", J.Str{M.Setting.get(s, "effort." ++ th, "")}},
    J.KV{"binding", J.Str{A.Accounts.binding.show(A.Accounts.id(s, th, from), A.Accounts.epoch(s, th, from))}}]))
  [M.SettingSet{Preference.key(th), J.Json.show(A.Accounts.put(A.Accounts.put(o, "activeProvider", J.Str{p}), "activeBinding", J.Str{binding})), now}]

def Preference.ready(+s: M.State, +now: Nat, +th: String) -> Bool:
  +o = Preference.record(s, th)
  +p = J.Json.get_str(o, "provider", "")
  +b = A.Accounts.binding.raw(J.Json.get_str(o, "binding", "!missing"))
  +id = J.Json.get_str(b, "id", "!missing")
  +epoch = J.Json.get_nat(b, "epoch", 0n)
  +model = J.Json.get_str(o, "model", "")
  +reset = Hold.until(s, p, id, epoch, model)
  Preference.matches(s, th, o) && Nat.is_gt(reset, 0n) && Nat.is_ge(now, reset) &&
    Bool.not(Hold.current(s, p, id, epoch, model, now)) &&
    (String.is_empty(id) || (A.Accounts.ready(A.Accounts.profile(s, id), p, epoch) && Profile.model(A.Accounts.profile(s, id), model)))

def Reset.earliest(+a: Nat, +b: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_eq(a, 0n), b, Bool.pick(Nat, Nat.is_eq(b, 0n), a, Bool.pick(Nat, Nat.is_le(a, b), a, b)))

def Reset.profiles(xs: List<&2, J.Json>, +s: M.State, +now: Nat, +p: String, +model: String, +best: Nat) -> Nat:
  match xs:
    case Nil{}: best
    case +h <> t:
      +r = Hold.until(s, p, J.Json.get_str(h, "id", ""), J.Json.get_nat(h, "epoch", 0n), model)
      Reset.profiles(t, s, now, p, model, Bool.pick(Nat, String.eq(J.Json.get_str(h, "provider", ""), p) && String.eq(J.Json.get_str(h, "state", ""), "connected") && Profile.model(h, model) && Nat.is_gt(r, now), Reset.earliest(best, r), best))

def Reset.next(+s: M.State, +now: Nat, +th: String, +p: String) -> Nat:
  +model = Thread.model(M.State.thread.get(s, th))
  Reset.profiles(A.Accounts.profiles(s), s, now, p, model, Hold.until(s, p, A.Accounts.id(s, th, p), A.Accounts.epoch(s, th, p), model))
