import Base
import ../core/json.bend as J
import ../core/model.bend as M
import ../core/codec.bend as C
import ../core/utf8.bend as U
import ../core/storedb.bend as SDB
import ./db.bend as D
import ./proc.bend as P
import ./perf.bend as PS

# The indexed store's IO (store.db; bp-coord/store/S1-SCHEMA.md)
# ===============================================================
# The hub holds no log lines. What clients see of the log is served from
# store.db's line(seq, off, len, wire) and events.jsonl read by offset:
# a run of lines is one read of the file from the first line's offset to
# the last line's end, cut at each line, its `wire` in place of the text
# when it has one (a phone's push registration without its token). Each
# commit writes its lines' facts and the records they touched in one
# Db.script (Sto.step, Sto.commit); a boot applies what the log holds past
# meta.applied_off (Sto.boot).

# The log as the hub keeps it: the store's handle ("" when there is none),
# the log's path, how many lines clients see (lines that parse: numbered
# from 1), the next entry's ordinal, and the oldest line as clients see it
# (the log's origin and the far history's identity are made from it)
type Lg is Data:
  Lg{db: String, path: String, n: Nat, ord: Nat, first: String}

def Lg.n(l: Lg) -> Nat:
  match l:
    case Lg{_, _, n, _, _}:
      n

def Lg.db(l: Lg) -> String:
  match l:
    case Lg{d, _, _, _, _}:
      d

def Lg.first(l: Lg) -> String:
  match l:
    case Lg{_, _, _, _, f}:
      f

def Lg.ord(l: Lg) -> Nat:
  match l:
    case Lg{_, _, _, o, _}:
      o

def Lg.empty(+path: String) -> Lg:
  Lg{"", path, 0n, 1n, ""}

# which log this is: the time of its oldest line ("0" for an empty log; a
# client resumes only into the log it came from)
def Lg.origin(+l: Lg) -> String:
  Nat.show(J.Json.get_nat(Maybe.default(&2, J.Json, J.Json.parse(Lg.first(l)), J.Null{}), "at", 0n))

def Sto.num(+s: String) -> Nat:
  M.Switch.num(s, 0n)

def Sto.path(+home: String) -> String:
  home ++ "/store.db"

# HOOK (F1): swap for FR.Fr.fact once backplane/far-live is on main. The
# state after the change; [] or one row ["#<seq>", scope]
def Sto.far.fact(+s: M.State, c: M.Change, +seq: Nat) -> List<&2, List<&2, String>>:
  Nil{}

def Sto.cpl(a: Bool, b: Bool, c: Bool) -> Nat:
  match a b c:
    case True{} _ _:
      1n
    case False{} True{} _:
      2n
    case False{} False{} True{}:
      3n
    case False{} False{} False{}:
      4n

# Bytes
# -----

# a String's UTF-8 length in bytes (as File.write puts it on disk)
def Sto.blen(s: String, +acc: Nat) -> Nat:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{+c}, t}:
      Sto.blen(t, Nat.add(acc, Sto.cpl(U32.is_lt(c, 128), U32.is_lt(c, 2048), U32.is_lt(c, 65536))))

def Sto.bytes(r: Result<&1, &1, U32 & String, List<&2, U32>>) -> List<&2, U32>:
  match r:
    case Done{bs}:
      bs
    case Fail{_}:
      Nil{}

def Sto.read.got(r: File & Result<&1, &1, U32 & String, List<&2, U32>>) -> IO(List<&2, U32>):
  (f, res) = r
  do IO<List<&2, U32>>:
    File.close(f)
    return Sto.bytes(res)

def Sto.read.k(r: Result<&1, &1, U32 & String, File>, +off: Nat, +size: Nat) -> IO(List<&2, U32>):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, List<&2, U32>>, List<&2, U32>, File.read_at(f, U32.from_nat(off), U32.from_nat(size)), Sto.read.got)
    case Fail{_}:
      IO.pure(List<&2, U32>, Nil{})

# size bytes of the file from off ([] when it cannot be read)
def Sto.read(+path: String, +off: Nat, +size: Nat) -> IO(List<&2, U32>):
  IO.bind(Result<&1, &1, U32 & String, File>, List<&2, U32>, File.open(path, "r"), r => Sto.read.k(r, off, size))

def Sto.size.of(r: Result<&1, &1, U32 & String, U32>) -> Nat:
  match r:
    case Done{n}:
      U32.to_nat(n)
    case Fail{_}:
      0n

