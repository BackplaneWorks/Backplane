import Base
import ./model.bend as M
import ./mail.bend as ML
import ./mailbody.bend as MB
import ./mailtriage.bend as TRI
import ./bot.bend as BT
import ./uber.bend as UB
import ./ubersync.bend as US
import ./json.bend as J
import ./ref.bend as R
import ./far.bend as FR

# Waiting owner mail follows the conversation, not the local bot replica.
# Only journal coordinates and original header/conv metadata enter the event
# log. Bodies stay in the mail journal. A run claims its input atomically with
# RunSet; an already running input is never copied to a second machine.
def Owner.remote(+s: M.State) -> Bool:
  Bool.and(US.Ub.on(s), Bool.not(US.Ub.holder.me(s)))

def Owner.redirect(+s: M.State, +bot: String) -> Bool:
  Bool.and(TRI.Triage.owner(s, bot), Owner.remote(s))

def Owner.addr(+s: M.State) -> String:
  "bot:" ++ UB.Uber.bot(s)

def Owner.addressed(+s: M.State, +b: MB.Body) -> Bool:
  Bool.and(TRI.Triage.owner(s, UB.Uber.bot(s)), Bool.or(String.eq(ML.Mail.to(MB.Body.mail(b)), Owner.addr(s)), ML.Parts.has(ML.Conv.parts(MB.Body.conv(b)), Owner.addr(s), False{})))

def Owner.global(+uid: String, +origin: String, +id: String) -> String:
  "owner:" ++ J.Json.show(J.Json.arr([J.Str{uid}, J.Str{origin}, J.Str{id}]))

def Owner.key(+r: J.Json) -> String:
  Owner.global(J.Json.get_str(r, "uid", ""), J.Json.get_str(r, "origin", ""), J.Json.get_str(J.Json.get_obj(r, "mail"), "id", ""))

def Owner.pending.key(+key: String) -> String:
  "owner.mail.pending." ++ key

def Owner.map.key(+id: String) -> String:
  "owner.mail.id." ++ id

def Owner.local.key(+key: String) -> String:
  "owner.mail.local." ++ key

def Owner.record(+s: M.State, +b: MB.Body, +now: Nat) -> J.Json:
  J.Json.obj([J.KV{"uid", J.Str{US.Ub.uid(s)}}, J.KV{"origin", J.Str{US.Ub.me(s)}},
    J.KV{"mail", MB.Body.mail.encode(MB.Body.mail(b))}, J.KV{"conversation", MB.Body.conv.encode(MB.Body.conv(b))},
    J.KV{"id", J.Str{ML.Mail.id(MB.Body.mail(b))}}, J.KV{"conv", J.Str{ML.Mail.conv(MB.Body.mail(b))}},
    J.KV{"stamp", J.Str{Nat.show(now) ++ ":" ++ Nat.show(M.State.seq(s)) ++ ":" ++ Nat.show(US.Ub.term(s))}}])

def Owner.register(+r: J.Json, +now: Nat) -> List<&2, M.Change>:
  +key = Owner.key(r)
  [M.SettingSet{Owner.map.key(J.Json.get_str(r, "id", "")), key, now},
    M.SettingSet{Owner.local.key(key), J.Json.show(r), now},
    M.SettingSet{Owner.pending.key(key), J.Json.show(r), now}]

def Owner.track.on(on: Bool, +s: M.State, +b: MB.Body, +now: Nat) -> List<&2, M.Change>:
  match on:
    case False{}: Nil{}
    case True{}: Owner.register(Owner.record(s, b, now), now)

def Owner.track(+s: M.State, +b: MB.Body, +now: Nat) -> List<&2, M.Change>:
  Owner.track.on(Bool.and(Bool.and(Owner.addressed(s, b), TRI.Triage.inbound(MB.Body.mail(b), UB.Uber.bot(s))), String.is_empty(M.Setting.get(s, Owner.map.key(ML.Mail.id(MB.Body.mail(b))), ""))), s, b, now)

def Owner.claim.key(+key: String, +now: Nat) -> List<&2, M.Change>:
  match key:
    case SNil{}: Nil{}
    case SCon{c, t}: [M.SettingSet{Owner.pending.key(SCon{c, t}), "", now}]

def Owner.claim(+s: M.State, +id: String, +now: Nat) -> List<&2, M.Change>:
  Owner.claim.key(M.Setting.get(s, Owner.map.key(id), ""), now)

