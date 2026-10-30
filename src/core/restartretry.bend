import Base
import ./hub.bend as B
import ./model.bend as M
import ./json.bend as J

# Carry's immediate anti-loop remains unchanged. A second restart before
# output gets a persisted, bounded delayed retry rather than silent idleness.
# This planner never signals a process; it only records and starts new turns.
def Retry.key(th: String) -> String:
  "restart.retry." ++ th

def Retry.count.key(th: String) -> String:
  "restart.retry.n." ++ th

def Retry.target(+t: M.Thread) -> String:
  B.Hub.provider(t) ++ "/" ++ B.Turn.model(t)

def Retry.activity(t: M.Thread) -> Nat:
  match t:
    case M.Thread{_, _, _, _, at, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}: at

def Retry.interrupted(t: M.TurnState) -> Bool:
  match t:
    case M.Interrupted{}: True{}
    case _: False{}

def Retry.kept(ids: List<&2, String>, +th: String) -> Bool:
  match ids:
    case Nil{}: False{}
    case +id <> rest: Bool.or(String.eq(id, th), Retry.kept(rest, th))

# Require an actual user continuation message, not arbitrary assistant text
# that happens to mention a restart. Only Carry's own restart notes are skipped.
def Retry.carry(es: List<&2, M.Entry>) -> Bool:
  match es:
    case Nil{}: False{}
    case M.Act{_, _, _, "restart", _, _} <> rest: Retry.carry(rest)
    case M.Msg{_, _, M.User{}, text, _} <> _: B.Carry.is(text)
    case _: False{}

def Retry.user(es: List<&2, M.Entry>) -> String:
  match es:
    case Nil{}: ""
    case M.Msg{id, _, M.User{}, _, _} <> _: id
    case _ <> rest: Retry.user(rest)

def Retry.count(s: M.State, th: String) -> Nat:
  B.WorkTimeout.parse(M.Setting.get(s, Retry.count.key(th), "0"), 0n)

def Retry.next(+n: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_ge(n, 20n), 20n, 1n+n)

def Retry.delay(n: Nat) -> Nat:
  Nat.mul(60n, Retry.next(n))

def Retry.record(s: M.State, th: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(M.Setting.get(s, Retry.key(th), "")), J.Null{})

def Retry.due(s: M.State, th: String) -> Nat:
  J.Json.get_nat(Retry.record(s, th), "due", 0n)

def Retry.candidate(+before: M.State, +after: M.State, +kept: List<&2, String>, +th: String) -> Bool:
  Bool.and(B.Carry.deferred(before, th), Bool.and(M.Carry.on(after), Bool.and(B.Carry.live(B.Hub.thread(after, th)),
    Bool.and(Bool.not(Retry.kept(kept, th)), Bool.and(Retry.interrupted(M.Thread.turn(B.Hub.thread(after, th))),
      Bool.and(String.eq(Retry.target(B.Hub.thread(before, th)), Retry.target(B.Hub.thread(after, th))),
        String.is_empty(M.Setting.get(after, Retry.key(th), ""))))))))

def Retry.schedule(go: Bool, +s: M.State, +now: Nat, +th: String) -> B.Step:
  match go:
    case False{}: B.Step.none()
    case True{}:
      +n = Retry.count(s, th)
      +due = Nat.add(now, Retry.delay(n))
      B.Step{[M.SettingSet{Retry.count.key(th), Nat.show(Retry.next(n)), now},
        M.SettingSet{Retry.key(th), J.Json.show(J.Json.obj([J.KV{"due", J.Json.nat(due)},
          J.KV{"target", J.Str{Retry.target(B.Hub.thread(s, th))}}, J.KV{"activity", J.Json.nat(Retry.activity(B.Hub.thread(s, th)))},
          J.KV{"message", J.Str{Retry.user(M.State.mine(s, th))}}])), now},
        M.ActivityLogged{th, B.Hub.id("retry", now, B.Hub.seq(s)), "warn", "restart-retry", "Restart interrupted a continuation before output; will retry in " ++ Nat.show(Retry.delay(n)) ++ " seconds. No busy work was stopped.", now}], Nil{}}

