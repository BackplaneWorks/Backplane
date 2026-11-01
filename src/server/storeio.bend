import Base
import ./db.bend as D
import ./perf.bend as PS
import ./hotboot.bend as HB
import ./stobuild.bend as SB
import ./host.bend as H
import ../core/storedb.bend as S
import ../core/snapshot.bend as SN
import ../core/model.bend as M
import ../core/codec.bend as C
import ../core/json.bend as J
import ../core/utf8.bend as U
import ../core/far.bend as F

# The event log is authoritative. Facts, affected records and the applied
# byte/sequence/ordinal checkpoint advance together in one SQLite transaction.
# A log append is acknowledged only after this transaction also succeeds.
type Plan is Data:
  Plan{st: M.State, counts: Map<&2, Nat>, seq: Nat, off: Nat, ord: Nat, items: List<&2, List<&2, String>>}

def Disk.read(path: String, off: String) -> IO(List<&2, String>):
  import "./effects/storeio.js"

def Disk.cut(path: String, len: String) -> IO(Bool):
  import "./effects/storeio.js"

def Disk.temp(home: String) -> IO(String):
  import "./effects/storeio.js"

def Disk.promote(home: String, temp: String) -> IO(String):
  import "./effects/storeio.js"

def Disk.sync(path: String) -> IO(String):
  import "./effects/storeio.js"

# the store's checks before it is opened: the log's tail closed, a prepared
# store adopted, an incompatible one set aside. codec is S.Sdb.codec()'s
# text and from S.Sdb.codec.from()'s, space-separated: the versions the JS
# checks keep (they hold none of their own: test/tools/
# store_checkpoint_io_test.cjs); a prepared store must be at codec itself
def Disk.prepare(home: String, codec: String, from: String) -> IO(String):
  import "./effects/storeio.js"

# the complete lines before byte `to` that contain any of the needles, oldest
# first, decoded as UTF-8 (effects/storeio.js reads the file outside Bend)
def Disk.pick(path: String, to: String, needles: List<&2, String>) -> IO(List<&2, String>):
  import "./effects/storeio.js"

def Plan.st(p: Plan) -> M.State:
  match p:
    case Plan{s, _, _, _, _, _}: s

def Plan.meta(+seq: Nat, +off: Nat, +ord: Nat) -> List<&2, List<&2, String>>:
  [SB.Stb.meta.sql("applied_seq", Nat.show(seq)), SB.Stb.meta.sql("applied_off", Nat.show(off)), SB.Stb.meta.sql("ord", Nat.show(ord))]

def Plan.far(xs: List<&2, F.Fact>, acc: List<&2, List<&2, String>>) -> List<&2, List<&2, String>>:
  SB.Stb.far(xs, acc)

def Plan.change(+c: M.Change, +line: String, +len: Nat, p: Plan) -> Plan:
  match p:
    case Plan{+pre, +cnt, +seq, +off, +ord, items}:
      +post : M.State = SB.Stb.apply(pre, cnt, c)
      +ops = List.append(&2, S.Op, S.Sdb.facts(pre, S.At{1n+seq, off, len, ord}, c), S.Row.ofs(S.RK{"state", "seq"} <> S.Sdb.keys(pre, post, c), post, Nil{}))
      Plan{post, SB.Stb.bump(S.F.posts(c), cnt), 1n+seq, Nat.add(off, 1n+len), S.Sdb.ord(c, ord), Plan.far(F.Fr.fact(post, c, 1n+seq), SB.Stb.sqls(ops, items))}

def Plan.decoded(m: Maybe<&2, M.Change>, +line: String, +len: Nat, p: Plan) -> Plan:
  match m p:
    case Some{c} _: Plan.change(c, line, len, p)
    case None{} Plan{st, cnt, +seq, +off, +ord, items}:
      Plan{st, cnt, 1n+seq, Nat.add(off, 1n+len), ord, SB.Stb.sqls([S.F.line(S.At{1n+seq, off, len, ord}, "")], items)}

def Plan.origin(+o: J.Json, p: Plan) -> Plan:
  match p:
    case Plan{st, cnt, 0n, off, ord, items}:
      Plan{st, cnt, 0n, off, ord, SB.Stb.meta.sql("origin", Nat.show(J.Json.get_nat(o, "at", 0n))) <> items}
    case other: other

def Plan.parsed(m: Maybe<&2, J.Json>, +line: String, +len: Nat, p: Plan) -> Plan:
  match m p:
    case Some{+o} _: Plan.decoded(C.Change.decode(SB.Stb.flat(o)), line, len, Plan.origin(o, p))
    case None{} Plan{st, cnt, seq, off, ord, items}: Plan{st, cnt, seq, Nat.add(off, 1n+len), ord, items}

