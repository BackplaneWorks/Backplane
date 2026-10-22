import Base
import ../core/json.bend as J
import ../core/model.bend as M
import ../core/codec.bend as C
import ../core/storedb.bend as SDB
import ../core/utf8.bend as U
import ./db.bend as D

# The store's first build (store.db from events.jsonl, docs:
# bp-coord/store/S1-SCHEMA.md)
# ---------------------------------------------------------------
# The log is read a chunk at a time (File.read_at), never whole: a chunk's
# complete lines are folded and their facts written in one Db.script; the
# partial last line is read again with the next chunk, and a line longer
# than a chunk makes the chunk grow until it fits. The fold keeps no
# entries (Stb.strip after every chunk): the one step that reads them,
# ProviderSet's Switch.count, is given a counter of each thread's own
# entries instead (M.State.switch.c). After the last line every record of
# the final state is written (SDB.Dump.ops), with the counters as
# obj('count', <thread>, n) and the meta rows. Numbering is SDB.Mat.step's:
# a line that is not JSON takes its bytes but no number, a line no change
# decodes from gets a line row (wire "") and is not folded; a trailing
# fragment with no newline is left for the hub (it never reads one either).

# Lines of a chunk
# ----------------

# a complete line: its length in bytes (no newline) and its text
type Ln is Data:
  Ln{len: Nat, text: String}

# a chunk cut into lines: the complete ones (oldest first), the bytes they
# take with their newlines, and the bytes the chunk had
type Cut is Data:
  Cut{lines: List<&2, Ln>, used: Nat, total: Nat}

# the walk: the line so far (reversed) and its length, the lines done
# (newest first), the bytes they take, the bytes seen
type CutS is Data:
  CutS{part: List<&2, U32>, plen: Nat, done: List<&2, Ln>, used: Nat, total: Nat}

def Cut.nl(nl: Bool, +x: U32, part: List<&2, U32>, +plen: Nat, done: List<&2, Ln>, +used: Nat, +total: Nat) -> CutS:
  match nl:
    case True{}:
      CutS{Nil{}, 0n, Ln{plen, U.Utf8.decode(List.reverse(&2, U32, part))} <> done, Nat.add(total, 1n), Nat.add(total, 1n)}
    case False{}:
      CutS{x <> part, 1n+plen, done, used, Nat.add(total, 1n)}

def Cut.step(s: CutS, +x: U32) -> CutS:
  match s:
    case CutS{part, +plen, done, +used, +total}:
      Cut.nl(U32.is_eq(x, 10), x, part, plen, done, used, total)

def Cut.fin(s: CutS) -> Cut:
  match s:
    case CutS{_, _, done, used, total}:
      Cut{List.reverse(&2, Ln, done), used, total}

def Cut.go(bs: List<&2, U32>, s: CutS) -> Cut:
  match bs:
    case Nil{}:
      Cut.fin(s)
    case x <> t:
      Cut.go(t, Cut.step(s, x))

def Cut.of(bs: List<&2, U32>) -> Cut:
  Cut.go(bs, CutS{Nil{}, 0n, Nil{}, 0n, 0n})

# The fold without entries
# ------------------------

def Stb.ext(x: M.Ext) -> M.Ext:
  match x:
    case M.Ext{w, _, mail}:
      M.Ext{w, Map.new(&2, List<&2, M.Entry>), mail}

def Stb.exts(xs: List<&2, M.Ext>) -> List<&2, M.Ext>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      Stb.ext(x) <> Stb.exts(t)

# the state with no entries kept (State.entries and the Idx by thread
# emptied): what the build carries between chunks
def Stb.strip(s: M.State) -> M.State:
  match s:
    case M.State{ps, ts, _, n, tk, sg, qu, ak, dv, nv, bw}:
      M.State{ps, ts, Nil{}, n, tk, sg, qu, ak, dv, nv, Stb.exts(bw)}

def Stb.got(r: Map<&2, Nat> & Nat) -> Nat:
  (_, n) = r
  n

# a thread's own entries so far (Switch.count's answer)
def Stb.count(cnt: Map<&2, Nat>, +th: String) -> Nat:
  Stb.got(Map.get(Nat, 0n, cnt, th))

def Stb.bump.k(r: Map<&2, Nat> & Nat, +th: String) -> Map<&2, Nat>:
  (m, n) = r
  Map.set(&2, Nat, m, th, 1n+n)

