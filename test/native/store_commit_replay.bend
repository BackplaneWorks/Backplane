import Base
import ../../src/server/argv.bend as A
import ../../src/server/storedb.bend as SO
import ../../src/server/host.bend as H
import ../../src/server/db.bend as D
import ../../src/core/store.bend as ST
import ../../src/core/codec.bend as C
import ../../src/core/json.bend as J
import ../../src/core/model.bend as M
import ../../src/core/storedb.bend as SDB

# HOME/store.db written by the hub's own commit path (SO.Sto.step, then
# SO.Sto.commit every 1000 lines, as Hub.fold and Hub.commit.k do), from
# HOME/events.jsonl, with no hub: so a store kept up commit by commit can be
# compared with the fold at a real log's scale (S3's snap_equiv). The home
# must have no store.db written yet (one made empty by this program is
# fine). JS only, a copy of a log only, never a live home:
#   BUN_JSC_maxPerThreadStackUsage=536870912 bend test/native/store_commit_replay.bend HOME
# Lines that do not decode (a torn write, a newer hub's change) never reach
# a commit; they are counted and their bytes would shift later offsets, so a
# log with any is reported and the store is not to be trusted.

type Part is Data:
  Part{rest: List<&2, String>, st: M.State, cm: SO.Cm, skipped: Nat}

def Rp.stepped(p: M.State & SO.Cm, +sk: Nat) -> Part:
  (st, cm) = p
  Part{Nil{}, st, cm, sk}

def Rp.dec(m: Maybe<&2, M.Change>, +o: J.Json, +l: String, +st: M.State, +cm: SO.Cm, +sk: Nat) -> Part:
  match m:
    case Some{+c}:
      Rp.stepped(SO.Sto.step(st, c, l, C.Change.wire_line(o, l), cm), sk)
    case None{}:
      Part{Nil{}, st, cm, 1n+sk}

def Rp.parsed(m: Maybe<&2, J.Json>, +l: String, +st: M.State, +cm: SO.Cm, +sk: Nat) -> Part:
  match m:
    case Some{+o}:
      Rp.dec(C.Change.decode(o), o, l, st, cm, sk)
    case None{}:
      Part{Nil{}, st, cm, 1n+sk}

def Rp.one(+l: String, +st: M.State, +cm: SO.Cm, +sk: Nat) -> Part:
  Rp.parsed(J.Json.parse(l), l, st, cm, sk)

# one line folded into the batch so far (not recursive: the walk below
# calls only itself)
def Rp.fold(+l: String, acc: Part) -> Part:
  match acc:
    case Part{_, +st, +cm, +sk}:
      Rp.one(l, st, cm, sk)

def Rp.rest(acc: Part, rest: List<&2, String>) -> Part:
  match acc:
    case Part{_, st, cm, sk}:
      Part{rest, st, cm, sk}

# up to k lines folded through the commit path's step; what is left rides
# in the answer's rest
def Rp.batch(ls: List<&2, String>, +k: Nat, acc: Part) -> Part:
  match ls k:
    case Nil{} _:
      Rp.rest(acc, Nil{})
    case +l <> +t 0n:
      Rp.rest(acc, l <> t)
    case +l <> t 1n+p:
      Rp.batch(t, p, Rp.fold(l, acc))

def Rp.off(c: SO.Cm) -> Nat:
  match c:
    case SO.Cm{_, o, _, _}:
      o

def Rp.done(+sk: Nat, lg: SO.Lg) -> IO(Unit):
  IO.print("lines=" ++ Nat.show(SO.Lg.n(lg)) ++ " skipped=" ++ Nat.show(sk))

# commit the batch, then fold the next (fuel: more batches than the log has)
def Rp.loop(fuel: Nat, p: Part) -> IO(Unit):
  match fuel p:
    case 0n Part{_, _, +cm, +sk}:
      IO.bind(SO.Lg, Unit, SO.Sto.commit(cm), lg => Rp.done(sk, lg))
    case 1n+f Part{Nil{}, _, +cm, +sk}:
      IO.bind(SO.Lg, Unit, SO.Sto.commit(cm), lg => Rp.done(sk, lg))
    case 1n+f Part{+l <> +t, +st, +cm, +sk}:
      IO.bind(SO.Lg, Unit, SO.Sto.commit(cm), lg => Rp.loop(f, Rp.batch(l <> t, 1000n, Part{Nil{}, st, SO.Cm.new(lg, Rp.off(cm)), sk})))

def Rp.empty(rs: List<&2, List<&2, String>>) -> Bool:
  match rs:
    case (+v <> _) <> _:
      String.is_empty(v)
    case _:
      True{}

def Rp.go(fresh: Bool, +h: String, +home: String) -> IO(Unit):
  match fresh:
    case False{}:
      IO.die(Unit, 1, "store_commit_replay: " ++ home ++ "/store.db already holds a store (meta applied_seq set); use a home without one")
    case True{}:
      do IO<Unit>:
        +rd : List<&2, String> <- H.EvLog.read_from(home ++ "/events.jsonl", "0")
        +ls : List<&2, String> = ST.Store.read.body(rd)
        +lg : SO.Lg = SO.Lg{h, home ++ "/events.jsonl", 0n, 1n, ""}
        # what Sto.boot writes into a store it makes (the hot boot reads codec)
        SO.Sto.script(h, [SO.Sto.meta("codec", Nat.show(SDB.Sdb.codec())), SO.Sto.meta("schema", "1")])
        Rp.loop(Nat.add(2n, Nat.div(List.length(&2, String, ls), 1000n)), Rp.batch(ls, 1000n, Part{Nil{}, M.State.empty(), SO.Cm.new(lg, 0n), 0n}))

def Rp.run(+home: String) -> IO(Unit):
  do IO<Unit>:
    +h : String <- SO.Sto.open.db(home ++ "/store.db")
    rs : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT v FROM meta WHERE k = 'applied_seq'", [], 1)
    Rp.go(Rp.empty(rs), h, home)

def Main.go(xs: List<String>) -> IO(Unit):
  match xs:
    case Nil{}:
      IO.die(Unit, 2, "usage: store_commit_replay HOME")
    case +home <> _:
      Rp.run(home)

def main() -> IO(Unit):
  IO.bind(List<String>, Unit, A.Argv.args(), Main.go)