def Plan.line(+line: String, p: Plan) -> Plan:
  Plan.parsed(J.Json.parse(line), line, List.length(&2, U32, U.Utf8.encode(line)), p)

def Plan.lines(ls: List<&2, String>, p: Plan) -> Plan:
  match ls:
    case Nil{}: p
    case l <> t: Plan.lines(t, Plan.line(l, p))

def Plan.items(p: Plan) -> List<&2, List<&2, String>>:
  match p:
    case Plan{_, _, seq, off, ord, items}: List.append(&2, List<&2, String>, List.reverse(&2, List<&2, String>, items), Plan.meta(seq, off, ord))

def Plan.next(p: Plan) -> Plan:
  match p:
    case Plan{st, cnt, seq, off, ord, _}: Plan{SB.Stb.strip(st), cnt, seq, off, ord, Nil{}}

def Sto.tail(fuel: Nat, +ls: List<&2, String>, +h: String, p: Plan) -> IO(Plan):
  match fuel ls:
    case 0n _: IO.pure(Plan, p)
    case _ Nil{}: IO.pure(Plan, p)
    case 1n+f _:
      do IO<Plan>:
        +next : Plan = Plan.lines(List.take(&2, String, ls, 128n), p)
        result : String <- D.Db.script(h, Plan.items(next))
        SB.Stb.check("incremental checkpoint", result)
        IO.sleep(0)
        Sto.tail(f, List.drop(&2, String, ls, 128n), h, Plan.next(next))

def Plan.off(p: Plan) -> Nat:
  match p:
    case Plan{_, _, _, off, _, _}: off

# Native IO cuts at complete UTF-8 line boundaries. Only this bounded batch
# is retained; the in-memory fold drops entries after every transaction.
def Sto.chunks(fuel: Nat, empty: Bool, +h: String, +home: String, +p: Plan) -> IO(Plan):
  match fuel empty:
    case 0n _: IO.pure(Plan, p)
    case _ True{}: IO.pure(Plan, p)
    case 1n+f False{}:
      do IO<Plan>:
        +batch : List<&2, String> <- Disk.read(home ++ "/events.jsonl", Nat.show(Plan.off(p)))
        SB.Stb.check("bounded log read", Maybe.default(&2, String, List.head(&2, String, batch), "fail missing batch"))
        +ls : List<&2, String> = List.drop(&2, String, batch, 1n)
        +next : Plan <- Sto.tail(1n+List.length(&2, String, ls), ls, h, p)
        Sto.chunks(f, List.is_empty(&2, String, ls), h, home, next)

type Opened is Data:
  Opened{handle: String, warm: Maybe<&2, HB.Warm>}

def Opened.handle(o: Opened) -> String:
  match o:
    case Opened{h, _}: h

def Opened.warm(o: Opened) -> Maybe<&2, HB.Warm>:
  match o:
    case Opened{_, warm}: warm

def Plan.threads(p: Plan) -> Map<&2, M.Thread>:
  match p:
    case Plan{M.State{_, ts, _, _, _, _, _, _, _, _, _}, _, _, _, _, _}: ts

def Plan.warm(+p: Plan, home: String, log: String) -> HB.Warm:
  match p:
    case Plan{_, _, +seq, +off, _, _}:
      HB.Warm{home, Nat.show(seq), Nat.show(off), log, Plan.threads(p)}

def Sto.catchup.read(empty: Bool, +h: String, +home: String, +meta: Map<&2, String>, off: Nat, size: Nat) -> IO(Opened):
  match empty:
    case True{}: IO.pure(Opened, Opened{h, None{}})
    case False{}:
      do IO<Opened>:
        +records : Map<&2, List<&2, List<&2, String>>> <- HB.Hot.read.recs(h)
        +rows : SN.SRows = SN.SRows{records}
        +p : Plan <- Sto.chunks(1n+size, False{}, h, home, Plan{SN.Snap.state(rows), SN.Snap.counts(rows), SN.Snap.num(HB.Hot.get(meta, "applied_seq"), 0n), off, Nat.max(1n, SN.Snap.num(HB.Hot.get(meta, "ord"), 0n)), Nil{}})
        return Opened{h, Some{Plan.warm(p, home, HB.Hot.get(meta, "log_id"))}}