def Stb.bump(m: Maybe<&2, M.Entry>, cnt: Map<&2, Nat>) -> Map<&2, Nat>:
  match m:
    case Some{+e}:
      +th = M.Idx.thread(e)
      Stb.bump.k(Map.get(Nat, 0n, cnt, th), th)
    case None{}:
      cnt

# one change folded: M.State.apply, but a ProviderSet takes the thread's
# count from the counters
def Stb.apply(+st: M.State, +cnt: Map<&2, Nat>, +c: M.Change) -> M.State:
  match c:
    case M.ProviderSet{+id, +p, m, _}:
      M.State.tick(M.State.switch.c(st, id, p, m, Stb.count(cnt, id)))
    case other:
      M.State.apply(st, other)

# The build
# ---------

# what the build carries: the state, the counters, the last line's number,
# the next line's offset, the next entry's ordinal, this chunk's statements
# (newest first), the log's id (SDB.Sdb.log_id) and origin ("" until the
# first line)
type Acc is Data:
  Acc{st: M.State, cnt: Map<&2, Nat>, seq: Nat, off: Nat, ord: Nat, items: List<&2, List<&2, String>>, id: String, origin: String}

def Acc.new(id: String) -> Acc:
  Acc{M.State.empty(), Map.new(&2, Nat), 0n, 0n, 1n, Nil{}, id, ""}

def Stb.sqls(os: List<&2, SDB.Op>, acc: List<&2, List<&2, String>>) -> List<&2, List<&2, String>>:
  match os:
    case Nil{}:
      acc
    case o <> t:
      Stb.sqls(t, SDB.Sql.of(o) <> acc)

# the log's origin: the "at" of its first line that parses (Hub.origin)
def Stb.origin(+seq: Nat, +o: J.Json, origin: String) -> String:
  match seq:
    case 0n:
      Nat.show(J.Json.get_nat(o, "at", 0n))
    case _:
      origin

def Stb.change(m: Maybe<&2, M.Change>, +st: M.State, +cnt: Map<&2, Nat>, +seq: Nat, +off: Nat, +len: Nat, +ord: Nat, items: List<&2, List<&2, String>>, id: String, origin: String) -> Acc:
  match m:
    case Some{+c}:
      +p = SDB.F.posts(c)
      +cnt2 = Stb.bump(p, cnt)
      Acc{Stb.apply(st, cnt, c), cnt2, seq, Nat.add(off, 1n+len), SDB.Sdb.ord(c, ord), Stb.sqls(SDB.Sdb.facts(st, SDB.At{seq, off, len, ord}, c), items), id, origin}
    case None{}:
      Acc{st, cnt, seq, Nat.add(off, 1n+len), ord, Stb.sqls([SDB.F.line(SDB.At{seq, off, len, ord}, "")], items), id, origin}

def Stb.parsed(m: Maybe<&2, J.Json>, st: M.State, cnt: Map<&2, Nat>, +seq: Nat, +off: Nat, +len: Nat, ord: Nat, items: List<&2, List<&2, String>>, id: String, origin: String) -> Acc:
  match m:
    case Some{+o}:
      Stb.change(C.Change.decode(o), st, cnt, 1n+seq, off, len, ord, items, id, Stb.origin(seq, o, origin))
    case None{}:
      Acc{st, cnt, seq, Nat.add(off, 1n+len), ord, items, id, origin}

# one line: numbered after the last, starting where it ended
def Stb.line(a: Acc, l: Ln) -> Acc:
  match a l:
    case Acc{st, cnt, +seq, +off, ord, items, id, origin} Ln{+len, +text}:
      Stb.parsed(J.Json.parse(text), st, cnt, seq, off, len, ord, items, id, origin)

def Stb.lines(ls: List<&2, Ln>, a: Acc) -> Acc:
  match ls:
    case Nil{}:
      a
    case l <> t:
      Stb.lines(t, Stb.line(a, l))

# a chunk's statements, oldest first, and the accumulator without them and
# without entries
def Acc.take(a: Acc) -> List<&2, List<&2, String>> & Acc:
  match a:
    case Acc{st, cnt, seq, off, ord, items, id, origin}:
      (List.reverse(&2, List<&2, String>, items), Acc{Stb.strip(st), cnt, seq, off, ord, Nil{}, id, origin})

def Acc.off(a: Acc) -> Nat:
  match a:
    case Acc{_, _, _, off, _, _, _, _}:
      off

# The end: every record, the counters and meta
# --------------------------------------------

