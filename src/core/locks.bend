import Base
import ./json.bend as J

# A single holder slot makes exclusivity structural. Waiters retain FIFO order.
type LockParty is Data:
  LockParty{id: String, kind: String, thread: String, note: String, since: Nat, lease: Nat}

type NamedLock is Data:
  NamedLock{holder: Maybe<&2, LockParty>, waiters: List<&2, LockParty>, ready: Bool, outside: Maybe<&2, LockParty>}

def Lock.empty() -> NamedLock:
  NamedLock{None{}, Nil{}, False{}, None{}}

def Lock.id(p: LockParty) -> String:
  match p:
    case LockParty{id, _, _, _, _, _}:
      id

def Lock.thread(p: LockParty) -> String:
  match p:
    case LockParty{_, _, th, _, _, _}:
      th

def Lock.kind(p: LockParty) -> String:
  match p:
    case LockParty{_, k, _, _, _, _}:
      k

def Lock.since(p: LockParty) -> Nat:
  match p:
    case LockParty{_, _, _, _, n, _}:
      n

def Lock.note(p: LockParty) -> String:
  match p:
    case LockParty{_, _, _, n, _, _}:
      n

def Lock.lease(p: LockParty) -> Nat:
  match p:
    case LockParty{_, _, _, _, _, n}:
      n

def Lock.party.json(p: LockParty) -> J.Json:
  match p:
    case LockParty{id, kind, thread, note, since, lease}:
      J.Json.obj([J.KV{"id", J.Str{id}}, J.KV{"kind", J.Str{kind}}, J.KV{"thread", J.Str{thread}}, J.KV{"note", J.Str{note}}, J.KV{"since", J.Num{Nat.show(since)}}, J.KV{"lease", J.Num{Nat.show(lease)}}])

def Lock.party.read(+o: J.Json) -> LockParty:
  LockParty{J.Json.get_str(o, "id", ""), J.Json.get_str(o, "kind", "thread"), J.Json.get_str(o, "thread", ""), J.Json.get_str(o, "note", ""), J.Json.get_nat(o, "since", 0n), J.Json.get_nat(o, "lease", 0n)}

def Lock.parties.json(ps: List<&2, LockParty>) -> List<&2, J.Json>:
  match ps:
    case Nil{}:
      Nil{}
    case h <> t:
      Lock.party.json(h) <> Lock.parties.json(t)

def Lock.parties.read(ps: List<&2, J.Json>) -> List<&2, LockParty>:
  match ps:
    case Nil{}:
      Nil{}
    case h <> t:
      Lock.party.read(h) <> Lock.parties.read(t)

def Lock.holder(l: NamedLock) -> Maybe<&2, LockParty>:
  match l:
    case NamedLock{h, _, _, _}:
      h

def Lock.waiters(l: NamedLock) -> List<&2, LockParty>:
  match l:
    case NamedLock{_, q, _, _}:
      q

def Lock.ready(l: NamedLock) -> Bool:
  match l:
    case NamedLock{_, _, r, _}:
      r

def Lock.outside(l: NamedLock) -> Maybe<&2, LockParty>:
  match l:
    case NamedLock{_, _, _, o}:
      o

def Lock.owner.h(h: Maybe<&2, LockParty>) -> String:
  match h:
    case None{}:
      ""
    case Some{p}:
      Lock.id(p)

def Lock.owner(l: NamedLock) -> String:
  Lock.owner.h(Lock.holder(l))

def Lock.holder.json(h: Maybe<&2, LockParty>) -> J.Json:
  match h:
    case None{}:
      J.Null{}
    case Some{p}:
      Lock.party.json(p)

# Who holds the OS flock outside the hub: id = pid, note = command line.
def Lock.outside.kv(o: Maybe<&2, LockParty>) -> List<&2, J.KV>:
  match o:
    case None{}:
      Nil{}
    case Some{LockParty{id, _, _, note, since, _}}:
      [J.KV{"outside", J.Json.obj([J.KV{"pid", J.Num{id}}, J.KV{"cmd", J.Str{note}}, J.KV{"since", J.Num{Nat.show(since)}}])}]

def Lock.outside.read(+o: J.Json) -> Maybe<&2, LockParty>:
  match o:
    case J.Null{}:
      None{}
    case _:
      Some{LockParty{Nat.show(J.Json.get_nat(o, "pid", 0n)), "outside", "", J.Json.get_str(o, "cmd", ""), J.Json.get_nat(o, "since", 0n), 0n}}

def Lock.json(l: NamedLock) -> J.Json:
  match l:
    case NamedLock{h, q, r, o}:
      J.Json.obj(List.append(&2, J.KV, [J.KV{"holder", Lock.holder.json(h)}, J.KV{"waiters", J.Json.arr(Lock.parties.json(q))}, J.KV{"ready", J.Flag{r}}], Lock.outside.kv(o)))

