import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/hub.bend as H
import ../src/core/mailowner.bend as O
import ../src/core/mail.bend as ML
import ../src/core/mailbody.bend as MB
import ../src/core/mailtriage.bend as T
import ../src/core/ubersync.bend as U
import ../src/core/ubop.bend as OP
import ../src/core/json.bend as J

def yes(label: String, b: Bool) -> IO(Unit):
  C.Check.str(label, Bool.show(b), "True")

def count(label: String, n: Nat, want: Nat) -> IO(Unit):
  C.Check.str(label, Nat.show(n), Nat.show(want))

def body(+id: String) -> MB.Body:
  MB.Body{ML.Conv{"c", "worker outcome", "launch", ["thread:worker", "bot:b1"], "p", Nil{}, 1n, 3n, 1n},
    ML.Mail{id, "c", "thread:worker", "bot:b1", "launch", "worker", "fyi", "waiting result", 14n, 3n}, "waiting result"}

def local() -> M.State:
  M.State.replay([M.SettingSet{"uber.link.u1-x.mB", "LB", 1n}], U.Ub.demo.a())

def away(+s: M.State) -> M.State:
  M.State.apply(s, U.Ub.demo.op("mB", 1n, 1001n, OP.Ub.b.lease("mB", 2n)))

def receiver() -> M.State:
  M.State.replay([M.SettingSet{"uber.link.u1-x.mA", "LA", 1n}, U.Ub.demo.op("mB", 1n, 1001n, OP.Ub.b.lease("mB", 2n))], U.Ub.demo.b())

def filed(+s: M.State, +b: MB.Body) -> M.State:
  M.State.replay(List.append(&2, M.Change, H.Mail.pending(String.is_empty(ML.Conv.id(ML.Box.conv(M.State.mail(s), ML.Mail.conv(MB.Body.mail(b))))), b), O.Owner.track(s, b, 4n)), s)

def routed(+s: M.State, +b: MB.Body) -> M.State:
  M.State.replay(H.Queue.changes(H.Mail.route(s, 5n, b)), s)

def first(+s: M.State) -> J.Json:
  Maybe.default(&2, J.Json, List.head(&2, J.Json, O.Owner.records(s)), J.Null{})

def planned(+s: M.State) -> M.State:
  M.State.replay(O.Owner.plan(s, first(s), 6n), s)

def packet(+s: M.State, +b: MB.Body) -> J.Json:
  J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(O.Owner.request(s, first(s), b)), J.Null{}), "p")

def imported(+s: M.State, +p: J.Json) -> MB.Body:
  Maybe.default(&2, MB.Body, O.Owner.import.body(s, p), body("INVALID"))

def received(+s: M.State, +p: J.Json) -> M.State:
  +b = imported(s, p)
  M.State.replay(List.append(&2, M.Change, H.Mail.pending(True{}, b), O.Owner.import.changes(s, p, b, 8n)), s)

def queue(+s: M.State) -> List<&2, String>:
  T.Triage.queue(s, "b1", "c")

