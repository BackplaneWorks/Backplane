import Base
import ./check.bend as C
import ../src/core/locks.bend as L
import ../src/core/model.bend as M
import ../src/core/hub.bend as H
import ../src/core/codec.bend as K
import ../src/core/json.bend as J
import ../src/core/plane.bend as P
import ../src/core/subs.bend as S
import ../src/core/client.bend as U
import ../src/core/mcp.bend as MC

def party(+id: String) -> L.LockParty:
  L.LockParty{id, "thread", id, "build", 2n, 0n}

def queue() -> L.NamedLock:
  L.Lock.acquire(L.Lock.acquire(L.Lock.acquire(L.Lock.empty(), party("ta"), True{}), party("tb"), False{}), party("tc"), False{})

def bool(b: Bool) -> String:
  Bool.pick(String, b, "true", "false")

def round(+c: M.Change) -> Bool:
  String.eq(J.Json.show(K.Change.encode(c)), J.Json.show(K.Change.encode(Maybe.default(&2, M.Change, K.Change.decode(K.Change.encode(c)), M.LockChanged{"bad", L.Lock.empty(), 0n}))))

def expired() -> String:
  +s = M.State.replay([M.LockChanged{"lease", L.Lock.acquire(L.Lock.empty(), L.LockParty{"pid:1", "client", "", "shell", 2n, 5n}, True{}), 2n}], H.Locks.demo())
  L.Lock.owner(M.Locks.get(M.State.replay(H.Queue.changes(H.Locks.after(s, 6n, H.Step.none())), s), "lease"))

def archived() -> String:
  +s = H.Locks.demo.queue()
  L.Lock.owner(M.Locks.get(M.State.replay(H.Queue.changes(H.Locks.after(s, 4n, H.Step{[M.ThreadArchived{"ta", True{}, 4n}], Nil{}})), s), "bend"))

def work.queue() -> M.State:
  +s = H.Locks.demo.queue()
  M.State.replay(H.Queue.changes(H.Locks.work.start(s, 4n, "ta", J.Json.obj([J.KV{"command", J.Str{"true"}}, J.KV{"lock", J.Str{"bend"}}]), False{})), s)

def work.cancel() -> String:
  +s = work.queue()
  +id = H.Watch.id(H.Locks.demo.queue(), "ta")
  L.Lock.owner(M.Locks.get(M.State.replay(H.Queue.changes(H.Locks.after(s, 5n, H.Watch.cancel(s, 5n, "ta", id))), s), "bend")) ++ ":" ++ Nat.show(L.Lock.position(M.Locks.get(M.State.replay(H.Queue.changes(H.Locks.after(s, 5n, H.Watch.cancel(s, 5n, "ta", id))), s), "bend"), id))

def work.restarted() -> String:
  +s = H.Locks.demo.work()
  L.Lock.owner(M.Locks.get(M.State.replay(H.Queue.changes(H.Locks.after(s, 5n, H.Locks.recover(s, 5n))), s), "bend"))

def work.timeout() -> String:
  +s = H.Locks.demo.work()
  L.Lock.owner(M.Locks.get(M.State.replay(H.Queue.changes(H.Locks.after(s, 4n, H.Watch.end(s, 4n, "ta", H.Watch.id(H.Locks.demo(), "ta"), "timeout", ""))), s), "bend"))

def outside() -> M.State:
  +s = H.Locks.demo.release()
  M.State.replay(H.Queue.changes(H.Locks.outside(s, 7n, "bend", "tb", 3662856n, "flock /tmp/bp-bend.lock sh verify.sh")), s)

def outside.cleared() -> M.State:
  +s = outside()
  M.State.replay(H.Queue.changes(H.Locks.ready(s, 8n, "bend", "tb")), s)

def has.outside.k(o: Maybe<&2, L.LockParty>) -> Bool:
  match o:
    case None{}:
      False{}
    case Some{_}:
      True{}

def has.outside(+l: L.NamedLock) -> Bool:
  has.outside.k(L.Lock.outside(l))

