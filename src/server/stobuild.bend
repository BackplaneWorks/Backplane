import Base
import ../core/json.bend as J
import ../core/model.bend as M
import ../core/codec.bend as C
import ../core/storedb.bend as SDB
import ../core/utf8.bend as U
import ./db.bend as D
import ./host.bend as H
import ../core/far.bend as FR

# The store's first build (store.db from events.jsonl, docs:
# bp-coord/store/S1-SCHEMA.md)
# ---------------------------------------------------------------
# The log is read a chunk at a time (File.read_at), never whole: a chunk's
# complete lines are folded and their facts written in one Db.script; the
# partial last line is read again with the next chunk, and a line longer
# than a chunk makes the chunk grow until it fits. The fold keeps no
# entries (Stb.strip after every chunk) or cold settings (written straight
# to SQL, never inserted in the fold): the one step that reads entries,
# ProviderSet's Switch.count, is given a counter of each thread's own
# entries instead (M.State.switch.c). After the last line every record of
# the final state is written (SDB.Dump.part) with the meta rows; the
# counters are not (nothing would keep them current on commit). Numbering
# is SDB.Mat.step's: a line that is not JSON takes its bytes but no number, a line no change
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

# Cold settings are opaque run-input copies: no model transition reads
# them. Keep their rows in SQL, and preserve the fold's tick without ever
# inserting their values into M.State. Empty cold values are records too
# (only told.*, rule.*, board.* use deletion semantics).
def Stb.setting(cold: Bool, st: M.State, k: String, v: String) -> M.State:
  match cold:
    case True{}:
      M.State.tick(st)
    case False{}:
      M.State.tick(M.State.setting(st, k, v))

# one change folded: M.State.apply, but cold settings bypass the fold and
# a ProviderSet takes the thread's count from the counters. WorkSet also
# writes a setting, so it must use the same cold-key decision.
def Stb.apply(+st: M.State, +cnt: Map<&2, Nat>, +c: M.Change) -> M.State:
  match c:
    case M.ProviderSet{+id, +p, m, _}:
      M.State.tick(M.State.switch.c(st, id, p, m, Stb.count(cnt, id)))
    case M.SettingSet{+k, v, _}:
      Stb.setting(SDB.Sdb.cold(k), st, k, v)
    case M.WorkSet{+th, +kind, rows, _}:
      +k = kind ++ "." ++ th
      Stb.setting(SDB.Sdb.cold(k), st, k, rows)
    case other:
      M.State.apply(st, other)

# The cold setting row named by this event, if any. SDB.Row.set uses the
# same raw text and cold flag as the final fold dump, including #digits.
def Stb.setting.ops(cold: Bool, k: String, v: String) -> List<&2, SDB.Op>:
  match cold:
    case True{}:
      [SDB.Row.set(k, v)]
    case False{}:
      Nil{}

def Stb.cold(c: M.Change) -> List<&2, SDB.Op>:
  match c:
    case M.SettingSet{+k, v, _}:
      Stb.setting.ops(SDB.Sdb.cold(k), k, v)
    case M.WorkSet{+th, +kind, rows, _}:
      +k = kind ++ "." ++ th
      Stb.setting.ops(SDB.Sdb.cold(k), k, rows)
    case _:
      Nil{}

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

# a change's far_line rows (FR.Fr.fact over the state after it: what the
# commit path writes, storedb.bend's Sto.far.fact), as statements
def Stb.far(fs: List<&2, FR.Fact>, acc: List<&2, List<&2, String>>) -> List<&2, List<&2, String>>:
  match fs:
    case Nil{}:
      acc
    case FR.Fact{+seq, +scope} <> t:
      Stb.far(t, ["INSERT INTO far_line(seq, scope) VALUES (CAST(? AS INTEGER), ?) ON CONFLICT DO NOTHING", Nat.show(seq), scope] <> acc)

# the log's origin: the "at" of its first line that parses (Hub.origin)
def Stb.origin(+seq: Nat, +o: J.Json, origin: String) -> String:
  match seq:
    case 0n:
      Nat.show(J.Json.get_nat(o, "at", 0n))
    case _:
      origin

def Stb.change(m: Maybe<&2, M.Change>, +st: M.State, +cnt: Map<&2, Nat>, +seq: Nat, +off: Nat, +len: Nat, +ord: Nat, items: List<&2, List<&2, String>>, id: String, origin: String, w: String) -> Acc:
  match m:
    case Some{+c}:
      +p = SDB.F.posts(c)
      +cnt2 = Stb.bump(p, cnt)
      +post = Stb.apply(st, cnt, c)
      Acc{post, cnt2, seq, Nat.add(off, 1n+len), SDB.Sdb.ord(c, ord), Stb.far(FR.Fr.fact(post, c, seq), Stb.sqls(Stb.cold(c), Stb.sqls(SDB.Sdb.facts(st, SDB.At{seq, off, len, ord}, c), items))), id, origin}
    case None{}:
      Acc{st, cnt, seq, Nat.add(off, 1n+len), ord, Stb.sqls([SDB.F.line(SDB.At{seq, off, len, ord}, w)], items), id, origin}