def Lock.holder.read(o: J.Json) -> Maybe<&2, LockParty>:
  match o:
    case J.Null{}:
      None{}
    case _:
      Some{Lock.party.read(o)}

def Lock.read(+o: J.Json) -> NamedLock:
  NamedLock{Lock.holder.read(J.Json.get_obj(o, "holder")), Lock.parties.read(J.Json.elems(J.Json.get_obj(o, "waiters"))), J.Json.get_bool(o, "ready", False{}), Lock.outside.read(J.Json.get_obj(o, "outside"))}

def Lock.position.q(q: List<&2, LockParty>, +id: String, +n: Nat) -> Nat:
  match q:
    case Nil{}:
      0n
    case +h <> t:
      Bool.pick(Nat, String.eq(Lock.id(h), id), n, Lock.position.q(t, id, 1n+n))

def Lock.position(+l: NamedLock, +id: String) -> Nat:
  Lock.position.q(Lock.waiters(l), id, 1n)

def Lock.has(+l: NamedLock, +id: String) -> Bool:
  Bool.or(String.eq(Lock.owner(l), id), Nat.is_gt(Lock.position(l, id), 0n))

def Lock.acquire.k(had: Bool, l: NamedLock, +p: LockParty, +ready: Bool) -> NamedLock:
  match had l:
    case True{} _:
      l
    case False{} NamedLock{None{}, Nil{}, _, _}:
      NamedLock{Some{p}, Nil{}, ready, None{}}
    case False{} NamedLock{h, q, r, o}:
      NamedLock{h, List.append(&2, LockParty, q, [p]), r, o}

def Lock.acquire(+l: NamedLock, +p: LockParty, ready: Bool) -> NamedLock:
  Lock.acquire.k(Lock.has(l, Lock.id(p)), l, p, ready)

def Lock.promote(q: List<&2, LockParty>) -> NamedLock:
  match q:
    case Nil{}:
      Lock.empty()
    case h <> t:
      NamedLock{Some{h}, t, False{}, None{}}

def Lock.release.k(own: Bool, l: NamedLock) -> NamedLock:
  match own l:
    case True{} NamedLock{_, q, _, _}:
      Lock.promote(q)
    case False{} _:
      l

def Lock.release(+l: NamedLock, +id: String) -> NamedLock:
  Lock.release.k(Bool.and(Bool.not(String.is_empty(id)), String.eq(Lock.owner(l), id)), l)

def Lock.cancel.q(q: List<&2, LockParty>, +id: String) -> List<&2, LockParty>:
  match q:
    case Nil{}:
      Nil{}
    case +h <> t:
      List.append(&2, LockParty, Bool.pick(List<&2, LockParty>, String.eq(Lock.id(h), id), Nil{}, [h]), Lock.cancel.q(t, id))

def Lock.cancel(l: NamedLock, +id: String) -> NamedLock:
  match l:
    case NamedLock{h, q, r, o}:
      NamedLock{h, Lock.cancel.q(q, id), r, o}

# Expiry also removes a queued party, so retired owners cannot re-enter.
def Lock.expire(+l: NamedLock, +id: String) -> NamedLock:
  Lock.cancel(Lock.release(l, id), id)

def Lock.renew.p(p: LockParty, +id: String, +lease: Nat) -> LockParty:
  match p:
    case LockParty{+i, k, th, n, s, old}:
      LockParty{i, k, th, n, s, Bool.pick(Nat, String.eq(i, id), lease, old)}

def Lock.renew.q(q: List<&2, LockParty>, +id: String, +lease: Nat) -> List<&2, LockParty>:
  match q:
    case Nil{}:
      Nil{}
    case h <> t:
      Lock.renew.p(h, id, lease) <> Lock.renew.q(t, id, lease)

def Lock.renew.h(h: Maybe<&2, LockParty>, +id: String, +lease: Nat) -> Maybe<&2, LockParty>:
  match h:
    case None{}:
      None{}
    case Some{p}:
      Some{Lock.renew.p(p, id, lease)}

def Lock.renew(l: NamedLock, +id: String, +lease: Nat) -> NamedLock:
  match l:
    case NamedLock{h, q, r, o}:
      NamedLock{Lock.renew.h(h, id, lease), Lock.renew.q(q, id, lease), r, o}

# The guard is taken: nobody outside the hub holds the flock any more.
def Lock.mark(l: NamedLock) -> NamedLock:
  match l:
    case NamedLock{h, q, _, _}:
      NamedLock{h, q, True{}, None{}}