def Sto.catchup(+h: String, +home: String, +meta: Map<&2, String>) -> IO(Opened):
  do IO<Opened>:
    +off : Nat = SN.Snap.num(HB.Hot.get(meta, "applied_off"), 0n)
    +size : U32 <- HB.Hot.size(home ++ "/events.jsonl")
    Sto.catchup.read(Nat.is_eq(off, U32.to_nat(size)), h, home, meta, off, U32.to_nat(size))

def Sto.rebuild(+home: String) -> IO(Opened):
  do IO<Opened>:
    +temp : String <- Disk.temp(home)
    b : SB.Built <- SB.Stb.build(temp, 65536)
    result : String <- Disk.promote(home, temp)
    SB.Stb.check("atomic store promotion", result)
    opened : String <- D.Db.open(home ++ "/store.db")
    +h : String <- SB.Stb.open(opened)
    durable : String <- D.Db.exec(h, "PRAGMA synchronous=FULL")
    SB.Stb.check("durable rebuilt checkpoint", durable)
    return Opened{h, None{}}

def Sto.checked(ok: Bool, +h: String, +home: String, +meta: Map<&2, String>) -> IO(Opened):
  match ok:
    case True{}:
      do IO<Opened>:
        sync : String <- D.Db.exec(h, "PRAGMA synchronous=FULL")
        SB.Stb.check("durable checkpoints", sync)
        Sto.catchup(h, home, meta)
    case False{}:
      do IO<Opened>:
        closed : String <- D.Db.close(h)
        Sto.rebuild(home)

# Upgrade in place (storedb.bend's "Upgrades in place")
# -----------------------------------------------------
# A store whose only difference is a codec in S.Sdb.codec.from() is brought
# to this codec without a rebuild: the picked lines before its applied_off
# are folded (S.Up.changes), their rows and meta's codec written in one
# transaction, then read back. If anything fails the store is rebuilt from
# the log as before; a store is never left half upgraded (the transaction
# commits whole or not at all), and the log says which path ran.

def Sto.up.said(+msg: String) -> IO(Unit):
  PS.Log.info("store", msg)

def Sto.up.count(rows: List<&2, List<&2, String>>) -> Nat:
  M.Switch.num(Maybe.default(&2, String, List.head(&2, String, Maybe.default(&2, List<&2, String>, List.head(&2, List<&2, String>, rows), Nil{})), ""), 0n)

def Sto.up.verified(ok: Bool, +h: String, +home: String, +meta: Map<&2, String>, +what: String) -> IO(Opened):
  match ok:
    case True{}:
      do IO<Opened>:
        Sto.up.said("upgraded store.db in place from codec " ++ HB.Hot.get(meta, "codec") ++ " to " ++ Nat.show(S.Sdb.codec()) ++ ": " ++ what)
        Sto.checked(True{}, h, home, Map.set(&2, String, meta, "codec", Nat.show(S.Sdb.codec())))
    case False{}:
      do IO<Opened>:
        Sto.up.said("in-place upgrade failed (" ++ what ++ "); rebuilding store.db from the log")
        closed : String <- D.Db.close(h)
        Sto.rebuild(home)

def Sto.up.check(+r: String, +h: String, +home: String, +meta: Map<&2, String>, +want: Nat, +picked: Nat) -> IO(Opened):
  do IO<Opened>:
    n : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT count(*) FROM setting WHERE substr(k, 1, 14) = 'thread.parent.'", [], 2)
    c : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT v FROM meta WHERE k = 'codec'", [], 2)
    +got : Nat = Sto.up.count(n)
    +what : String = Nat.show(picked) ++ " lines picked, " ++ Nat.show(want) ++ " thread.parent rows written, " ++ Nat.show(got) ++ " read back"
    Sto.up.verified(Bool.and(String.starts_with(r, "ok"), Bool.and(Nat.is_eq(got, want), String.eq(Nat.show(Sto.up.count(c)), Nat.show(S.Sdb.codec())))), h, home, meta, Bool.pick(String, String.starts_with(r, "ok"), what, r))

def Sto.upgrade(+h: String, +home: String, +meta: Map<&2, String>) -> IO(Opened):
  do IO<Opened>:
    +ls : List<&2, String> <- Disk.pick(home ++ "/events.jsonl", HB.Hot.get(meta, "applied_off"), S.Up.names())
    +cs : List<&2, M.Change> = S.Up.changes(ls)
    r : String <- D.Db.script(h, S.Up.rows(cs))
    Sto.up.check(r, h, home, meta, S.Up.count(cs), List.length(&2, String, ls))

def Sto.up.or(up: Bool, ok: Bool, +h: String, +home: String, +meta: Map<&2, String>) -> IO(Opened):
  match up:
    case True{}:
      Sto.upgrade(h, home, meta)
    case False{}:
      Sto.checked(ok, h, home, meta)

