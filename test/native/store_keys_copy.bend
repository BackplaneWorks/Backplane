import Base
import ../../src/core/json.bend as J
import ../../src/core/model.bend as M
import ../../src/core/codec.bend as C
import ../../src/core/storedb.bend as SDB
import ../../src/core/utf8.bend as U
import ../../src/server/argv.bend as A

# Store.keys coverage on a real log: "store == fold" over every line.
# HOME/events.jsonl (a COPY, never a live home) is read a chunk at a time
# and folded line by line as SDB.Mat.step does, the records each change
# names (SDB.Sdb.keys) written from the state after it into a records map
# (no facts). At the end that map is compared with SDB.Tab.dump of the
# final state: every key missing, extra or different is a key Sdb.keys
# does not name (or names wrongly). JS only:
#   bend test/native/store_keys_copy.bend HOME [CHUNK_BYTES]

# Lines of a chunk (as stobuild.bend's Cut.*)
# ---------------------------------------

type KLn is Data:
  KLn{len: Nat, text: String}

type KCut is Data:
  KCut{lines: List<&2, KLn>, used: Nat, total: Nat}

type KCutS is Data:
  KCutS{part: List<&2, U32>, plen: Nat, done: List<&2, KLn>, used: Nat, total: Nat}

def KCut.nl(nl: Bool, +x: U32, part: List<&2, U32>, +plen: Nat, done: List<&2, KLn>, +used: Nat, +total: Nat) -> KCutS:
  match nl:
    case True{}:
      KCutS{Nil{}, 0n, KLn{plen, U.Utf8.decode(List.reverse(&2, U32, part))} <> done, Nat.add(total, 1n), Nat.add(total, 1n)}
    case False{}:
      KCutS{x <> part, 1n+plen, done, used, Nat.add(total, 1n)}

def KCut.step(s: KCutS, +x: U32) -> KCutS:
  match s:
    case KCutS{part, +plen, done, +used, +total}:
      KCut.nl(U32.is_eq(x, 10), x, part, plen, done, used, total)

def KCut.fin(s: KCutS) -> KCut:
  match s:
    case KCutS{_, _, done, used, total}:
      KCut{List.reverse(&2, KLn, done), used, total}

def KCut.go(bs: List<&2, U32>, s: KCutS) -> KCut:
  match bs:
    case Nil{}:
      KCut.fin(s)
    case x <> t:
      KCut.go(t, KCut.step(s, x))

def KCut.of(bs: List<&2, U32>) -> KCut:
  KCut.go(bs, KCutS{Nil{}, 0n, Nil{}, 0n, 0n})

# The fold
# --------
# the state, the records map, the next line's offset, the lines folded

type KAcc is Data:
  KAcc{st: M.State, recs: Map<&2, String>, off: Nat, n: Nat}

def KAcc.off(a: KAcc) -> Nat:
  match a:
    case KAcc{_, _, o, _}:
      o

def K.recs(+pre: M.State, +post: M.State, +c: M.Change, recs: Map<&2, String>) -> Map<&2, String>:
  SDB.Tab.ops(SDB.Row.ofs(SDB.RK{"state", "seq"} <> SDB.Sdb.keys(pre, post, c), post, Nil{}), recs)

def K.fold.k(+pre: M.State, +post: M.State, +c: M.Change, recs: Map<&2, String>, +off: Nat, +n: Nat) -> KAcc:
  KAcc{post, K.recs(pre, post, c, recs), off, 1n+n}

def K.change(m: Maybe<&2, M.Change>, +st: M.State, recs: Map<&2, String>, +off: Nat, +n: Nat) -> KAcc:
  match m:
    case Some{+c}:
      K.fold.k(st, M.State.apply(st, c), c, recs, off, n)
    case None{}:
      KAcc{st, recs, off, n}

def K.parsed(m: Maybe<&2, J.Json>, st: M.State, recs: Map<&2, String>, +off: Nat, +n: Nat) -> KAcc:
  match m:
    case Some{+o}:
      K.change(C.Change.decode(o), st, recs, off, n)
    case None{}:
      KAcc{st, recs, off, n}

def K.line(a: KAcc, l: KLn) -> KAcc:
  match a l:
    case KAcc{st, recs, +off, +n} KLn{+len, +text}:
      K.parsed(J.Json.parse(text), st, recs, Nat.add(off, 1n+len), n)

def K.lines(ls: List<&2, KLn>, a: KAcc) -> KAcc:
  match ls:
    case Nil{}:
      a
    case l <> t:
      K.lines(t, K.line(a, l))

# The chunked read (as stobuild.bend's Stb.loop)
# ----------------------------------------------

def K.next.k(none: Bool, nothing: Bool, short: Bool) -> Nat:
  match none nothing short:
    case True{} _ _:
      0n
    case False{} False{} _:
      1n
    case False{} True{} True{}:
      0n
    case False{} True{} False{}:
      2n

