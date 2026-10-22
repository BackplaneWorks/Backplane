import Base
import ../../src/core/json.bend as J
import ../../src/core/model.bend as M
import ../../src/core/codec.bend as C
import ../../src/core/far.bend as FR
import ../../src/core/utf8.bend as U
import ../../src/server/argv.bend as A

# far_line's back-fill on a real log: FR.Fr.fact with the state right
# after a line (what the commit path writes) against FR.Fr.fact with the
# final state (what a back-fill over an existing log writes, and what the
# far feed has always judged by). HOME/events.jsonl (a COPY, never a live
# home) is read twice a chunk at a time: first folded, each line's fact
# kept (number -> scope, shared lines only); then read again against the
# final state. Lines are numbered as the store numbers them (lines that
# parse, from 1). Prints how many lines differ and the first 50 (number,
# change, id, then and now), then "ok far_fact_copy" or "FAIL ...". JS only:
#   bend test/native/far_fact_copy.bend HOME [CHUNK_BYTES]

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

# The passes
# ----------
# the state (folded in pass 1, the final one in pass 2), the facts of pass
# 1 (number -> scope), the next line's offset, the lines that parse, the
# pass, the differences counted and the first of them, the shared lines

type KAcc is Data:
  KAcc{st: M.State, at: Map<&2, String>, off: Nat, n: Nat, two: Bool, nd: Nat, ds: List<&2, String>, ns: Nat}

def KAcc.off(a: KAcc) -> Nat:
  match a:
    case KAcc{_, _, o, _, _, _, _, _}:
      o

def F.most() -> Nat:
  50n

def F.scope(fs: List<&2, FR.Fact>) -> String:
  match fs:
    case FR.Fact{_, sc} <> _:
      "1" ++ sc
    case Nil{}:
      "0"

def F.add.k(room: Bool, +line: String, ds: List<&2, String>) -> List<&2, String>:
  match room:
    case True{}:
      line <> ds
    case False{}:
      ds

def F.add(+nd: Nat, +line: String, ds: List<&2, String>) -> List<&2, String>:
  F.add.k(Nat.is_lt(nd, F.most()), line, ds)

def F.got(r: Map<&2, String> & String) -> String:
  (_, v) = r
  v

def F.tag(+c: M.Change) -> String:
  J.Json.get_str(C.Change.encode(c), "$", "?")

def F.id(+c: M.Change) -> String:
  J.Json.get_str(C.Change.encode(c), "id", J.Json.get_str(C.Change.encode(c), "thread", J.Json.get_str(C.Change.encode(c), "task", J.Json.get_str(C.Change.encode(c), "bot", ""))))

def F.show(+v: String) -> String:
  Bool.pick(String, String.eq(v, "0"), "not shared", "shared, scope \"" ++ String.drop(v, 1n) ++ "\"")

def F.keep(shared: Bool, at: Map<&2, String>, +n: Nat, +v: String) -> Map<&2, String>:
  match shared:
    case True{}:
      Map.set(&2, String, at, Nat.show(n), v)
    case False{}:
      at

def F.two.k(same: Bool, +then: String, +now: String, +c: M.Change, a: KAcc) -> KAcc:
  match same a:
    case True{} x:
      x
    case False{} KAcc{st, at, off, +n, two, +nd, ds, ns}:
      KAcc{st, at, off, n, two, 1n+nd, F.add(nd, Nat.show(n) ++ " " ++ F.tag(c) ++ " " ++ F.id(c) ++ ": then " ++ F.show(then) ++ ", now " ++ F.show(now), ds), ns}

def F.shared(+v: String, +ns: Nat) -> Nat:
  Bool.pick(Nat, String.starts_with(v, "1"), 1n+ns, ns)

# pass 1: fold, keep the fact from the state after the line
def F.one(+st: M.State, +c: M.Change, at: Map<&2, String>, +off: Nat, +n: Nat, nd: Nat, ds: List<&2, String>, +ns: Nat) -> KAcc:
  +post = M.State.apply(st, c)
  +v = F.scope(FR.Fr.fact(post, c, n))
  KAcc{post, F.keep(Bool.not(String.eq(v, "0")), at, n, v), off, n, False{}, nd, ds, F.shared(v, ns)}

# pass 2: the fact from the final state, against pass 1's
def F.two(+st: M.State, +c: M.Change, +at: Map<&2, String>, +off: Nat, +n: Nat, +nd: Nat, ds: List<&2, String>, +ns: Nat) -> KAcc:
  +now = F.scope(FR.Fr.fact(st, c, n))
  +then = F.got(Map.get(String, "0", at, Nat.show(n)))
  F.two.k(String.eq(then, now), then, now, c, KAcc{st, at, off, n, True{}, nd, ds, F.shared(now, ns)})

def F.pass(two: Bool, +st: M.State, +c: M.Change, at: Map<&2, String>, +off: Nat, +n: Nat, +nd: Nat, ds: List<&2, String>, +ns: Nat) -> KAcc:
  match two:
    case False{}:
      F.one(st, c, at, off, n, nd, ds, ns)
    case True{}:
      F.two(st, c, at, off, n, nd, ds, ns)