def Sto.opened(ok: Bool, +opened: String, +home: String) -> IO(Opened):
  match ok:
    case False{}: Sto.rebuild(home)
    case True{}:
      do IO<Opened>:
        +h : String = String.drop(opened, 3n)
        rows : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT k,v FROM meta", [], 1000)
        +meta : Map<&2, String> = HB.Hot.meta.go(rows, Map.new(&2, String))
        +size : U32 <- HB.Hot.size(home ++ "/events.jsonl")
        head : List<&2, U32> <- HB.Hot.head(home ++ "/events.jsonl")
        +id : String = S.Sdb.log_id(head)
        +why : String = HB.Hot.check(meta, size, id)
        +only : Bool = String.is_empty(HB.Hot.check(Map.set(&2, String, meta, "codec", Nat.show(S.Sdb.codec())), size, id))
        Sto.up.or(Bool.and(Bool.not(String.is_empty(why)), Bool.and(only, S.Sdb.codec.up(HB.Hot.get(meta, "codec")))), String.is_empty(why), h, home, meta)

def Sto.from.go(xs: List<&2, Nat>) -> String:
  match xs:
    case Nil{}:
      ""
    case x <> t:
      Nat.show(x) ++ " " ++ Sto.from.go(t)

def Sto.boot(+home: String) -> IO(Opened):
  do IO<Opened>:
    ready : String <- Disk.prepare(home, Nat.show(S.Sdb.codec()), String.trim(Sto.from.go(S.Sdb.codec.from())))
    SB.Stb.check("log tail", ready)
    +opened : String <- D.Db.open(home ++ "/store.db")
    Sto.opened(String.starts_with(opened, "ok "), opened, home)

# Keep the handle-only API for writers and existing checkpoint callers.
def Sto.open(home: String) -> IO(String):
  do IO<String>:
    o : Opened <- Sto.boot(home)
    return Opened.handle(o)

type Updated is Data:
  Updated{st: M.State, result: String}

def Sto.providers(ls: List<&2, String>) -> Bool:
  match ls:
    case Nil{}: False{}
    case +l <> t: Bool.or(String.contains(l, "\"ProviderSet\""), Sto.providers(t))

def Sto.counts(need: Bool, +h: String) -> IO(Map<&2, Nat>):
  match need:
    case False{}: IO.pure(Map<&2, Nat>, Map.new(&2, Nat))
    case True{}:
      do IO<Map<&2, Nat>>:
        rows : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT thread,count(*) FROM entry GROUP BY thread", [], 4000000000)
        return SN.Fill.counts(rows, Map.new(&2, Nat))

def Sto.write(ok: Bool, +sync: String, +h: String, +home: String, +p: Plan) -> IO(Updated):
  match ok:
    case False{}: IO.pure(Updated, Updated{Plan.st(p), sync})
    case True{}:
      do IO<Updated>:
        head : List<&2, U32> <- HB.Hot.head(home ++ "/events.jsonl")
        result : String <- D.Db.script(h, List.append(&2, List<&2, String>, Plan.items(p), [SB.Stb.meta.sql("log_id", S.Sdb.log_id(head))]))
        return Updated{Plan.st(p), result}

def Sto.committed(ok: Bool, +h: String, +home: String, st: M.State, +lines: List<&2, String>, +off: Nat, +meta: Map<&2, String>) -> IO(Updated):
  match ok:
    case False{}: IO.pure(Updated, Updated{st, "fail store checkpoint does not match the log append"})
    case True{}:
      do IO<Updated>:
        counts : Map<&2, Nat> <- Sto.counts(Sto.providers(lines), h)
        +p : Plan = Plan.lines(lines, Plan{st, counts, SN.Snap.num(HB.Hot.get(meta, "applied_seq"), 0n), off, Nat.max(1n, SN.Snap.num(HB.Hot.get(meta, "ord"), 0n)), Nil{}})
        +sync : String <- Disk.sync(home ++ "/events.jsonl")
        Sto.write(String.eq(sync, "ok"), sync, h, home, p)


def Sto.commit(+h: String, +home: String, st: M.State, lines: List<&2, String>, +off: Nat) -> IO(Updated):
  do IO<Updated>:
    rows : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT k,v FROM meta", [], 1000)
    +meta : Map<&2, String> = HB.Hot.meta.go(rows, Map.new(&2, String))
    Sto.committed(Nat.is_eq(off, SN.Snap.num(HB.Hot.get(meta, "applied_off"), 0n)), h, home, st, lines, off, meta)