def Owner.remove(ids: List<&2, String>, +id: String, +acc: List<&2, String>) -> List<&2, String>:
  match ids:
    case Nil{}: List.reverse(&2, String, acc)
    case +h <> t: Owner.remove(t, id, Bool.pick(List<&2, String>, String.eq(h, id), acc, h <> acc))

# A stale worker cannot clear a ticket imported back after a lease move.
def Owner.current(+s: M.State, +r: J.Json) -> Bool:
  String.eq(M.Setting.get(s, Owner.pending.key(Owner.key(r)), ""), J.Json.show(r))

def Owner.sent.on(on: Bool, +s: M.State, +r: J.Json, +now: Nat) -> List<&2, M.Change>:
  match on:
    case False{}: Nil{}
    case True{}:
      +bot = UB.Uber.bot(s)
      +conv = J.Json.get_str(r, "conv", "")
      [M.SettingSet{Owner.pending.key(Owner.key(r)), "", now}, TRI.Triage.queue.set(bot, conv,
        Owner.remove(TRI.Triage.queue(s, bot, conv), J.Json.get_str(r, "id", ""), Nil{}), now)]

def Owner.sent(+s: M.State, +r: J.Json, +now: Nat) -> List<&2, M.Change>:
  Owner.sent.on(Owner.current(s, r), s, r, now)