def Stb.counts(xs: List<&2, Sigma<&2, &2, String, _ => Nat>>) -> List<&2, SDB.Op>:
  match xs:
    case Nil{}:
      Nil{}
    case (k, n) <> t:
      SDB.Put{"obj", ["count", k, Nat.show(n)]} <> Stb.counts(t)

def Stb.meta.sql(+k: String, v: String) -> List<&2, String>:
  ["INSERT OR REPLACE INTO meta(k, v) VALUES (?, ?)", k, v]

# the statements written after the last line
def Stb.end(a: Acc) -> List<&2, List<&2, String>>:
  match a:
    case Acc{+st, cnt, +seq, +off, +ord, _, id, origin}:
      List.append(&2, List<&2, String>, List.reverse(&2, List<&2, String>, Stb.sqls(Stb.counts(Map.to_list(&2, Nat, cnt)), Stb.sqls(SDB.Dump.ops(st), Nil{}))), [
        Stb.meta.sql("schema", "1"),
        Stb.meta.sql("codec", Nat.show(SDB.Sdb.codec())),
        Stb.meta.sql("applied_seq", Nat.show(seq)),
        Stb.meta.sql("applied_off", Nat.show(off)),
        Stb.meta.sql("ord", Nat.show(ord)),
        Stb.meta.sql("log_id", id),
        Stb.meta.sql("origin", origin)])

def Acc.seq(a: Acc) -> Nat:
  match a:
    case Acc{_, _, seq, _, _, _, _, _}:
      seq

def Acc.ord(a: Acc) -> Nat:
  match a:
    case Acc{_, _, _, _, ord, _, _, _}:
      ord

def Acc.st(a: Acc) -> M.State:
  match a:
    case Acc{st, _, _, _, _, _, _, _}:
      st

# IO
# --

def Stb.ok.k(ok: Bool, +what: String, +r: String) -> IO(Unit):
  match ok:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      IO.die(Unit, 1, "store build: " ++ what ++ ": " ++ r)

# stop the build on a "fail ..." answer
def Stb.check(+what: String, +r: String) -> IO(Unit):
  Stb.ok.k(String.starts_with(r, "ok"), what, r)

def Stb.write.k(+h: String, r: List<&2, List<&2, String>> & Acc) -> IO(Acc):
  (items, +a) = r
  do IO<Acc>:
    +res : String <- D.Db.script(h, items)
    Stb.check("chunk ending at " ++ Nat.show(Acc.off(a)), res)
    return a

# a chunk's lines folded and written in one transaction; answers the
# accumulator after them, entries stripped
def Stb.write(+h: String, ls: List<&2, Ln>, a: Acc) -> IO(Acc):
  Stb.write.k(h, Acc.take(Stb.lines(ls, a)))

# what to do after a chunk: 0 the log has ended (nothing read, or only a
# fragment with no newline before the end), 1 lines were read (go on), 2 a
# line longer than the chunk (read again, twice as much)
def Stb.next.k(none: Bool, nothing: Bool, short: Bool) -> Nat:
  match none nothing short:
    case True{} _ _:
      0n
    case False{} False{} _:
      1n
    case False{} True{} True{}:
      0n
    case False{} True{} False{}:
      2n

def Stb.next(+used: Nat, +total: Nat, +max: U32) -> Nat:
  Stb.next.k(Nat.is_eq(total, 0n), Nat.is_eq(used, 0n), Nat.is_lt(total, U32.to_nat(max)))

def Stb.then(i: Nat, f: File, +h: String, +max: U32, ls: List<&2, Ln>, a: Acc, k: File -> Acc -> U32 -> IO(File & Acc)) -> IO(File & Acc):
  match i:
    case 0n:
      IO.pure(File & Acc, (f, a))
    case 1n:
      IO.bind(Acc, File & Acc, Stb.write(h, ls, a), a2 => k(f, a2, max))
    case _:
      k(f, a, U32.mul(max, 2))

def Stb.cut(f: File, +h: String, +max: U32, c: Cut, a: Acc, k: File -> Acc -> U32 -> IO(File & Acc)) -> IO(File & Acc):
  match c:
    case Cut{ls, +used, +total}:
      Stb.then(Stb.next(used, total, max), f, h, max, ls, a, k)

def Stb.read(r: File & Result<&1, &1, U32 & String, List<&2, U32>>, +h: String, +max: U32, a: Acc, k: File -> Acc -> U32 -> IO(File & Acc)) -> IO(File & Acc):
  (f, res) = r
  match res:
    case Done{bs}:
      Stb.cut(f, h, max, Cut.of(bs), a, k)
    case Fail{(_, e)}:
      IO.die(File & Acc, 1, "store build: read: " ++ e)