def K.next(+used: Nat, +total: Nat, +max: U32) -> Nat:
  K.next.k(Nat.is_eq(total, 0n), Nat.is_eq(used, 0n), Nat.is_lt(total, U32.to_nat(max)))

def K.then(i: Nat, f: File, +max: U32, ls: List<&2, KLn>, a: KAcc, k: File -> KAcc -> U32 -> IO(File & KAcc)) -> IO(File & KAcc):
  match i:
    case 0n:
      IO.pure(File & KAcc, (f, a))
    case 1n:
      k(f, K.lines(ls, a), max)
    case _:
      k(f, a, U32.mul(max, 2))

def K.cut(f: File, +max: U32, c: KCut, a: KAcc, k: File -> KAcc -> U32 -> IO(File & KAcc)) -> IO(File & KAcc):
  match c:
    case KCut{ls, +used, +total}:
      K.then(K.next(used, total, max), f, max, ls, a, k)

def K.read(r: File & Result<&1, &1, U32 & String, List<&2, U32>>, +max: U32, a: KAcc, k: File -> KAcc -> U32 -> IO(File & KAcc)) -> IO(File & KAcc):
  (f, res) = r
  match res:
    case Done{bs}:
      K.cut(f, max, KCut.of(bs), a, k)
    case Fail{(_, e)}:
      IO.die(File & KAcc, 1, "store_keys_copy: read: " ++ e)

def K.loop(fuel: Nat, f: File, +max: U32, +a: KAcc) -> IO(File & KAcc):
  match fuel:
    case 0n:
      IO.pure(File & KAcc, (f, a))
    case 1n+n:
      +off : Nat = KAcc.off(a)
      IO.bind(File & Result<&1, &1, U32 & String, List<&2, U32>>, File & KAcc, File.read_at(f, U32.from_nat(off), max),
        r => K.read(r, max, a, f2 => a2 => m2 => K.loop(n, f2, m2, a2)))

def K.file(r: Result<&1, &1, U32 & String, File>) -> IO(File):
  match r:
    case Done{f}:
      IO.pure(File, f)
    case Fail{(_, e)}:
      IO.die(File, 1, "store_keys_copy: open the log: " ++ e)

def K.size(r: File & Result<&1, &1, U32 & String, U32>) -> IO(File & Nat):
  (f, res) = r
  match res:
    case Done{n}:
      IO.pure(File & Nat, (f, U32.to_nat(n)))
    case Fail{(_, e)}:
      IO.die(File & Nat, 1, "store_keys_copy: log size: " ++ e)

def K.run(r: File & Nat, +chunk: U32) -> IO(File & KAcc):
  (f, size) = r
  K.loop(Nat.add(size, 64n), f, chunk, KAcc{M.State.empty(), Map.new(&2, String), 0n, 0n})

# The comparison
# --------------
# counts and the first 50 keys of each kind (newest first while walking)

type KDiff is Data:
  KDiff{nm: Nat, ne: Nat, nd: Nat, ms: List<&2, String>, es: List<&2, String>, ds: List<&2, String>}

def K.most() -> Nat:
  50n

def K.show.key(+k: String) -> String:
  String.join(String.split(k, '\u{1f}'), " | ")

def K.cut200(+v: String) -> String:
  String.take(v, 200n)

def K.add(+n: Nat, +line: String, +xs: List<&2, String>) -> List<&2, String>:
  Bool.pick(List<&2, String>, Nat.is_lt(n, K.most()), line <> xs, xs)

def K.got(r: Map<&2, String> & String) -> String:
  (_, v) = r
  v

# a equals b, every step a tail call (Base's String.eq keeps a frame per
# character: a 35 MB setting faulted the comparison on the real log)
def K.same.go(a: String, b: String, ok: Bool) -> Bool:
  match a b:
    case SNil{} SNil{}:
      ok
    case SCon{Chr{x}, xt} SCon{Chr{y}, yt}:
      K.same.go(xt, yt, Bool.and(ok, U32.is_eq(x, y)))
    case _ _:
      False{}

def K.same(a: String, b: String) -> Bool:
  K.same.go(a, b, True{})

def K.absent() -> String:
  "\u{0}absent"

# one record of the full dump against the records map: missing or different
def K.dump.k(gone: Bool, same: Bool, +k: String, +want: String, +got: String, d: KDiff) -> KDiff:
  match gone same d:
    case True{} _ KDiff{+nm, ne, nd, ms, es, ds}:
      KDiff{1n+nm, ne, nd, K.add(nm, K.show.key(k) ++ "\n      dump: " ++ K.cut200(want), ms), es, ds}
    case False{} False{} KDiff{nm, ne, +nd, ms, es, ds}:
      KDiff{nm, ne, 1n+nd, ms, es, K.add(nd, K.show.key(k) ++ "\n      dump:  " ++ K.cut200(want) ++ "\n      store: " ++ K.cut200(got), ds)}
    case False{} True{} _:
      d