def Owner.records.add(hit: Bool, +v: String, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match hit:
    case True{}: Maybe.default(&2, J.Json, J.Json.parse(v), J.Null{}) <> acc
    case False{}: acc

def Owner.records.go(xs: List<&2, String>, fuel: Nat, +s: M.State, +acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match xs fuel:
    case _ 0n: List.reverse(&2, J.Json, acc)
    case Nil{} _: List.reverse(&2, J.Json, acc)
    case +k <> t +n:
      +v = M.Setting.get(s, k, "")
      Owner.records.go(t, Nat.sub(n, Bool.pick(Nat, Bool.and(String.starts_with(k, "owner.mail.pending."), Bool.not(String.is_empty(v))), 1n, 0n)), s,
        Owner.records.add(Bool.and(String.starts_with(k, "owner.mail.pending."), Bool.not(String.is_empty(v))), v, acc))

def Owner.records(+s: M.State) -> List<&2, J.Json>:
  Owner.records.go(TRI.Triage.settings(s), 16n, s, Nil{})

def Owner.legacy.one(m: Maybe<&2, ML.Mail>, +s: M.State, +now: Nat) -> List<&2, M.Change>:
  match m:
    case None{}: Nil{}
    case Some{+m}: Owner.track(s, MB.Body{ML.Box.conv(M.State.mail(s), ML.Mail.conv(m)), m, ""}, now)

def Owner.receipt(+sender: String, +term: Nat, +key: String) -> String:
  "owner.mail.receipt." ++ J.Json.show(J.Json.arr([J.Str{sender}, J.Json.nat(term), J.Str{key}]))

def Owner.request(+s: M.State, +r: J.Json, +b: MB.Body) -> String:
  J.Json.show(J.Json.obj([J.KV{"m", J.Str{"owner.mail"}}, J.KV{"p", J.Json.obj([
    J.KV{"sender", J.Str{US.Ub.me(s)}}, J.KV{"uid", J.Str{US.Ub.uid(s)}}, J.KV{"term", J.Json.nat(J.Json.get_nat(r, "lease", US.Ub.term(s)))},
    J.KV{"ticket", r}, J.KV{"body", MB.Body.encode(b)},
    J.KV{"hold", J.Str{M.Setting.get(s, TRI.Triage.hold.key(UB.Uber.bot(s), J.Json.get_str(r, "conv", "")), "")}}])}]))

def Owner.accept(+s: M.State, +link: String, +p: J.Json) -> Bool:
  Bool.and(US.Ub.on(s), Bool.and(US.Ub.holder.me(s), Bool.and(String.eq(J.Json.get_str(p, "uid", ""), US.Ub.uid(s)),
    Bool.and(Nat.is_eq(J.Json.get_nat(p, "term", 0n), US.Ub.term(s)),
      Bool.and(Bool.not(String.is_empty(link)), String.eq(US.Ub.link(s, J.Json.get_str(p, "sender", "")), link))))))

def Owner.seen(+s: M.State, +p: J.Json) -> Bool:
  Bool.not(String.is_empty(M.Setting.get(s, Owner.receipt(J.Json.get_str(p, "sender", ""), J.Json.get_nat(p, "term", 0n), Owner.key(J.Json.get_obj(p, "ticket"))), "")))

def Owner.source.id(+s: M.State, +origin: String, +id: String) -> String:
  Bool.pick(String, Bool.or(String.is_empty(id), FR.Fr.is(id)), id,
    Bool.pick(String, String.eq(origin, US.Ub.me(s)), id, FR.Fr.id(Bool.pick(String, String.is_empty(US.Ub.link(s, origin)), origin, US.Ub.link(s, origin)), id)))

def Owner.source.addr(r: Maybe<&2, R.Ref>, +s: M.State, +origin: String, fallback: String) -> String:
  match r:
    case Some{R.Ref{+kind, id <> _, _}}: R.Ref.addr(R.Ref{kind, [Owner.source.id(s, origin, id)], ""})
    case _: fallback

def Owner.import.mail(m: ML.Mail, +s: M.State, +origin: String, +id: String, +conv: String) -> ML.Mail:
  match m:
    case ML.Mail{_, _, +from, _, +kind, +ref, want, head, size, at}:
      ML.Mail{id, conv, Owner.source.addr(R.Ref.of_addr(from), s, origin, from), Owner.addr(s), kind,
        Bool.pick(String, M.Launch.has(["thread", "launch", "work", "lock"], kind), Owner.source.id(s, origin, ref), ref), want, head, size, at}

def Owner.import.record(+s: M.State, +p: J.Json, +b: MB.Body, +now: Nat) -> J.Json:
  +r = J.Json.get_obj(p, "ticket")
  J.Json.obj([J.KV{"uid", J.Json.get_obj(r, "uid")}, J.KV{"origin", J.Json.get_obj(r, "origin")},
    J.KV{"mail", J.Json.get_obj(r, "mail")}, J.KV{"conversation", J.Json.get_obj(r, "conversation")},
    J.KV{"id", J.Str{ML.Mail.id(MB.Body.mail(b))}}, J.KV{"conv", J.Str{ML.Mail.conv(MB.Body.mail(b))}},
    J.KV{"stamp", J.Str{Nat.show(now) ++ ":" ++ Nat.show(M.State.seq(s)) ++ ":" ++ Nat.show(US.Ub.term(s))}}])

def Owner.import.changes(+s: M.State, +p: J.Json, +b: MB.Body, +now: Nat) -> List<&2, M.Change>:
  +r = Owner.import.record(s, p, b, now)
  +conv = ML.Mail.conv(MB.Body.mail(b))
  List.append(&2, M.Change, Owner.register(r, now),
    M.SettingSet{Owner.receipt(J.Json.get_str(p, "sender", ""), J.Json.get_nat(p, "term", 0n), Owner.key(r)), "committed", now} <>
    Bool.pick(List<&2, M.Change>, String.is_empty(J.Json.get_str(p, "hold", "")), Nil{}, [M.SettingSet{TRI.Triage.hold.key(UB.Uber.bot(s), conv), J.Json.get_str(p, "hold", ""), now}]))

def Owner.peer(+s: M.State, +link: String, +p: J.Json) -> Bool:
  Bool.and(US.Ub.on(s), Bool.and(String.eq(J.Json.get_str(p, "uid", ""), US.Ub.uid(s)),
    Bool.and(Bool.not(String.is_empty(link)), String.eq(US.Ub.link(s, J.Json.get_str(p, "sender", "")), link))))

# Persist the chosen recipient before IO. An ambiguous request is retried at
# that recipient even after a lease move; only a definitive moved/no-receipt
# reply permits choosing another holder. This prevents a lost ACK from
# starting the same input on two owners.
def Owner.attempt(+r: J.Json, +to: String, +term: Nat, +stamp: String) -> J.Json:
  J.Json.obj([J.KV{"uid", J.Json.get_obj(r, "uid")}, J.KV{"origin", J.Json.get_obj(r, "origin")},
    J.KV{"mail", J.Json.get_obj(r, "mail")}, J.KV{"conversation", J.Json.get_obj(r, "conversation")},
    J.KV{"id", J.Json.get_obj(r, "id")}, J.KV{"conv", J.Json.get_obj(r, "conv")},
    J.KV{"stamp", J.Str{stamp}}, J.KV{"to", J.Str{to}}, J.KV{"lease", J.Json.nat(term)}])

def Owner.plan.on(current: Bool, +s: M.State, +r: J.Json, +now: Nat) -> List<&2, M.Change>:
  match current:
    case False{}: Nil{}
    case True{}:
      +next = Owner.attempt(r, Bool.pick(String, Owner.remote(s), US.Ub.holder(s), ""), US.Ub.term(s), Nat.show(now) ++ ":" ++ Nat.show(M.State.seq(s)))
      [M.SettingSet{Owner.pending.key(Owner.key(r)), J.Json.show(next), now}]

def Owner.plan(+s: M.State, +r: J.Json, +now: Nat) -> List<&2, M.Change>:
  Owner.plan.on(Owner.current(s, r), s, r, now)

# A status code alone is not a receipt: an older hub may return HTTP 200
# with an unknown-method error. Only this exact protocol/key acknowledges IO.
def Owner.response(+r: J.Json, +state: String) -> String:
  J.Json.show(J.Json.obj([J.KV{"owner_mail", J.Str{state}}, J.KV{"key", J.Str{Owner.key(r)}}]))

def Owner.response.is(+text: String, +r: J.Json, +state: String) -> Bool:
  +j = Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})
  Bool.and(String.eq(J.Json.get_str(j, "owner_mail", ""), state), String.eq(J.Json.get_str(j, "key", ""), Owner.key(r)))