def Sto.size.got(r: File & Result<&1, &1, U32 & String, U32>) -> IO(Nat):
  (f, res) = r
  do IO<Nat>:
    File.close(f)
    return Sto.size.of(res)

def Sto.size.k(r: Result<&1, &1, U32 & String, File>) -> IO(Nat):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, U32>, Nat, File.size(f), Sto.size.got)
    case Fail{_}:
      IO.pure(Nat, 0n)

def Sto.size(+path: String) -> IO(Nat):
  IO.bind(Result<&1, &1, U32 & String, File>, Nat, File.open(path, "r"), Sto.size.k)

# The log's identity: its first line (at most 4096 bytes of it) hashed, so
# a store made from another log is made again
# ----------------------------------------------------------------------

def Sto.log_id(+path: String) -> IO(String):
  IO.bind(List<&2, U32>, String, Sto.read(path, 0n, 4096n), bs => IO.pure(String, SDB.Sdb.log_id(bs)))

# Lines by number
# ---------------

type LRow is Data:
  LRow{off: Nat, len: Nat, wire: String}

def Sto.lrow(r: List<&2, String>) -> LRow:
  match r:
    case +o <> +l <> +w <> _:
      LRow{Sto.num(o), Sto.num(l), w}
    case _:
      LRow{0n, 0n, ""}

def Sto.lrows(rs: List<&2, List<&2, String>>, acc: List<&2, LRow>) -> List<&2, LRow>:
  match rs:
    case Nil{}:
      List.reverse(&2, LRow, acc)
    case r <> t:
      Sto.lrows(t, Sto.lrow(r) <> acc)

# an answer of Db.query that is its error row
def Sto.failed.r(r: List<&2, String>) -> Bool:
  match r:
    case +c <> _:
      String.starts_with(c, "\u{0}")
    case Nil{}:
      False{}

def Sto.failed(rs: List<&2, List<&2, String>>) -> Bool:
  match rs:
    case +r <> _:
      Sto.failed.r(r)
    case Nil{}:
      False{}

def Sto.why(rs: List<&2, List<&2, String>>) -> String:
  match rs:
    case (_ <> +w <> _) <> _:
      w
    case _:
      "?"

def Sto.wire(+w: String, raw: String) -> String:
  Bool.pick(String, String.is_empty(w), raw, w)

# the event log's whole lines from byte off within about max bytes, each
# as its byte length and its text: [len, text, ...] (effects/storedb.js)
def Sto.fx.lines(path: String, off: U32, max: U32) -> IO(List<&2, String>):
  import "./effects/storedb.js"

def Sto.texts.acc(hit: Bool, +w: String, t: String, acc: List<&2, String>) -> List<&2, String>:
  match hit:
    case True{}:
      Sto.wire(w, t) <> acc
    case False{}:
      acc

def Sto.texts.rows(hit: Bool, +r: LRow, rs: List<&2, LRow>) -> List<&2, LRow>:
  match hit:
    case True{}:
      rs
    case False{}:
      r <> rs

def LRow.off(r: LRow) -> Nat:
  match r:
    case LRow{o, _, _}:
      o

def LRow.wire(r: LRow) -> String:
  match r:
    case LRow{_, _, w}:
      w

# each row's line from the file's lines (ps: [len, text, ...] from pos),
# oldest first; a line with no row (a torn write's fragment) is passed
def Sto.texts(ps: List<&2, String>, rows: List<&2, LRow>, +pos: Nat, acc: List<&2, String>) -> List<&2, String>:
  match ps rows:
    case _ Nil{}:
      List.reverse(&2, String, acc)
    case +l <> +t <> +r +x <> +rs:
      Sto.texts(r, Sto.texts.rows(Nat.is_eq(LRow.off(x), pos), x, rs), Nat.add(pos, 1n+Sto.num(l)), Sto.texts.acc(Nat.is_eq(LRow.off(x), pos), LRow.wire(x), t, acc))
    case _ _:
      List.reverse(&2, String, acc)

def Sto.lastend(rows: List<&2, LRow>, +acc: Nat) -> Nat:
  match rows:
    case Nil{}:
      acc
    case LRow{+off, +len, _} <> t:
      Sto.lastend(t, Nat.add(off, len))

def Sto.lines.rows(rows: List<&2, LRow>, +path: String) -> IO(List<&2, String>):
  match rows:
    case Nil{}:
      IO.pure(List<&2, String>, Nil{})
    case LRow{+off, +len, +w} <> +t:
      do IO<List<&2, String>>:
        ps : List<&2, String> <- Sto.fx.lines(path, U32.from_nat(off), U32.from_nat(1n+Nat.sub(Sto.lastend(t, Nat.add(off, len)), off)))
        return Sto.texts(ps, LRow{off, len, w} <> t, off, Nil{})