# Flat strings
# ------------
# In the JS build a String grown a char at a time (the JSON lexer's
# accumulator, then String.reverse) is a rope: one JS string cell per
# character until something reads it. A setting's value, a thread's title,
# a mail header are kept in the state and never read here, so every one
# stayed a rope: 28 million string cells (1 GB of heap, 3 GB of RSS) by the
# log's 160th MB, OOM-killed under 3 GB. Matching a String reads its first
# character, which makes JSC flatten the rope in place; so each line's
# strings are read once before the change is decoded from them.

# a String read once (flattened), the same text
def Stb.str(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{c, t}:
      SCon{c, t}

def Stb.flat(j: J.Json) -> J.Json:
  match j:
    case J.Str{t}:
      J.Str{Stb.str(t)}
    case J.Arr{xs}:
      J.Arr{Stb.flat(xs)}
    case J.Obj{fs}:
      J.Obj{Stb.flat(fs)}
    case J.Item{h, t}:
      J.Item{Stb.flat(h), Stb.flat(t)}
    case J.Field{k, v, t}:
      J.Field{Stb.str(k), Stb.flat(v), Stb.flat(t)}
    case other:
      other

def Stb.parsed(m: Maybe<&2, J.Json>, st: M.State, cnt: Map<&2, Nat>, +seq: Nat, +off: Nat, +len: Nat, ord: Nat, items: List<&2, List<&2, String>>, id: String, origin: String) -> Acc:
  match m:
    case Some{+o}:
      Stb.change(C.Change.decode(Stb.flat(o)), st, cnt, 1n+seq, off, len, ord, items, id, Stb.origin(seq, o, origin), C.Change.wire_over(o, ""))
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

# The end: every record and meta
# --------------------------------------------

def Stb.meta.sql(+k: String, v: String) -> List<&2, String>:
  ["INSERT OR REPLACE INTO meta(k, v) VALUES (?, ?)", k, v]

# the meta, written last (after every record): a store with a codec in its
# meta is a whole one; far_line is filled (Fr.fact per line above)
def Stb.end(a: Acc) -> List<&2, List<&2, String>>:
  match a:
    case Acc{_, _, +seq, +off, +ord, _, id, origin}:
      [Stb.meta.sql("schema", "1"),
       Stb.meta.sql("applied_seq", Nat.show(seq)),
       Stb.meta.sql("applied_off", Nat.show(off)),
       Stb.meta.sql("ord", Nat.show(ord)),
       Stb.meta.sql("log_id", id),
       Stb.meta.sql("origin", origin),
       Stb.meta.sql("far_line", "filled"),
       Stb.meta.sql("codec", Nat.show(SDB.Sdb.codec()))]

def Acc.seq(a: Acc) -> Nat:
  match a:
    case Acc{_, _, seq, _, _, _, _, _}:
      seq

def Acc.ord(a: Acc) -> Nat:
  match a:
    case Acc{_, _, _, _, ord, _, _, _}:
      ord

def Acc.cnt(a: Acc) -> Map<&2, Nat>:
  match a:
    case Acc{_, cnt, _, _, _, _, _, _}:
      cnt

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

# Progress
# --------
# The hub that waits for a build reads HOME/store.build ("progress <lines>
# <bytes done> <log bytes>", replaced whole after each chunk) for its note;
# the same line goes to stdout.

def Stb.put.done(r: File & Result<&1, &1, U32 & String, Unit>) -> IO(Unit):
  (f, _) = r
  File.close(f)

def Stb.put.k(r: Result<&1, &1, U32 & String, File>, +text: String) -> IO(Unit):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, Unit>, Unit, File.write(f, text), Stb.put.done)
    case Fail{_}:
      IO.pure(Unit, Unit{})

# a file's text replaced whole (written beside it, then renamed over it)
def Stb.put(+path: String, +text: String) -> IO(Unit):
  do IO<Unit>:
    IO.bind(Result<&1, &1, U32 & String, File>, Unit, File.open(path ++ ".tmp", "w"), r => Stb.put.k(r, text))
    mv : Result<&1, &1, U32 & String, Unit> <- H.FS.rename(path ++ ".tmp", path)
    return Unit{}