def Retry.after.threads(ts: List<&2, M.Thread>, +before: M.State, +after: M.State, +kept: List<&2, String>, +now: Nat) -> B.Step:
  match ts:
    case Nil{}: B.Step.none()
    case +t <> rest:
      +st = Retry.schedule(Retry.candidate(before, after, kept, M.Thread.id(t)), after, now, M.Thread.id(t))
      B.Step.cat(st, Retry.after.threads(rest, before, M.State.replay(B.Queue.changes(st), after), kept, now))

def Retry.after(+before: M.State, after: M.State, kept: List<&2, String>, now: Nat) -> B.Step:
  Retry.after.threads(B.Rec.threads(before), before, after, kept, now)

def Retry.asks(xs: List<&2, M.Ask>, +th: String) -> Bool:
  match xs:
    case Nil{}: False{}
    case M.Ask{_, +id, _, _, _, _, +open, _} <> rest:
      Bool.or(Bool.and(open, String.eq(id, th)), Retry.asks(rest, th))

def Retry.valid(+s: M.State, +th: String) -> Bool:
  +r = Retry.record(s, th)
  +t = B.Hub.thread(s, th)
  Bool.and(Bool.and(M.Carry.on(s), Bool.not(M.Dormant.of(s, t))), Bool.and(B.Carry.live(t), Bool.and(Retry.interrupted(M.Thread.turn(t)),
    Bool.and(String.eq(J.Json.get_str(r, "target", ""), Retry.target(t)),
      Bool.and(Nat.is_eq(J.Json.get_nat(r, "activity", 0n), Retry.activity(t)),
        String.eq(J.Json.get_str(r, "message", ""), Retry.user(M.State.mine(s, th))))))))

def Retry.ready(+s: M.State, +now: Nat, +th: String) -> Bool:
  Bool.and(Retry.valid(s, th), Bool.and(Nat.is_gt(Retry.due(s, th), 0n), Bool.and(Nat.is_ge(now, Retry.due(s, th)),
    Bool.and(Nat.is_eq(B.Hub.pending(B.Hub.thread(s, th)), 0n), Bool.not(Retry.asks(M.Asks.all(s), th))))))

def Retry.text() -> String:
  M.Carry.text() ++ " Continue from your last step. Read the saved tool results and inspect any work still running before repeating an action. Do not repeat completed work."

def Retry.resume(go: Bool, +s: M.State, +now: Nat, +th: String) -> B.Step:
  match go:
    case False{}: B.Step.none()
    case True{}:
      +cs = {[M.SettingSet{Retry.key(th), "", now}] : List<&2, M.Change>}
      B.Step.before(cs, Nil{}, B.Step.after(B.Hub.post(M.State.replay(cs, s), now, th, B.Hub.id("retry", now, B.Hub.seq(s)), M.User{}, Retry.text()),
        [M.ActivityLogged{th, B.Hub.id("retry-start", now, B.Hub.seq(s)), "info", "restart", "Continuing after the restart retry delay", now}]))

def Retry.one(present: Bool, +s: M.State, +now: Nat, +th: String) -> B.Step:
  match present:
    case False{}: B.Step.none()
    case True{}:
      Bool.pick(B.Step, Retry.valid(s, th), Retry.resume(Retry.ready(s, now, th), s, now, th),
        B.Step{[M.SettingSet{Retry.key(th), "", now}], Nil{}})

def Retry.threads(ts: List<&2, M.Thread>, +s: M.State, +now: Nat) -> B.Step:
  match ts:
    case Nil{}: B.Step.none()
    case +t <> rest:
      +st = Retry.one(Bool.not(String.is_empty(M.Setting.get(s, Retry.key(M.Thread.id(t)), ""))), s, now, M.Thread.id(t))
      B.Step.cat(st, Retry.threads(rest, M.State.replay(B.Queue.changes(st), s), now))

def Retry.tick(+s: M.State, now: Nat) -> B.Step:
  Retry.threads(B.Rec.threads(s), s, now)