def K.dump.one(+k: String, +want: String, +got: String, d: KDiff) -> KDiff:
  K.dump.k(K.same(got, K.absent()), K.same(got, want), k, want, got, d)

def K.dump.go(xs: List<&2, Sigma<&2, &2, String, _ => String>>, +recs: Map<&2, String>, d: KDiff) -> KDiff:
  match xs:
    case Nil{}:
      d
    case (+k, +v) <> t:
      K.dump.go(t, recs, K.dump.one(k, v, K.got(Map.get(String, K.absent(), recs, k)), d))

# one record of the records map that the dump does not have: extra
def K.extra.k(extra: Bool, +k: String, +v: String, d: KDiff) -> KDiff:
  match extra d:
    case True{} KDiff{nm, +ne, nd, ms, es, ds}:
      KDiff{nm, 1n+ne, nd, ms, K.add(ne, K.show.key(k) ++ "\n      store: " ++ K.cut200(v), es), ds}
    case False{} _:
      d

def K.extra.go(xs: List<&2, Sigma<&2, &2, String, _ => String>>, +dump: Map<&2, String>, d: KDiff) -> KDiff:
  match xs:
    case Nil{}:
      d
    case (+k, +v) <> t:
      K.extra.go(t, dump, K.extra.k(K.same(K.got(Map.get(String, K.absent(), dump, k)), K.absent()), k, v, d))

def K.diff(+recs: Map<&2, String>, +dump: Map<&2, String>) -> KDiff:
  K.extra.go(Map.to_list(&2, String, recs), dump, K.dump.go(Map.to_list(&2, String, dump), recs, KDiff{0n, 0n, 0n, Nil{}, Nil{}, Nil{}}))

def K.lines.text(xs: List<&2, String>, acc: String) -> String:
  match xs:
    case Nil{}:
      acc
    case x <> t:
      K.lines.text(t, "  " ++ x ++ "\n" ++ acc)

def K.verdict(+d: KDiff) -> String:
  match d:
    case KDiff{+nm, +ne, +nd, _, _, _}:
      Bool.pick(String, Nat.is_eq(Nat.add(nm, Nat.add(ne, nd)), 0n), "ok store_keys_copy",
        "FAIL store_keys_copy missing=" ++ Nat.show(nm) ++ " extra=" ++ Nat.show(ne) ++ " different=" ++ Nat.show(nd))

def K.report(+d: KDiff) -> String:
  match d:
    case KDiff{+nm, +ne, +nd, ms, es, ds}:
      "missing (in the dump, not written incrementally): " ++ Nat.show(nm) ++ "\n" ++ K.lines.text(ms, "") ++
      "extra (written, not in the dump): " ++ Nat.show(ne) ++ "\n" ++ K.lines.text(es, "") ++
      "different: " ++ Nat.show(nd) ++ "\n" ++ K.lines.text(ds, "")

def K.finish(r: File & KAcc, +t0: Nat) -> IO(Unit):
  (f, a) = r
  match a:
    case KAcc{+st, +recs, +off, +n}:
      do IO<Unit>:
        File.close(f)
        +t1 : Nat <- IO.now()
        IO.print("folded " ++ Nat.show(n) ++ " changes, " ++ Nat.show(off) ++ " bytes, seq " ++ Nat.show(M.State.seq(st)) ++ " in " ++ Nat.show(Nat.sub(t1, t0)) ++ " ms")
        +dump : Map<&2, String> = SDB.Tab.dump(st)
        IO.print("records: store " ++ Nat.show(Map.size(&2, String, recs)) ++ ", dump " ++ Nat.show(Map.size(&2, String, dump)))
        +d : KDiff = K.diff(recs, dump)
        IO.print(K.report(d))
        +t2 : Nat <- IO.now()
        IO.print("compared in " ++ Nat.show(Nat.sub(t2, t1)) ++ " ms")
        IO.print(K.verdict(d))

def K.chunk(xs: List<String>) -> U32:
  match xs:
    case _ <> c <> _:
      U32.from_nat(Maybe.default(&2, Nat, Nat.read(c), 1048576n))
    case _:
      1048576

def K.main(+home: String, +chunk: U32) -> IO(Unit):
  do IO<Unit>:
    +t0 : Nat <- IO.now()
    f0 : File <- IO.bind(Result<&1, &1, U32 & String, File>, File, File.open(home ++ "/events.jsonl", "r"), K.file)
    fs : File & Nat <- IO.bind(File & Result<&1, &1, U32 & String, U32>, File & Nat, File.size(f0), K.size)
    out : File & KAcc <- K.run(fs, chunk)
    K.finish(out, t0)

def K.go(xs: List<String>) -> IO(Unit):
  match xs:
    case Nil{}:
      IO.die(Unit, 2, "usage: store_keys_copy HOME [CHUNK_BYTES]")
    case +home <> rest:
      K.main(home, K.chunk(home <> rest))

def main() -> IO(Unit):
  IO.bind(List<String>, Unit, A.Argv.args(), K.go)