def main() -> IO(Unit):
  do IO<Unit>:
    +b : MB.Body = body("m1")
    +here : M.State = filed(local(), b)
    +started : M.State = routed(here, b)
    count("header commits waiting ticket", List.length(&2, J.Json, O.Owner.records(here)), 1n)
    count("same-owner RunSet claims ticket", List.length(&2, J.Json, O.Owner.records(started)), 0n)
    yes("same-owner creates private run", Bool.not(String.is_empty(ML.Box.live(M.State.mail(started), "b1", "c"))))
    +remote : M.State = filed(away(local()), b)
    +noLocalRun : M.State = routed(remote, b)
    yes("remote owner starts no local run", String.is_empty(ML.Box.live(M.State.mail(noLocalRun), "b1", "c")))
    count("remote waiting input retained", List.length(&2, J.Json, O.Owner.records(noLocalRun)), 1n)
    +send : M.State = planned(noLocalRun)
    C.Check.str("recipient is persisted before IO", J.Json.get_str(first(send), "to", ""), "mB")
    +p : J.Json = packet(send, b)
    +dst : M.State = receiver()
    yes("remote owner accepts exact lease", O.Owner.accept(dst, "LA", p))
    yes("wrong signed machine binding refused", Bool.not(O.Owner.accept(dst, "other", p)))
    +got : MB.Body = imported(dst, p)
    C.Check.str("full text preserved", MB.Body.text(got), "waiting result")
    C.Check.str("origin thread is qualified", ML.Mail.from(MB.Body.mail(got)), "thread:LA~worker")
    C.Check.str("source reference is qualified", ML.Mail.ref(MB.Body.mail(got)), "LA~worker")
    C.Check.str("destination is holder's bot", ML.Mail.to(MB.Body.mail(got)), "bot:b2")
    C.Check.str("FYI provenance preserved", ML.Mail.wanted(MB.Body.mail(got)), "fyi")
    count("original source timestamp preserved", ML.Mail.at(MB.Body.mail(got)), 3n)
    yes("original participants retained", ML.Parts.has(ML.Conv.parts(MB.Body.conv(got)), "thread:LA~worker", False{}))
    +accepted : M.State = received(dst, p)
    yes("ACK receipt is durable with header", O.Owner.seen(accepted, p))
    +runningThere : M.State = routed(accepted, got)
    count("receiver claims exact input once", List.length(&2, J.Json, O.Owner.records(runningThere)), 0n)
    +movedAgain : M.State = M.State.apply(runningThere, U.Ub.demo.op("mA", 1n, 1002n, OP.Ub.b.lease("mA", 3n)))
    yes("old receipt survives a lease transition", O.Owner.seen(movedAgain, p))
    yes("old lease cannot accept a new input", Bool.not(O.Owner.accept(movedAgain, "LA", p)))
    yes("HTTP200 unknown-method is not ACK", Bool.not(O.Owner.response.is("{\"ok\":false}", first(send), "committed")))
    yes("exact-key ACK recognized", O.Owner.response.is(O.Owner.response(first(send), "committed"), first(send), "committed"))
    count("sender retires only acknowledged ticket", List.length(&2, J.Json, O.Owner.records(M.State.replay(O.Owner.sent(send, first(send), 9n), send))), 0n)
    +waiting : M.State = routed(filed(started, body("m2")), body("m2"))
    count("second input waits behind existing run", List.length(&2, String, queue(waiting)), 1n)
    count("queued input retains outbox responsibility", List.length(&2, J.Json, O.Owner.records(waiting)), 1n)
    +transition : M.State = away(waiting)
    yes("lease move stops queue from starting locally", List.is_empty(&2, M.Change, H.Queue.changes(H.Run.queue.start(transition, 12n, "b1", "c"))))
    +more : M.State = routed(filed(transition, body("m3")), body("m3"))
    +sentOne : M.State = M.State.replay(O.Owner.sent(more, first(more), 13n), more)
    count("ACK removes exactly one waiting ID", List.length(&2, J.Json, O.Owner.records(sentOne)), 1n)
    yes("in-flight first input remains claimed", String.eq(M.Setting.get(transition, "run.bound." ++ ML.Box.live(M.State.mail(transition), "b1", "c"), ""), "m1"))
    +legacy : M.State = M.State.replay(List.append(&2, M.Change, H.Mail.pending(True{}, body("legacy")), [T.Triage.queue.set("b1", "c", ["legacy"], 1n)]), away(local()))
    count("pre-upgrade waiting ID is enrolled", List.length(&2, J.Json, O.Owner.records(M.State.replay(O.Owner.legacy(legacy, 14n), legacy))), 1n)
    +history : M.State = M.State.replay(H.Mail.pending(True{}, body("old")), local())
    count("ordinary historical mail is not replayed", List.length(&2, M.Change, O.Owner.legacy(history, 14n)), 0n)
    +recovered : List<&2, M.Change> = H.Mail.pending(True{}, body("torn-header"))
    count("torn header recovery enrolls only new input", List.length(&2, J.Json, O.Owner.records(M.State.replay(List.append(&2, M.Change, recovered, O.Owner.recovered(local(), recovered, 15n)), local()))), 1n)
    +returned : M.State = M.State.replay(O.Owner.import.changes(send, p, b, 16n), send)
    count("stale ACK cannot retire a returned ticket", List.length(&2, M.Change, O.Owner.sent(returned, first(send), 17n)), 0n)
    +stillPending : M.State = M.State.apply(send, U.Ub.demo.op("mA", 1n, 1002n, OP.Ub.b.lease("mA", 3n)))
    C.Check.str("unanswered attempt stays pinned after move", J.Json.get_str(first(stillPending), "to", ""), "mB")
    yes("typed reports remain private with remote owner", H.Operational.is(away(local()), "t1"))