def Sto.lines.k(bad: Bool, rs: List<&2, List<&2, String>>, +path: String) -> IO(List<&2, String>):
  match bad:
    case True{}:
      do IO<List<&2, String>>:
        PS.Log.warn("store", "lines: " ++ Sto.why(rs))
        return Nil{}
    case False{}:
      Sto.lines.rows(Sto.lrows(rs, Nil{}), path)

def Sto.lines.q(+lg: Lg, +from: Nat, +count: Nat) -> IO(List<&2, String>):
  match lg:
    case Lg{+db, +path, _, _, _}:
      do IO<List<&2, String>>:
        +rs : List<&2, List<&2, String>> <- D.Db.query.t(db, "SELECT off, len, wire FROM line WHERE seq > ? ORDER BY seq LIMIT ?", [D.Db.nat(from), D.Db.nat(count)], U32.from_nat(count))
        Sto.lines.k(Sto.failed(rs), rs, path)

def Sto.lines.go(none: Bool, +lg: Lg, +from: Nat, +count: Nat) -> IO(List<&2, String>):
  match none:
    case True{}:
      IO.pure(List<&2, String>, Nil{})
    case False{}:
      Sto.lines.q(lg, from, count)

# the lines clients see numbered from+1 .. from+count (fewer at the log's
# end), oldest first, each exactly as the log held it in memory before
# (C.Change.wire_line): byte for byte what a join sends
def Sto.lines(+lg: Lg, +from: Nat, +count: Nat) -> IO(List<&2, String>):
  Sto.lines.go(Bool.or(Nat.is_eq(count, 0n), String.is_empty(Lg.db(lg))), lg, from, count)

# Lines by a list of numbers (F1's far feed from far_line)
# --------------------------------------------------------

# consecutive numbers as one run: its first number and how many
type LRun is Data:
  LRun{from: Nat, n: Nat}

def Runs.close(+from: Nat, +n: Nat, +acc: List<&2, LRun>) -> List<&2, LRun>:
  Bool.pick(List<&2, LRun>, Nat.is_eq(n, 0n), acc, LRun{from, n} <> acc)

# the runs of consecutive numbers, in the order given (each step's values
# picked without recursion: the walk calls only itself)
def Runs.go(xs: List<&2, Nat>, +from: Nat, +n: Nat, +acc: List<&2, LRun>) -> List<&2, LRun>:
  match xs:
    case Nil{}:
      List.reverse(&2, LRun, Runs.close(from, n, acc))
    case +x <> t:
      +next = Bool.and(Nat.is_gt(n, 0n), Nat.is_eq(x, Nat.add(from, n)))
      Runs.go(t, Bool.pick(Nat, next, from, x), Bool.pick(Nat, next, 1n+n, 1n), Bool.pick(List<&2, LRun>, next, acc, Runs.close(from, n, acc)))

def Sto.runs(xs: List<&2, Nat>) -> List<&2, LRun>:
  Runs.go(xs, 0n, 0n, Nil{})

