import Base
import ./check.bend as C
import ../src/core/locks.bend as L
import ../src/core/model.bend as M
import ../src/core/hub.bend as H
import ../src/core/json.bend as J

def bool(b: Bool) -> String:
  Bool.pick(String, b, "true", "false")

def state() -> M.State:
  M.State.replay([M.LockChanged{"bend", L.Lock.acquire(L.Lock.acquire(L.Lock.empty(), L.LockParty{"ta", "thread", "ta", "build", 2n, 100n}, True{}), L.LockParty{"tb", "thread", "tb", "next", 3n, 0n}, False{}), 3n}], H.Locks.demo())

def warned() -> M.State:
  +s = state()
  M.State.replay(H.Queue.changes(H.Locks.after(s, 100n, H.Step.none())), s)

def ended() -> M.State:
  M.State.replay([M.TurnChanged{"ta", M.Completed{}, 120n}], warned())

def released() -> M.State:
  +s = ended()
  M.State.replay(H.Queue.changes(H.Locks.after(s, 161n, H.Step.none())), s)

def holder(+s: M.State) -> L.LockParty:
  Maybe.default(&2, L.LockParty, L.Lock.holder(M.Locks.get(s, "bend")), L.LockParty{"", "", "", "", 0n, 0n})

def count(+s: M.State, +now: Nat, +th: String) -> Nat:
  H.Lr.count(H.Queue.changes(H.Locks.after(s, now, H.Step.none())), th)

def heartbeat() -> M.State:
  +s = warned()
  M.State.replay(H.Queue.changes(H.Locks.heartbeat(s, 110n, "ta")), s)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("no expiry before deadline", bool(L.Lock.lease.expired(100n, 99n)), "false")
    C.Check.str("lease expiry includes deadline", bool(L.Lock.lease.expired(100n, 100n)), "true")
    C.Check.str("legacy missing lease is initialized", bool(L.Lock.lease.expired(0n, 100n)), "false")
    C.Check.str("busy lease cannot release", bool(L.Lock.lease.release(True{}, True{}, True{}, True{})), "false")
    C.Check.str("lease needs warning", bool(L.Lock.lease.release(True{}, False{}, True{}, False{})), "false")
    C.Check.str("lease needs warning grace", bool(L.Lock.lease.release(True{}, True{}, False{}, False{})), "false")
    C.Check.str("lease expires idle owner after grace", bool(L.Lock.lease.release(True{}, True{}, True{}, False{})), "true")
    C.Check.str("first expiry warns holder once", Nat.show(count(state(), 100n, "ta")), "1")
    C.Check.str("warning keeps reservation", L.Lock.owner(M.Locks.get(warned(), "bend")), "ta")
    C.Check.str("repeated expiry has no second warning", Nat.show(count(warned(), 101n, "ta")), "0")
    C.Check.str("warning grace retains reservation", L.Lock.owner(M.Locks.get(M.State.replay(H.Queue.changes(H.Locks.after(ended(), 159n, H.Step.none())), ended()), "bend")), "ta")
    C.Check.str("busy holder retains expired reservation", L.Lock.owner(M.Locks.get(M.State.replay(H.Queue.changes(H.Locks.after(warned(), 170n, H.Step.none())), warned()), "bend")), "ta")
    C.Check.str("idle stale lease promotes FIFO head", L.Lock.owner(M.Locks.get(released(), "bend")), "tb")
    C.Check.str("expired owner gets release notice", Nat.show(count(ended(), 161n, "ta")), "1")
    C.Check.str("waiter gets stale release notice", Nat.show(count(ended(), 161n, "tb")), "1")
    C.Check.str("real heartbeat renews lease", Nat.show(L.Lock.lease(holder(heartbeat()))), "1310")
    C.Check.str("renewed lease ignores old warning", bool(H.Locks.warning.same(heartbeat(), "bend", holder(heartbeat()))), "false")
    C.Check.str("heartbeat does not change FIFO", Nat.show(L.Lock.position(M.Locks.get(heartbeat(), "bend"), "tb")), "1")
    C.Check.str("locked work survives recovery until checked", L.Lock.owner(M.Locks.get(M.State.replay(H.Queue.changes(H.Locks.after(H.Locks.demo.work(), 5n, H.Locks.recover(H.Locks.demo.work(), 5n))), H.Locks.demo.work()), "bend")), H.Watch.id(H.Locks.demo(), "ta"))
