import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/locks.bend as LK
import ../src/core/json.bend as J

# Per-frame reads that no longer walk every setting (fast hub R1, the log
# copy's profile): Work.rows.map against its specification
# Work.rows.map.all, Pfx.keys against a filter of Map.keys, and an idle lock
# told without parsing it.

def keys() -> List<&2, String>:
  ["bg.t1", "bg.t2", "bg.", "bga", "b", "bg", "watch.t1", "watch.t3", "watch", "watch.", "watchers", "wake.x", "bg.é", "bg.t1x", "summary.t1",
   "lock.bend", "locks.names", "w", "", "bg.ü日本", "watch.t2", "zz", "a", "ba", "bh.q", "bg.t9"]

def sets(ks: List<&2, String>, +i: Nat) -> List<&2, M.Change>:
  match ks:
    case Nil{}:
      Nil{}
    case k <> t:
      M.SettingSet{k, "{\"rows\":[{\"title\":\"x\"}]}" , i} <> sets(t, 1n+i)

def st() -> M.State:
  M.State.replay(List.append(&2, M.Change, sets(keys(), 1n), [M.WorkSet{"t1", "bg", "[{\"id\":\"a\",\"title\":\"build\"}]", 40n}, M.WorkSet{"t2", "watch", "[{\"id\":\"b\",\"title\":\"wait\"}]", 41n}]), M.State.empty())

def show.nat(xs: List<&2, Sigma<&2, &2, String, _ => Nat>>) -> String:
  match xs:
    case Nil{}:
      ""
    case (k, v) <> t:
      k ++ "=" ++ Nat.show(v) ++ " " ++ show.nat(t)

def filter(ks: List<&2, String>, +pre: String) -> List<&2, String>:
  match ks:
    case Nil{}:
      Nil{}
    case +k <> t:
      List.append(&2, String, Bool.pick(List<&2, String>, String.starts_with(k, pre), [k], Nil{}), filter(t, pre))

def same.pre(+pre: String) -> IO(Unit):
  C.Check.str("Pfx.keys " ++ pre, String.join(M.Setting.keys.pre(st(), pre), ","), String.join(filter(M.Setting.keys(st()), pre), ","))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("Work.rows.map is Work.rows.map.all", show.nat(Map.to_list(&2, Nat, M.Work.rows.map(st()))), show.nat(Map.to_list(&2, Nat, M.Work.rows.map.all(st()))))
    same.pre("bg.")
    same.pre("watch.")
    same.pre("b")
    same.pre("")
    same.pre("bg.t1")
    same.pre("bg.é")
    same.pre("watch.t")
    same.pre("nothing")
    same.pre("lock.")
    same.pre("bg.t1xyzzy")
    C.Check.str("a lock never set is idle", Bool.show(M.Locks.idle(M.State.empty(), "bend")), "True")
    C.Check.str("an idle lock reads as empty", J.Json.show(LK.Lock.json(M.Locks.get(M.State.replay([M.LockChanged{"bend", LK.Lock.empty(), 1n}], M.State.empty()), "bend"))), J.Json.show(LK.Lock.json(LK.Lock.empty())))
    C.Check.str("a lock released is idle", Bool.show(M.Locks.idle(M.State.replay([M.LockChanged{"bend", LK.Lock.empty(), 1n}], M.State.empty()), "bend")), "True")