# xs pushed onto acc (reversed): a tail call per line
def Sto.onto(xs: List<&2, String>, acc: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      acc
    case x <> t:
      Sto.onto(t, x <> acc)

def Sto.lines.run(r: LRun, +lg: Lg, k: List<&2, String> -> IO(List<&2, String>)) -> IO(List<&2, String>):
  match r:
    case LRun{+from, +n}:
      IO.bind(List<&2, String>, List<&2, String>, Sto.lines(lg, Nat.sub(from, 1n), n), k)

def Sto.lines.at.go(rs: List<&2, LRun>, +lg: Lg, acc: List<&2, String>) -> IO(List<&2, String>):
  match rs:
    case Nil{}:
      IO.pure(List<&2, String>, List.reverse(&2, String, acc))
    case r <> t:
      Sto.lines.run(r, lg, xs => Sto.lines.at.go(t, lg, Sto.onto(xs, acc)))

# the lines numbered seqs (from 1), in the order given, each as Sto.lines
# gives it; one file read per run of consecutive numbers. A number past
# the log's end has no line (none is answered for it)
def Sto.lines.at(+lg: Lg, seqs: List<&2, Nat>) -> IO(List<&2, String>):
  Sto.lines.at.go(Sto.runs(seqs), lg, Nil{})

# Committing
# ----------
# Hub.fold steps each written change through Sto.step: the state after it
# (SDB.Sdb.step's, the fold's only M.State.apply), the line counted, and
# its statements gathered (newest first); Sto.commit writes them, with the
# meta, in one Db.script.

# a commit being written: the log, where the next line starts, the store's
# statements (newest first) and the log's identity when this commit starts
# the file ("" else)
type Cm is Data:
  Cm{lg: Lg, off: Nat, items: List<&2, List<&2, String>>, id: String}

def Cm.new(lg: Lg, +off: Nat) -> Cm:
  Cm{lg, off, Nil{}, ""}

def Cm.lg(c: Cm) -> Lg:
  match c:
    case Cm{l, _, _, _}:
      l

def Sto.push.ops(os: List<&2, SDB.Op>, acc: List<&2, List<&2, String>>) -> List<&2, List<&2, String>>:
  match os:
    case Nil{}:
      acc
    case o <> t:
      Sto.push.ops(t, SDB.Sql.of(o) <> acc)

def Sto.push.far(rs: List<&2, List<&2, String>>, acc: List<&2, List<&2, String>>) -> List<&2, List<&2, String>>:
  match rs:
    case Nil{}:
      acc
    case r <> t:
      Sto.push.far(t, ("INSERT INTO far_line(seq, scope) VALUES (CAST(ltrim(?, '#') AS INTEGER), ?) ON CONFLICT DO NOTHING" <> r) <> acc)

def Sto.first(+n: Nat, first: String, w: String) -> String:
  match n:
    case 0n:
      w
    case _:
      first

def Sto.idnew(+off: Nat, id: String, +disk: String) -> String:
  match off:
    case 0n:
      SDB.Sdb.log_id(U.Utf8.encode(disk))
    case _:
      id

def Sto.step.k(s: SDB.Step, +c: M.Change, +db: String, +path: String, +n: Nat, first: String, +off: Nat, items: List<&2, List<&2, String>>, id: String) -> M.State & Cm:
  match s:
    case SDB.Step{+post, fs, rs, ord}:
      (post, Cm{Lg{db, path, n, ord, first}, off, Sto.push.far(Sto.far.fact(post, c, n), Sto.push.ops(rs, Sto.push.ops(fs, items))), id})

# one written change: its disk line and what clients see of it (w)
def Sto.step(+st: M.State, +c: M.Change, +disk: String, w: String, cm: Cm) -> M.State & Cm:
  match cm:
    case Cm{Lg{+db, +path, +n, +ord, first}, +off, items, id}:
      +len = Sto.blen(disk, 0n)
      Sto.step.k(SDB.Sdb.step(st, SDB.At{1n+n, off, len, ord}, c), c, db, path, 1n+n, Sto.first(n, first, w), Nat.add(off, 1n+len), items, Sto.idnew(off, id, disk))

def Sto.meta(+k: String, +v: String) -> List<&2, String>:
  ["INSERT OR REPLACE INTO meta(k, v) VALUES (?, ?)", k, v]

def Sto.meta.id.k(none: Bool, +id: String, rest: List<&2, List<&2, String>>) -> List<&2, List<&2, String>>:
  match none:
    case True{}:
      rest
    case False{}:
      Sto.meta("log_id", id) <> rest

def Sto.meta.id(+id: String, rest: List<&2, List<&2, String>>) -> List<&2, List<&2, String>>:
  Sto.meta.id.k(String.is_empty(id), id, rest)

# where the store stands: the next line's number, offset and ordinal
def Sto.metas(+seq: Nat, +off: Nat, +ord: Nat, rest: List<&2, List<&2, String>>) -> List<&2, List<&2, String>>:
  Sto.meta("applied_seq", Nat.show(seq)) <> Sto.meta("applied_off", Nat.show(off)) <> Sto.meta("ord", Nat.show(ord)) <> rest

def Sto.script.k(+r: String) -> IO(Unit):
  Bool.pick(IO(Unit), String.starts_with(r, "ok"), IO.pure(Unit, Unit{}), PS.Log.warn("store", "write: " ++ r))

def Sto.script(+db: String, items: List<&2, List<&2, String>>) -> IO(Unit):
  IO.bind(String, Unit, D.Db.script(db, items), r => Sto.script.k(r))

def Sto.commit.k(none: Bool, +db: String, items: List<&2, List<&2, String>>) -> IO(Unit):
  match none:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      Sto.script(db, items)

# a commit's statements and the meta, in one transaction; the log as it
# stands after it
def Sto.commit(cm: Cm) -> IO(Lg):
  match cm:
    case Cm{Lg{+db, +path, +n, +ord, +first}, +off, +items, +id}:
      do IO<Lg>:
        Sto.commit.k(Bool.or(String.is_empty(db), List.is_empty(&2, List<&2, String>, items)), db, List.reverse(&2, List<&2, String>, Sto.meta.id(id, Sto.metas(n, off, ord, items))))
        return Lg{db, path, n, ord, first}

# Boot: the log past applied_off applied
# --------------------------------------
# Facts per line; the records every applied line names, read from the
# booted state, once per chunk. (Used for a store made afresh too, until
# the streaming rebuild, stobuild.bend's Stb.*, takes that over: a room
# post's or a mail's header is then judged against the booted state, so
# headers a later line made known are left out.)

type Raw is Data:
  Raw{len: Nat, text: String}

# [len, text, ...] as lines
def Sto.raws(ps: List<&2, String>, acc: List<&2, Raw>) -> List<&2, Raw>:
  match ps:
    case +l <> +t <> +r:
      Sto.raws(r, Raw{Sto.num(l), t} <> acc)
    case _:
      List.reverse(&2, Raw, acc)

type Tail is Data:
  Tail{seq: Nat, ord: Nat, off: Nat}

type Pr is Data:
  Pr{t: Tail, items: List<&2, List<&2, String>>, keys: Map<&2, SDB.RK>}

def Sto.keys.add(ks: List<&2, SDB.RK>, m: Map<&2, SDB.RK>) -> Map<&2, SDB.RK>:
  match ks:
    case Nil{}:
      m
    case SDB.RK{+d, +k} <> t:
      Sto.keys.add(t, Map.set(&2, SDB.RK, m, d ++ "\u{1f}" ++ k, SDB.RK{d, k}))

def Sto.wire.k(private: Bool, +o: J.Json, text: String) -> String:
  match private:
    case True{}:
      C.Change.wire_line(o, text)
    case False{}:
      ""

def Sto.line.c(m: Maybe<&2, M.Change>, +o: J.Json, text: String, +len: Nat, +st: M.State, p: Pr) -> Pr:
  match m p:
    case Some{+c} Pr{Tail{+s, +ord, +off}, its, ks}:
      Pr{Tail{1n+s, SDB.Sdb.ord(c, ord), Nat.add(off, 1n+len)}, Sto.push.far(Sto.far.fact(st, c, 1n+s), Sto.push.ops(SDB.Sdb.facts(st, SDB.At{1n+s, off, len, ord}, c), its)), Sto.keys.add(SDB.Sdb.keys(st, st, c), ks)}
    case None{} Pr{Tail{+s, +ord, +off}, its, ks}:
      Pr{Tail{1n+s, ord, Nat.add(off, 1n+len)}, Sto.push.ops([SDB.F.line(SDB.At{1n+s, off, len, ord}, Sto.wire.k(C.Change.private(J.Json.get_str(o, "$", "")), o, text))], its), ks}

def Sto.line.k(m: Maybe<&2, J.Json>, text: String, +len: Nat, +st: M.State, p: Pr) -> Pr:
  match m p:
    case None{} Pr{Tail{s, ord, +off}, its, ks}:
      Pr{Tail{s, ord, Nat.add(off, 1n+len)}, its, ks}
    case Some{+o} _:
      Sto.line.c(C.Change.decode(o), o, text, len, st, p)

def Sto.line(r: Raw, +st: M.State, p: Pr) -> Pr:
  match r:
    case Raw{+len, +text}:
      Sto.line.k(J.Json.parse(text), text, len, st, p)

def Sto.lines.apply(rs: List<&2, Raw>, +st: M.State, p: Pr) -> Pr:
  match rs:
    case Nil{}:
      p
    case r <> t:
      Sto.lines.apply(t, st, Sto.line(r, st, p))

# a chunk's statements: its lines' facts, the records they named, the meta
def Sto.chunk.items(p: Pr, +st: M.State) -> List<&2, List<&2, String>> & Tail:
  match p:
    case Pr{Tail{+s, +ord, +off}, its, ks}:
      (List.reverse(&2, List<&2, String>, Sto.metas(s, off, ord, Sto.push.ops(SDB.Row.ofs(SDB.RK{"state", "seq"} <> Map.values(&2, SDB.RK, ks), st, Nil{}), its))), Tail{s, ord, off})

def Sto.chunk.go(r: List<&2, List<&2, String>> & Tail, +db: String, k: Tail -> Nat -> IO(Tail)) -> IO(Tail):
  (items, t) = r
  do IO<Tail>:
    Sto.script(db, items)
    k(t, 1048576n)

def Tail.off(t: Tail) -> Nat:
  match t:
    case Tail{_, _, o}:
      o

def Sto.chunk.got(none: Bool, +ps: List<&2, String>, +db: String, +st: M.State, t: Tail, k: Tail -> Nat -> IO(Tail)) -> IO(Tail):
  match none:
    case True{}:
      IO.pure(Tail, t)
    case False{}:
      Sto.chunk.go(Sto.chunk.items(Sto.lines.apply(Sto.raws(ps, Nil{}), st, Pr{t, Nil{}, Map.new(&2, SDB.RK)}), st), db, k)

def Sto.tail.at(go: Bool, +db: String, +path: String, +st: M.State, +size: Nat, t: Tail, +chunk: Nat, k: Tail -> Nat -> IO(Tail)) -> IO(Tail):
  match go t:
    case False{} _:
      IO.pure(Tail, t)
    case True{} Tail{+s, +ord, +off}:
      do IO<Tail>:
        +ps : List<&2, String> <- Sto.fx.lines(path, U32.from_nat(off), U32.from_nat(chunk))
        Sto.chunk.got(List.is_empty(&2, String, ps), ps, db, st, Tail{s, ord, off}, k)

def Sto.tail(fuel: Nat, +db: String, +path: String, +st: M.State, +size: Nat, +t: Tail, +chunk: Nat) -> IO(Tail):
  match fuel:
    case 0n:
      IO.pure(Tail, t)
    case 1n+f:
      Sto.tail.at(Nat.is_lt(Tail.off(t), size), db, path, st, size, t, chunk, t2 => c2 => Sto.tail(f, db, path, st, size, t2, c2))

# Opening
# -------

def Sto.got(+k: String, rs: List<&2, List<&2, String>>) -> String:
  match rs:
    case Nil{}:
      ""
    case (+a <> +v <> _) <> t:
      Bool.pick(String, String.eq(a, k), v, Sto.got(k, t))
    case _ <> t:
      Sto.got(k, t)

def Sto.handle(+a: String) -> String:
  Bool.pick(String, String.starts_with(a, "ok "), String.drop(a, 3n), "")

def Sto.open.made(ok: Bool, +h: String, +e: String) -> IO(String):
  match ok:
    case True{}:
      IO.pure(String, h)
    case False{}:
      do IO<String>:
        PS.Log.warn("store", "schema: " ++ e)
        return ""

def Sto.open.schema(bad: Bool, +h: String, +a: String) -> IO(String):
  match bad:
    case True{}:
      do IO<String>:
        PS.Log.warn("store", "open: " ++ a)
        return ""
    case False{}:
      do IO<String>:
        +e : String <- D.Db.exec(h, SDB.Sdb.schema())
        Sto.open.made(String.starts_with(e, "ok"), h, e)

# the store opened with its tables made: its handle, or "" with the reason
# logged
def Sto.open.db(+dbp: String) -> IO(String):
  do IO<String>:
    +a : String <- D.Db.open(dbp)
    +h : String = Sto.handle(a)
    Sto.open.schema(String.is_empty(h), h, a)

# HOOK (stobuild.bend, Stb.*): a store made from another log, or with
# other rows (meta.codec), is made again. Until the streaming rebuild
# lands it is deleted and the boot applies the whole log to a new one.
def Sto.remake(+h: String, +dbp: String) -> IO(String):
  do IO<String>:
    PS.Log.info("store", "store.db is out of date: made again from the log")
    c : String <- D.Db.close(h)
    r : P.Ran <- P.Proc.run("rm", ["-f", dbp, dbp ++ "-wal", dbp ++ "-shm"], "/")
    Sto.open.db(dbp)

# applied: (seq, ord, off) from the meta, or a new store's (0, 1, 0)
def Sto.boot.from(fresh: Bool, +h: String, +rs: List<&2, List<&2, String>>, +id: String) -> IO(Tail):
  match fresh:
    case True{}:
      do IO<Tail>:
        Sto.script(h, [Sto.meta("codec", Nat.show(SDB.Sdb.codec())), Sto.meta("log_id", id), Sto.meta("schema", "1")])
        return Tail{0n, 1n, 0n}
    case False{}:
      IO.pure(Tail, Tail{Sto.num(Sto.got("applied_seq", rs)), Bool.pick(Nat, String.is_empty(Sto.got("ord", rs)), 1n, Sto.num(Sto.got("ord", rs))), Sto.num(Sto.got("applied_off", rs))})

def Sto.boot.check(good: Bool, +h: String, +dbp: String, +rs: List<&2, List<&2, String>>, +id: String) -> IO(String & Tail):
  match good:
    case True{}:
      IO.bind(Tail, String & Tail, Sto.boot.from(String.is_empty(Sto.got("codec", rs)), h, rs, id), t => IO.pure(String & Tail, (h, t)))
    case False{}:
      do IO<String & Tail>:
        +h2 : String <- Sto.remake(h, dbp)
        t : Tail <- Sto.boot.from(True{}, h2, Nil{}, id)
        return (h2, t)

# a store whose rows are this build's (meta.codec) and that was made from
# this log (meta.log_id, and the log not shorter than what it applied); a
# new store (no meta yet) is good
def Sto.good(+rs: List<&2, List<&2, String>>, +id: String, +size: Nat) -> Bool:
  Bool.or(String.is_empty(Sto.got("codec", rs)),
    Bool.and(Bool.and(String.eq(Sto.got("codec", rs), Nat.show(SDB.Sdb.codec())), Bool.or(String.eq(Sto.got("log_id", rs), id), String.is_empty(Sto.got("log_id", rs)))), Nat.is_le(Sto.num(Sto.got("applied_off", rs)), size)))

def Sto.boot.first(p: String & Tail, +path: String) -> IO(Lg):
  (+h, t) = p
  match t:
    case Tail{+s, +ord, _}:
      do IO<Lg>:
        fs : List<&2, String> <- Sto.lines(Lg{h, path, s, ord, ""}, 0n, 1n)
        return Lg{h, path, s, ord, Maybe.default(&2, String, List.head(&2, String, fs), "")}

def Sto.boot.applied(p: String & Tail, +path: String, +st: M.State, +size: Nat) -> IO(Lg):
  (+h, t) = p
  do IO<Lg>:
    t2 : Tail <- Sto.tail(1000000000n, h, path, st, size, t, 1048576n)
    Sto.boot.first((h, t2), path)

def Sto.boot.k(none: Bool, +h: String, +dbp: String, +path: String, +st: M.State) -> IO(Lg):
  match none:
    case True{}:
      IO.pure(Lg, Lg.empty(path))
    case False{}:
      do IO<Lg>:
        +rs : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT k, v FROM meta", Nil{}, 64)
        +id : String <- Sto.log_id(path)
        +size : Nat <- Sto.size(path)
        p : String & Tail <- Sto.boot.check(Sto.good(rs, id, size), h, dbp, rs, id)
        Sto.boot.applied(p, path, st, size)

# Opened after the log's replay (st: the booted state): the store brought
# up to the log's end, and the log as the hub keeps it
def Sto.boot(+home: String, +st: M.State) -> IO(Lg):
  do IO<Lg>:
    +h : String <- Sto.open.db(Sto.path(home))
    Sto.boot.k(String.is_empty(h), h, Sto.path(home), home ++ "/events.jsonl", st)

# Queries (S3, F1)
# ----------------
# Entry rows: ord, seq, id, thread, src, kind, role, akind, text, at (all
# as text); a thread's are those with thread = th or src = th
# (M.State.mine). before 0: no bound.

def Sto.ecols() -> String:
  "ord, seq, id, thread, src, kind, role, akind, text, at"

def Sto.q(+lg: Lg, +sql: String, ps: List<&2, D.DbVal>, +limit: Nat) -> IO(List<&2, List<&2, String>>):
  Bool.pick(IO(List<&2, List<&2, String>>), String.is_empty(Lg.db(lg)), IO.pure(List<&2, List<&2, String>>, Nil{}), D.Db.query.t(Lg.db(lg), sql, ps, U32.from_nat(limit)))

def Sto.n(+n: Nat) -> D.DbVal:
  D.Db.nat(n)

def Sto.t(+x: String) -> D.DbVal:
  D.DbText{x}

def Sto.ts(xs: List<&2, String>, acc: List<&2, D.DbVal>) -> List<&2, D.DbVal>:
  match xs:
    case Nil{}:
      List.reverse(&2, D.DbVal, acc)
    case x <> t:
      Sto.ts(t, D.DbText{x} <> acc)

# a thread's entries before ordinal `before`, newest first
def Sto.page(+lg: Lg, +thread: String, +before: Nat, +limit: Nat) -> IO(List<&2, List<&2, String>>):
  Sto.q(lg, "SELECT " ++ Sto.ecols() ++ " FROM entry WHERE (thread = ? OR src = ?) AND (? = 0 OR ord < ?) ORDER BY ord DESC LIMIT ?",
    [Sto.t(thread), Sto.t(thread), Sto.n(before), Sto.n(before), Sto.n(limit)], limit)

def Sto.marks(xs: List<&2, String>) -> String:
  match xs:
    case Nil{}:
      ""
    case _ <> Nil{}:
      "?"
    case _ <> t:
      "?, " ++ Sto.marks(t)

def Sto.search.k(none: Bool, +lg: Lg, +thread: String, +words: List<&2, String>, +before: Nat, +limit: Nat) -> IO(List<&2, List<&2, String>>):
  match none:
    case True{}:
      IO.pure(List<&2, List<&2, String>>, Nil{})
    case False{}:
      Sto.q(lg, "SELECT " ++ Sto.ecols() ++ " FROM entry WHERE ord IN (SELECT ord FROM term WHERE word IN (" ++ Sto.marks(words) ++ ") GROUP BY ord HAVING count(DISTINCT word) = ?)"
        ++ " AND (? = '' OR thread = ? OR src = ?) AND (? = 0 OR ord < ?) ORDER BY ord DESC LIMIT ?",
        List.append(&2, D.DbVal, Sto.ts(words, Nil{}), [Sto.n(List.length(&2, String, words)), Sto.t(thread), Sto.t(thread), Sto.t(thread), Sto.n(before), Sto.n(before), Sto.n(limit)]), limit)

# entries holding every word (THIN.Words.of's words: the caller makes them
# from the query the same way), in a thread ("" for all), newest first
def Sto.search(+lg: Lg, +thread: String, +words: List<&2, String>, +before: Nat, +limit: Nat) -> IO(List<&2, List<&2, String>>):
  Sto.search.k(List.is_empty(&2, String, words), lg, thread, words, before, limit)

# the entries with this id (a duplicate's too), oldest first
def Sto.entry(+lg: Lg, +id: String) -> IO(List<&2, List<&2, String>>):
  Sto.q(lg, "SELECT " ++ Sto.ecols() ++ " FROM entry WHERE id = ? ORDER BY seq LIMIT 64", [Sto.t(id)], 64n)

def Sto.count.of(rs: List<&2, List<&2, String>>) -> Nat:
  match rs:
    case (+c <> _) <> _:
      Sto.num(c)
    case _:
      0n

# how many entries a thread has
def Sto.count(+lg: Lg, +thread: String) -> IO(Nat):
  IO.bind(List<&2, List<&2, String>>, Nat, Sto.q(lg, "SELECT count(*) FROM entry WHERE thread = ? OR src = ?", [Sto.t(thread), Sto.t(thread)], 1n), rs => IO.pure(Nat, Sto.count.of(rs)))

def Sto.tcols() -> String:
  "id, project, title, created, activity, turn, pending, archived, pinned, settled_at, provider, model, session, deleted, json"

# thread rows (Sto.tcols), most recently active first
def Sto.thread_recent(+lg: Lg, +limit: Nat) -> IO(List<&2, List<&2, String>>):
  Sto.q(lg, "SELECT " ++ Sto.tcols() ++ " FROM thread WHERE deleted = 0 ORDER BY activity DESC LIMIT ?", [Sto.n(limit)], limit)

# a project's ("" for every project) settled, archived or deleted threads
# (`state`), active before `before` (0: no bound), most recent first.
# settled: settled_at set, neither archived nor deleted; archived: not
# deleted; deleted: deleted
def Sto.threads(+lg: Lg, +project: String, +state: String, +before: Nat, +limit: Nat) -> IO(List<&2, List<&2, String>>):
  Sto.q(lg, "SELECT " ++ Sto.tcols() ++ " FROM thread WHERE (? = '' OR project = ?)"
    ++ " AND ((? = 'settled' AND settled_at > 0 AND archived = 0 AND deleted = 0) OR (? = 'archived' AND archived = 1 AND deleted = 0) OR (? = 'deleted' AND deleted = 1))"
    ++ " AND (? = 0 OR activity < ?) ORDER BY activity DESC LIMIT ?",
    [Sto.t(project), Sto.t(project), Sto.t(state), Sto.t(state), Sto.t(state), Sto.n(before), Sto.n(before), Sto.n(limit)], limit)