# chunks until the log ends (fuel: more than the chunks it can take)
def Stb.loop(fuel: Nat, f: File, +h: String, +max: U32, +a: Acc) -> IO(File & Acc):
  match fuel:
    case 0n:
      IO.pure(File & Acc, (f, a))
    case 1n+n:
      +off : Nat = Acc.off(a)
      IO.bind(File & Result<&1, &1, U32 & String, List<&2, U32>>, File & Acc, File.read_at(f, U32.from_nat(off), max),
        r => Stb.read(r, h, max, a, f2 => a2 => m2 => Stb.loop(n, f2, h, m2, a2)))

def Stb.file(r: Result<&1, &1, U32 & String, File>) -> IO(File):
  match r:
    case Done{f}:
      IO.pure(File, f)
    case Fail{(_, e)}:
      IO.die(File, 1, "store build: open the log: " ++ e)

def Stb.size(r: File & Result<&1, &1, U32 & String, U32>) -> IO(File & Nat):
  (f, res) = r
  match res:
    case Done{n}:
      IO.pure(File & Nat, (f, U32.to_nat(n)))
    case Fail{(_, e)}:
      IO.die(File & Nat, 1, "store build: log size: " ++ e)

def Stb.open.k(ok: Bool, +r: String) -> IO(String):
  match ok:
    case True{}:
      IO.pure(String, String.drop(r, 3n))
    case False{}:
      IO.die(String, 1, "store build: open the store: " ++ r)

def Stb.open(+r: String) -> IO(String):
  Stb.open.k(String.starts_with(r, "ok "), r)

# what the build did: lines numbered, the offset after the last, entries,
# threads counted
type Built is Data:
  Built{seq: Nat, off: Nat, ord: Nat}

# the log's id (SDB.Sdb.log_id of its first 4096 bytes, as the commit path
# checks it), then the chunks
def Stb.first(+size: Nat, +h: String, +chunk: U32, r: File & Result<&1, &1, U32 & String, List<&2, U32>>) -> IO(File & Acc):
  (f, res) = r
  Stb.loop(Nat.add(size, 64n), f, h, chunk, Acc.new(SDB.Sdb.log_id(Result.default(&1, &1, U32 & String, List<&2, U32>, res, Nil{}))))

def Stb.run(r: File & Nat, +h: String, +chunk: U32) -> IO(File & Acc):
  (f, +size) = r
  IO.bind(File & Result<&1, &1, U32 & String, List<&2, U32>>, File & Acc, File.read_at(f, 0, 4096), x => Stb.first(size, h, chunk, x))

def Stb.finish.k(+h: String, f: File, +a: Acc) -> IO(Built):
  do IO<Built>:
    File.close(f)
    +res : String <- D.Db.script(h, Stb.end(a))
    Stb.check("records", res)
    sy : String <- D.Db.exec(h, "PRAGMA synchronous = NORMAL")
    Stb.check("pragma", sy)
    cl : String <- D.Db.close(h)
    Stb.check("close", cl)
    return Built{Acc.seq(a), Acc.off(a), Acc.ord(a)}

def Stb.finish(+h: String, r: File & Acc) -> IO(Built):
  (f, a) = r
  Stb.finish.k(h, f, a)

# HOME/store.db made again from HOME/events.jsonl, read chunk bytes at a
# time (a line longer than that grows the read). The old store, if any, is
# deleted first.
def Stb.build(+home: String, +chunk: U32) -> IO(Built):
  +db : String = home ++ "/store.db"
  do IO<Built>:
    rm : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("rm", ["-f", db, db ++ "-wal", db ++ "-shm", db ++ "-journal"], "", 4096, 30000)
    o : String <- D.Db.open(db)
    +h : String <- Stb.open(o)
    sch : String <- D.Db.exec(h, SDB.Sdb.schema())
    Stb.check("schema", sch)
    sy : String <- D.Db.exec(h, "PRAGMA synchronous = OFF")
    Stb.check("pragma", sy)
    f0 : File <- IO.bind(Result<&1, &1, U32 & String, File>, File, File.open(home ++ "/events.jsonl", "r"), Stb.file)
    fs : File & Nat <- IO.bind(File & Result<&1, &1, U32 & String, U32>, File & Nat, File.size(f0), Stb.size)
    out : File & Acc <- Stb.run(fs, h, chunk)
    Stb.finish(h, out)