def Lock.outside.set(l: NamedLock, o: Maybe<&2, LockParty>) -> NamedLock:
  match l:
    case NamedLock{h, q, r, _}:
      NamedLock{h, q, r, o}

# After a restart every client party gets a fresh lease: the downtime must not
# prune a live helper. Whoever held the flock outside is probed again.
def Lock.extend.p(p: LockParty, +lease: Nat) -> LockParty:
  match p:
    case LockParty{id, "client", th, n, s, _}:
      LockParty{id, "client", th, n, s, lease}
    case _:
      p

def Lock.extend.q(q: List<&2, LockParty>, +lease: Nat) -> List<&2, LockParty>:
  match q:
    case Nil{}:
      Nil{}
    case h <> t:
      Lock.extend.p(h, lease) <> Lock.extend.q(t, lease)

def Lock.extend.h(h: Maybe<&2, LockParty>, +lease: Nat) -> Maybe<&2, LockParty>:
  match h:
    case None{}:
      None{}
    case Some{p}:
      Some{Lock.extend.p(p, lease)}

def Lock.extend(l: NamedLock, +lease: Nat) -> NamedLock:
  match l:
    case NamedLock{h, q, r, _}:
      NamedLock{Lock.extend.h(h, lease), Lock.extend.q(q, lease), r, None{}}

def Lock.name.chars(s: String) -> Bool:
  match s:
    case SNil{}:
      True{}
    case SCon{+c, t}:
      Bool.and(Bool.or(Char.is_alpha(c), Bool.or(Char.is_digit(c), Bool.or(Char.is_eq(c, '-'), Char.is_eq(c, '_')))), Lock.name.chars(t))

def Lock.name.valid(+s: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(s)), Bool.and(Nat.is_le(String.length(s), 64n), Lock.name.chars(s)))

def Lock.holder.list(h: Maybe<&2, LockParty>) -> List<&2, LockParty>:
  match h:
    case None{}:
      Nil{}
    case Some{p}:
      [p]

def Lock.count.h(h: Maybe<&2, LockParty>) -> Nat:
  match h:
    case None{}:
      0n
    case Some{_}:
      1n

def Lock.count(l: NamedLock) -> Nat:
  Lock.count.h(Lock.holder(l))

def Lock.ordinal(+n: Nat) -> String:
  +last = Nat.mod(n, 10n)
  +teen = Nat.mod(n, 100n)
  Nat.show(n) ++ Bool.pick(String, Bool.and(Nat.is_ge(teen, 11n), Nat.is_le(teen, 13n)), "th", Bool.pick(String, Nat.is_eq(last, 1n), "st", Bool.pick(String, Nat.is_eq(last, 2n), "nd", Bool.pick(String, Nat.is_eq(last, 3n), "rd", "th"))))

def Lock.wait.word(+name: String, +position: Nat) -> String:
  "waiting for lock " ++ name ++ " (" ++ Bool.pick(String, Nat.is_eq(position, 0n), "OS flock", Lock.ordinal(position)) ++ ")"

# Lease policy never authorizes signalling a process. Busy owners keep their
# exclusion while the hub reports the missing heartbeat.
def Lock.lease.expired(+lease: Nat, +now: Nat) -> Bool:
  Bool.and(Nat.is_gt(lease, 0n), Nat.is_ge(now, lease))

def Lock.lease.release(+expired: Bool, +warned: Bool, +grace: Bool, busy: Bool) -> Bool:
  Bool.and(expired, Bool.and(warned, Bool.and(grace, Bool.not(busy))))

def Lock.heartbeat.p(+p: LockParty, +thread: String, +lease: Nat) -> LockParty:
  Bool.pick(LockParty, Bool.and(String.eq(Lock.thread(p), thread), Bool.not(String.eq(Lock.kind(p), "client"))), Lock.renew.p(p, Lock.id(p), lease), p)

def Lock.heartbeat.q(ps: List<&2, LockParty>, +thread: String, +lease: Nat) -> List<&2, LockParty>:
  match ps:
    case Nil{}: Nil{}
    case p <> t: Lock.heartbeat.p(p, thread, lease) <> Lock.heartbeat.q(t, thread, lease)

def Lock.heartbeat.h(h: Maybe<&2, LockParty>, +thread: String, +lease: Nat) -> Maybe<&2, LockParty>:
  match h:
    case None{}: None{}
    case Some{p}: Some{Lock.heartbeat.p(p, thread, lease)}

def Lock.heartbeat(l: NamedLock, +thread: String, +lease: Nat) -> NamedLock:
  match l:
    case NamedLock{h, q, r, o}:
      NamedLock{Lock.heartbeat.h(h, thread, lease), Lock.heartbeat.q(q, thread, lease), r, o}