def K.change(m: Maybe<&2, M.Change>, a: KAcc) -> KAcc:
  match m a:
    case Some{+c} KAcc{+st, at, +off, +n, two, +nd, ds, +ns}:
      F.pass(two, st, c, at, off, n, nd, ds, ns)
    case None{} x:
      x

def K.counted(a: KAcc, +off: Nat) -> KAcc:
  match a:
    case KAcc{st, at, _, +n, two, nd, ds, ns}:
      KAcc{st, at, off, 1n+n, two, nd, ds, ns}

def K.moved(a: KAcc, +off: Nat) -> KAcc:
  match a:
    case KAcc{st, at, _, n, two, nd, ds, ns}:
      KAcc{st, at, off, n, two, nd, ds, ns}

def K.parsed(m: Maybe<&2, J.Json>, a: KAcc, +off: Nat) -> KAcc:
  match m:
    case Some{+o}:
      K.change(C.Change.decode(o), K.counted(a, off))
    case None{}:
      K.moved(a, off)

def K.line(+a: KAcc, l: KLn) -> KAcc:
  match l:
    case KLn{+len, +text}:
      K.parsed(J.Json.parse(text), a, Nat.add(KAcc.off(a), 1n+len))

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
      IO.die(File & KAcc, 1, "far_fact_copy: read: " ++ e)

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
      IO.die(File, 1, "far_fact_copy: open the log: " ++ e)

def K.size(r: File & Result<&1, &1, U32 & String, U32>) -> IO(File & Nat):
  (f, res) = r
  match res:
    case Done{n}:
      IO.pure(File & Nat, (f, U32.to_nat(n)))
    case Fail{(_, e)}:
      IO.die(File & Nat, 1, "far_fact_copy: log size: " ++ e)

def K.run(r: File & Nat, +chunk: U32, a: KAcc) -> IO(File & KAcc):
  (f, size) = r
  K.loop(Nat.add(size, 64n), f, chunk, a)

def K.open(+home: String) -> IO(File & Nat):
  IO.bind(Result<&1, &1, U32 & String, File>, File & Nat, File.open(home ++ "/events.jsonl", "r"), r =>
    IO.bind(File, File & Nat, K.file(r), f => IO.bind(File & Result<&1, &1, U32 & String, U32>, File & Nat, File.size(f), K.size)))

def K.again(a: KAcc) -> KAcc:
  match a:
    case KAcc{st, at, _, _, _, _, _, _}:
      KAcc{st, at, 0n, 0n, True{}, 0n, Nil{}, 0n}

def K.text(xs: List<&2, String>, acc: String) -> String:
  match xs:
    case Nil{}:
      acc
    case x <> t:
      K.text(t, "  " ++ x ++ "\n" ++ acc)

def K.report(+a: KAcc, +shared1: Nat) -> String:
  match a:
    case KAcc{_, _, +off, +n, _, +nd, ds, +ns}:
      "lines " ++ Nat.show(n) ++ " (" ++ Nat.show(off) ++ " bytes); shared then " ++ Nat.show(shared1) ++ ", now " ++ Nat.show(ns) ++ "\ndiffering: " ++ Nat.show(nd) ++ "\n" ++ K.text(ds, "") ++
        Bool.pick(String, Nat.is_eq(nd, 0n), "ok far_fact_copy", "FAIL far_fact_copy differing=" ++ Nat.show(nd))

def K.ns(a: KAcc) -> Nat:
  match a:
    case KAcc{_, _, _, _, _, _, _, ns}:
      ns

def K.done(r: File & KAcc) -> IO(KAcc):
  (f, a) = r
  do IO<KAcc>:
    File.close(f)
    return a

def K.main(+home: String, +chunk: U32) -> IO(Unit):
  do IO<Unit>:
    +t0 : Nat <- IO.now()
    r1 : File & Nat <- K.open(home)
    p1 : File & KAcc <- K.run(r1, chunk, KAcc{M.State.empty(), Map.new(&2, String), 0n, 0n, False{}, 0n, Nil{}, 0n})
    +a1 : KAcc <- K.done(p1)
    +t1 : Nat <- IO.now()
    IO.print("pass 1 (fold) " ++ Nat.show(Nat.sub(t1, t0)) ++ " ms")
    r2 : File & Nat <- K.open(home)
    p2 : File & KAcc <- K.run(r2, chunk, K.again(a1))
    +a2 : KAcc <- K.done(p2)
    +t2 : Nat <- IO.now()
    IO.print("pass 2 (final state) " ++ Nat.show(Nat.sub(t2, t1)) ++ " ms")
    IO.print(K.report(a2, K.ns(a1)))

def K.chunk(xs: List<String>) -> U32:
  match xs:
    case _ <> c <> _:
      U32.from_nat(Maybe.default(&2, Nat, Nat.read(c), 1048576n))
    case _:
      1048576

def K.go(xs: List<String>) -> IO(Unit):
  match xs:
    case Nil{}:
      IO.die(Unit, 2, "usage: far_fact_copy HOME [CHUNK_BYTES]")
    case +home <> rest:
      K.main(home, K.chunk(home <> rest))

def main() -> IO(Unit):
  IO.bind(List<String>, Unit, A.Argv.args(), K.go)