# A body that survived a torn header commit is new input, unlike old historical
# mail. Recovery seeds exactly those recovered headers, not the whole mailbox.
def Owner.recovered.headers(cs: List<&2, M.Change>, +s: M.State, +now: Nat) -> List<&2, M.Change>:
  match cs:
    case Nil{}: Nil{}
    case M.MailPosted{+id, +conv, _, _, _, _, _, _, _, _} <> rest:
      List.append(&2, M.Change, Owner.legacy.one(TRI.Triage.queue.mail(ML.Box.headers(M.State.mail(s), conv), id), s, now), Owner.recovered.headers(rest, s, now))
    case _ <> rest: Owner.recovered.headers(rest, s, now)

def Owner.recovered(+s: M.State, +cs: List<&2, M.Change>, +now: Nat) -> List<&2, M.Change>:
  Owner.recovered.headers(cs, M.State.replay(cs, s), now)

def Owner.import.parts(parts: List<&2, String>, +s: M.State, +origin: String, +oldowner: String) -> List<&2, String>:
  match parts:
    case Nil{}: Nil{}
    case +h <> rest:
      Bool.pick(String, Bool.and(String.starts_with(oldowner, "bot:"), String.eq(h, oldowner)), Owner.addr(s), Owner.source.addr(R.Ref.of_addr(h), s, origin, h)) <> Owner.import.parts(rest, s, origin, oldowner)

def Owner.import.conv(c: ML.Conv, +s: M.State, +origin: String, +conv: String, +oldowner: String) -> ML.Conv:
  match c:
    case ML.Conv{_, subject, kind, parts, +project, tags, made, last, n}:
      ML.Conv{conv, subject, kind, ML.Parts.add(Owner.import.parts(parts, s, origin, oldowner), Owner.addr(s)), Owner.source.id(s, origin, project), tags, made, last, n}