def Stb.progress(+home: String, +seq: Nat, +off: Nat, +total: Nat) -> IO(Unit):
  +line = "progress " ++ Nat.show(seq) ++ " " ++ Nat.show(off) ++ " " ++ Nat.show(total)
  do IO<Unit>:
    IO.print("store-rebuild " ++ line)
    Stb.put(home ++ "/store.build", line ++ "\n")

def Stb.write.k(+h: String, +home: String, +total: Nat, r: List<&2, List<&2, String>> & Acc) -> IO(Acc):
  (items, +a) = r
  do IO<Acc>:
    +res : String <- D.Db.script(h, items)
    Stb.check("chunk ending at " ++ Nat.show(Acc.off(a)), res)
    Stb.progress(home, Acc.seq(a), Acc.off(a), total)
    return a

# a chunk's lines folded and written in one transaction; answers the
# accumulator after them, entries stripped
def Stb.write(+h: String, +home: String, +total: Nat, ls: List<&2, Ln>, a: Acc) -> IO(Acc):
  Stb.write.k(h, home, total, Acc.take(Stb.lines(ls, a)))

# what to do after a chunk: 0 the log has ended (nothing read, or only a
# fragment with no newline before the end), 1 lines were read (go on), 2 a
# line longer than the chunk (read again, twice as much); a successful
# read resets to the configured chunk so a long line cannot raise all
# subsequent chunk allocations
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

def Stb.then(i: Nat, f: File, +h: String, +home: String, +logsize: Nat, +chunk: U32, +max: U32, ls: List<&2, Ln>, a: Acc, k: File -> Acc -> U32 -> IO(File & Acc)) -> IO(File & Acc):
  match i:
    case 0n:
      IO.pure(File & Acc, (f, a))
    case 1n:
      IO.bind(Acc, File & Acc, Stb.write(h, home, logsize, ls, a), a2 => k(f, a2, chunk))
    case _:
      k(f, a, U32.mul(max, 2))

def Stb.cut(f: File, +h: String, +home: String, +logsize: Nat, +chunk: U32, +max: U32, c: Cut, a: Acc, k: File -> Acc -> U32 -> IO(File & Acc)) -> IO(File & Acc):
  match c:
    case Cut{ls, +used, +total}:
      Stb.then(Stb.next(used, total, max), f, h, home, logsize, chunk, max, ls, a, k)

def Stb.read(r: File & Result<&1, &1, U32 & String, List<&2, U32>>, +h: String, +home: String, +logsize: Nat, +chunk: U32, +max: U32, a: Acc, k: File -> Acc -> U32 -> IO(File & Acc)) -> IO(File & Acc):
  (f, res) = r
  match res:
    case Done{bs}:
      Stb.cut(f, h, home, logsize, chunk, max, Cut.of(bs), a, k)
    case Fail{(_, e)}:
      IO.die(File & Acc, 1, "store build: read: " ++ e)

# chunks until the log ends (fuel: more than the chunks it can take)
def Stb.loop(fuel: Nat, f: File, +h: String, +home: String, +logsize: Nat, +chunk: U32, +max: U32, +a: Acc) -> IO(File & Acc):
  match fuel:
    case 0n:
      IO.pure(File & Acc, (f, a))
    case 1n+n:
      +off : Nat = Acc.off(a)
      IO.bind(File & Result<&1, &1, U32 & String, List<&2, U32>>, File & Acc, File.read_at(f, U32.from_nat(off), max),
        r => Stb.read(r, h, home, logsize, chunk, max, a, f2 => a2 => m2 => Stb.loop(n, f2, h, home, logsize, chunk, m2, a2)))

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
def Stb.first(+size: Nat, +h: String, +home: String, +chunk: U32, r: File & Result<&1, &1, U32 & String, List<&2, U32>>) -> IO(File & Acc):
  (f, res) = r
  Stb.loop(Nat.add(size, 64n), f, h, home, size, chunk, chunk, Acc.new(SDB.Sdb.log_id(Result.default(&1, &1, U32 & String, List<&2, U32>, res, Nil{}))))

def Stb.run(r: File & Nat, +h: String, +home: String, +chunk: U32) -> IO(File & Acc):
  (f, +size) = r
  do IO<File & Acc>:
    IO.print("store-rebuild start " ++ Nat.show(size))
    IO.bind(File & Result<&1, &1, U32 & String, List<&2, U32>>, File & Acc, File.read_at(f, 0, 4096), x => Stb.first(size, h, home, chunk, x))

# every record of the state, a domain at a time (SDB.Dump.part): one
# domain's statements at most in memory, never all of them
def Stb.parts(k: Nat, +h: String, +st: M.State, +i: Nat) -> IO(Unit):
  match k:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+r:
      do IO<Unit>:
        +res : String <- D.Db.script(h, List.reverse(&2, List<&2, String>, Stb.sqls(SDB.Dump.part(st, i), Nil{})))
        Stb.check("records", res)
        Stb.parts(r, h, st, 1n+i)