def outside.quiet() -> Nat:
  +s = outside()
  List.length(&2, M.Change, H.Queue.changes(H.Locks.outside(s, 9n, "bend", "tb", 3662856n, "flock /tmp/bp-bend.lock sh verify.sh")))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("free acquire grants", L.Lock.owner(queue()), "ta")
    C.Check.str("FIFO B then C", L.Lock.owner(L.Lock.release(queue(), "ta")) ++ ":" ++ L.Lock.owner(L.Lock.release(L.Lock.release(queue(), "ta"), "tb")), "tb:tc")
    C.Check.str("duplicate acquire keeps position", Nat.show(L.Lock.position(L.Lock.acquire(queue(), party("tb"), True{}), "tb")), "1")
    C.Check.str("duplicate holder keeps queue", Nat.show(List.length(&2, L.LockParty, L.Lock.waiters(L.Lock.acquire(queue(), party("ta"), True{})))), "2")
    C.Check.str("non holder cannot release", L.Lock.owner(L.Lock.release(queue(), "tb")), "ta")
    C.Check.str("pure expiry releases holder", L.Lock.owner(L.Lock.expire(queue(), "ta")), "tb")
    C.Check.str("cancel shifts FIFO position", Nat.show(L.Lock.position(L.Lock.cancel(queue(), "tb"), "tc")), "1")
    C.Check.str("release reserves head until OS guard", bool(L.Lock.ready(L.Lock.release(queue(), "ta"))), "false")
    C.Check.str("grant sends one message", Nat.show(H.Lr.count(H.Queue.changes(H.Locks.demo.grant()), "tb")), "1")
    C.Check.str("grant repeated sends no message", Nat.show(H.Locks.demo.twice()), "0")
    C.Check.str("deleted holder frees", H.Locks.demo.deleted(), "tb")
    C.Check.str("archived holder frees", archived(), "tb")
    C.Check.str("external lease expires", expired(), "")
    C.Check.str("timeout retains lock over live work", work.timeout(), H.Watch.id(H.Locks.demo(), "ta"))
    C.Check.str("work end releases", H.Locks.demo.work.end(), "")
    C.Check.str("queued work cancellation removes waiter", work.cancel(), "ta:0")
    C.Check.str("restart retains locked work until checked", work.restarted(), H.Watch.id(H.Locks.demo(), "ta"))
    C.Check.str("codec held queue round trip", bool(round(M.LockChanged{"bend", queue(), 5n})), "true")
    C.Check.str("codec empty round trip", bool(round(M.LockChanged{"bend", L.Lock.empty(), 6n})), "true")
    C.Check.str("indexed name listed once", Nat.show(List.length(&2, String, M.Locks.names(M.State.replay([M.LockChanged{"bend", queue(), 8n}], H.Locks.demo.queue())))), "1")
    C.Check.str("plane includes lock", Nat.show(List.length(&2, P.PRow, P.Plane.locks(P.Plane.of.st(H.Locks.demo.queue(), 9n)))), "1")
    C.Check.str("holder has release row", S.Subs.cancel(S.Subs.lock.row("bend", queue(), "ta")), "lock-release")
    C.Check.str("waiter has cancel row", S.Subs.cancel(S.Subs.lock.row("bend", queue(), "tb")), "lock-cancel")
    C.Check.str("waiting lock counts as waiting", Nat.show(S.Subs.waiting([S.Subs.lock.row("bend", queue(), "tb")])), "1")
    C.Check.str("waiting lock is not running work", Nat.show(S.Subs.live([S.Subs.lock.row("bend", queue(), "tb")])), "0")
    C.Check.str("invalid path name rejected", bool(L.Lock.name.valid("../bend")), "false")
    C.Check.str("status requeues a lost client", H.Locks.demo.requeue("status", "pid:9"), "2:1:lost")
    C.Check.str("renew requeues a lost client", H.Locks.demo.requeue("renew", "pid:9"), "2:1:lost")
    C.Check.str("known waiter is not requeued", H.Locks.demo.requeue("status", "tb"), "1:1:kept")
    C.Check.str("requeue happens once", Nat.show(H.Locks.demo.requeue.twice()), "2")
    C.Check.str("restart extends client leases", H.Locks.demo.lease(), "96:1")
    C.Check.str("outside holder recorded", L.Lock.id(Maybe.default(&2, L.LockParty, L.Lock.outside(M.Locks.get(outside(), "bend")), party("none"))) ++ "|" ++ L.Lock.note(Maybe.default(&2, L.LockParty, L.Lock.outside(M.Locks.get(outside(), "bend")), party("none"))), "3662856|flock /tmp/bp-bend.lock sh verify.sh")
    C.Check.str("outside holder repeat writes nothing", Nat.show(outside.quiet()), "0")
    C.Check.str("outside cleared when guard taken", bool(has.outside(M.Locks.get(outside.cleared(), "bend"))), "false")
    C.Check.str("outside holder in lock_list", bool(String.contains(J.Json.show(J.Json.arr(H.Locks.list.go(M.Locks.names(outside()), outside()))), "\"outside\":{\"pid\":3662856,\"cmd\":\"flock /tmp/bp-bend.lock sh verify.sh\"")), "true")
    C.Check.str("outside holder in plane row", bool(String.contains(P.PRow.title(Maybe.default(&2, P.PRow, List.head(&2, P.PRow, P.Plane.locks(P.Plane.of.st(outside(), 9n))), P.PRow{"", "", "", "", "", "", "", "", 0n, False{}})), "bend · held outside the hub by pid 3662856 (flock /tmp/bp-bend.lock sh verify.sh) · 0 waiting")), "true")
    C.Check.str("outside codec round trip", bool(round(M.LockChanged{"bend", L.Lock.outside.set(queue(), Some{L.LockParty{"12", "outside", "", "flock x", 3n, 0n}}), 5n})), "true")