# Returning mail reuses this machine's original journal coordinates. A retry
# receipt is scoped to sender + lease term, not just the header ID: a genuine
# later handoff back can reactivate a still-unclaimed input once.
def Owner.import.body(+s: M.State, +p: J.Json) -> Maybe<&2, MB.Body>:
  do Maybe<&2, MB.Body>:
    +r : J.Json = J.Json.get_obj(p, "ticket")
    +key : String = Owner.key(r)
    +prior : J.Json = Maybe.default(&2, J.Json, J.Json.parse(M.Setting.get(s, Owner.local.key(key), "")), J.Null{})
    +origin : String = J.Json.get_str(r, "origin", "")
    +id : String = J.Json.get_str(prior, "id", key)
    +conv : String = J.Json.get_str(prior, "conv", "owner:" ++ J.Json.show(J.Json.arr([J.Str{J.Json.get_str(r, "uid", "")}, J.Str{origin}, J.Str{J.Json.get_str(J.Json.get_obj(r, "conversation"), "id", "")}])) )
    +b : MB.Body <- MB.Body.decode(J.Json.get_obj(p, "body"))
    m : ML.Mail <- MB.Body.mail.decode(J.Json.get_obj(r, "mail"))
    c : ML.Conv <- MB.Body.conv.decode(J.Json.get_obj(r, "conversation"))
    valid : Unit <- MB.Body.when(Unit, Bool.and(String.eq(J.Json.get_str(r, "uid", ""), US.Ub.uid(s)), Bool.and(String.eq(ML.Mail.id(MB.Body.mail(b)), J.Json.get_str(r, "id", "")), String.eq(ML.Mail.conv(MB.Body.mail(b)), J.Json.get_str(r, "conv", "")))), Unit{})
    return MB.Body{Owner.import.conv(c, s, origin, conv, J.Json.get_str(J.Json.get_obj(r, "mail"), "to", "")), Owner.import.mail(m, s, origin, id, conv), MB.Body.text(b)}

def Owner.waiting.known(empty: Bool, +s: M.State, +key: String, +now: Nat) -> List<&2, M.Change>:
  match empty:
    case False{}: Nil{}
    case True{}:
      +prior = Maybe.default(&2, J.Json, J.Json.parse(M.Setting.get(s, Owner.local.key(key), "")), J.Null{})
      Owner.register(Owner.attempt(prior, "", US.Ub.term(s), Nat.show(now) ++ ":" ++ Nat.show(M.State.seq(s))), now)

# A stopped input is put back in the existing held FIFO. It previously had
# been claimed, so restore its ticket only from that explicit waiting ID.
# No historical or still-running input is swept into the outbox.
def Owner.waiting.restore(missing: Bool, +s: M.State, +m: ML.Mail, +key: String, +now: Nat) -> List<&2, M.Change>:
  match missing:
    case True{}: Owner.legacy.one(Some{m}, s, now)
    case False{}:
      Owner.waiting.known(String.is_empty(M.Setting.get(s, Owner.pending.key(key), "")), s, key, now)

def Owner.waiting.one(m: Maybe<&2, ML.Mail>, +s: M.State, +now: Nat) -> List<&2, M.Change>:
  match m:
    case None{}: Nil{}
    case Some{+m}:
      +key = M.Setting.get(s, Owner.map.key(ML.Mail.id(m)), "")
      Owner.waiting.restore(String.is_empty(key), s, m, key, now)

# Old queues predate ticket metadata. Seed only their waiting IDs; never
# replay a historical header or steal an input already bound to a run.
def Owner.legacy.ids(ids: List<&2, String>, +s: M.State, +conv: String, +now: Nat) -> List<&2, M.Change>:
  match ids:
    case Nil{}: Nil{}
    case +id <> rest:
      List.append(&2, M.Change, Owner.waiting.one(TRI.Triage.queue.mail(ML.Box.headers(M.State.mail(s), conv), id), s, now), Owner.legacy.ids(rest, s, conv, now))

def Owner.legacy.parts(parts: List<&2, String>, +s: M.State, +now: Nat) -> List<&2, M.Change>:
  match parts:
    case +bot <> +conv <> Nil{}: Bool.pick(List<&2, M.Change>, TRI.Triage.owner(s, bot), Owner.legacy.ids(TRI.Triage.queue(s, bot, conv), s, conv, now), Nil{})
    case _: Nil{}

def Owner.legacy.queue(on: Bool, +s: M.State, +k: String, +now: Nat) -> List<&2, M.Change>:
  match on:
    case False{}: Nil{}
    case True{}: Owner.legacy.parts(TRI.Triage.queue.strings(J.Json.elems(Maybe.default(&2, J.Json, J.Json.parse(String.drop(k, 13n)), J.Null{}))), s, now)

def Owner.legacy.keys(xs: List<&2, String>, +s: M.State, +now: Nat) -> List<&2, M.Change>:
  match xs:
    case Nil{}: Nil{}
    case +k <> rest:
      List.append(&2, M.Change, Owner.legacy.queue(String.starts_with(k, "triage.queue."), s, k, now), Owner.legacy.keys(rest, s, now))

def Owner.legacy(+s: M.State, +now: Nat) -> List<&2, M.Change>:
  Owner.legacy.keys(TRI.Triage.settings(s), s, now)