def Stb.gone(+path: String) -> IO(Unit):
  do IO<Unit>:
    r : Result<&1, &1, U32 & String, Unit> <- H.FS.remove(path)
    return Unit{}

def Stb.moved(r: Result<&1, &1, U32 & String, Unit>) -> IO(Unit):
  match r:
    case Done{_}:
      IO.pure(Unit, Unit{})
    case Fail{(_, e)}:
      IO.die(Unit, 1, "store build: rename store.db.tmp: " ++ e)

def Stb.finish.k(+h: String, +home: String, f: File, +a: Acc) -> IO(Built):
  +db = home ++ "/store.db"
  do IO<Built>:
    File.close(f)
    Stb.parts(SDB.Dump.n(), h, Acc.st(a), 0n)
    +res : String <- D.Db.script(h, Stb.end(a))
    Stb.check("meta", res)
    jm : String <- D.Db.exec(h, "PRAGMA journal_mode = DELETE")
    Stb.check("journal", jm)
    cl : String <- D.Db.close(h)
    Stb.check("close", cl)
    Stb.gone(db ++ "-wal")
    Stb.gone(db ++ "-shm")
    Stb.gone(db)
    mv : Result<&1, &1, U32 & String, Unit> <- H.FS.rename(db ++ ".tmp", db)
    Stb.moved(mv)
    Stb.gone(home ++ "/store.build")
    return Built{Acc.seq(a), Acc.off(a), Acc.ord(a)}

def Stb.finish(+h: String, +home: String, r: File & Acc) -> IO(Built):
  (f, a) = r
  Stb.finish.k(h, home, f, a)

# HOME/store.db made again from HOME/events.jsonl, read chunk bytes at a
# time (a line longer than that grows the read), built in store.db.tmp and
# renamed over store.db only once whole: an interrupted build leaves no
# store.db of its own (the old one, if any, stays until the rename)
def Stb.build(+home: String, +chunk: U32) -> IO(Built):
  +db : String = home ++ "/store.db.tmp"
  do IO<Built>:
    rm : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("rm", ["-f", db, db ++ "-wal", db ++ "-shm", db ++ "-journal"], "", 4096, 30000)
    o : String <- D.Db.open(db)
    +h : String <- Stb.open(o)
    # Bound SQLite's page cache to 2 MiB; sort/index scratch goes to disk.
    cfg : String <- D.Db.exec(h, "PRAGMA cache_size = -2048; PRAGMA temp_store = FILE")
    Stb.check("cache and temp storage", cfg)
    sch : String <- D.Db.exec(h, SDB.Sdb.schema())
    Stb.check("schema", sch)
    sy : String <- D.Db.exec(h, "PRAGMA synchronous = OFF")
    Stb.check("pragma", sy)
    f0 : File <- IO.bind(Result<&1, &1, U32 & String, File>, File, File.open(home ++ "/events.jsonl", "r"), Stb.file)
    fs : File & Nat <- IO.bind(File & Result<&1, &1, U32 & String, U32>, File & Nat, File.size(f0), Stb.size)
    out : File & Acc <- Stb.run(fs, h, home, chunk)
    Stb.finish(h, home, out)

# The child: backplane-serve --store-rebuild HOME (src/core/cli.bend)
# --------------------------------------------------------------------

def Stb.args.copy(xs: List<String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      x <> Stb.args.copy(t)

def Stb.args.back(xs: List<&2, String>) -> List<String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      x <> Stb.args.back(t)

def Stb.done(b: Built, +t0: Nat, +t1: Nat) -> IO(Unit):
  match b:
    case Built{seq, off, ord}:
      IO.print("store built lines=" ++ Nat.show(seq) ++ " applied_off=" ++ Nat.show(off) ++ " entries=" ++ Nat.show(Nat.sub(ord, 1n)) ++ " ms=" ++ Nat.show(Nat.sub(t1, t0)))

def Stb.child(+home: String) -> IO(Unit):
  do IO<Unit>:
    t0 : Nat <- IO.now()
    b : Built <- Stb.build(home, 262144)
    t1 : Nat <- IO.now()
    Stb.done(b, t0, t1)

def Stb.main.k(xs: List<&2, String>, go: List<String> -> IO(Unit)) -> IO(Unit):
  match xs:
    case "--store-rebuild" <> +home <> _:
      Stb.child(home)
    case +a:
      go(Stb.args.back(a))

# the arguments, if they ask for a store build (else go on with them)
def Stb.main(xs: List<String>, go: List<String> -> IO(Unit)) -> IO(Unit):
  Stb.main.k(Stb.args.copy(xs), go)
