import Base
import ../../src/server/argv.bend as A
import ../../src/core/json.bend as J
import ../../src/core/model.bend as M
import ../../src/core/codec.bend as C
import ../../src/core/storedb.bend as SDB
import ../../src/core/utf8.bend as U

# What store.db's record tables must hold after HOME/events.jsonl: the log
# folded line by line (J.Json.parse, C.Change.decode, M.State.apply), then
# every record of the state (SDB.Tab.dump), one line each:
#   <table>\t<key cols>\t<all cols>
# The two column fields are JSON string literals (a value may hold a tab
# or a newline) whose columns are joined by "\u{1f}", as the pure store
# keeps them (SDB.Tab.key/Tab.op): an integer column reads "#<n>".
# test/tools/store_commit_e2e.ts compares it with store.db.
#   bend test/native/store_dump.bend HOME

type LN is Data:
  LN{len: Nat, text: String}

type Cut is Data:
  Cut{part: List<&2, U32>, n: Nat, done: List<&2, LN>}

def Cut.step(nl: Bool, +x: U32, c: Cut) -> Cut:
  match nl c:
    case True{} Cut{part, +n, done}:
      Cut{Nil{}, 0n, LN{n, U.Utf8.decode(List.reverse(&2, U32, part))} <> done}
    case False{} Cut{part, +n, done}:
      Cut{x <> part, 1n+n, done}

def Cut.go(bs: List<&2, U32>, c: Cut) -> Cut:
  match bs:
    case Nil{}:
      c
    case +x <> t:
      Cut.go(t, Cut.step(U32.is_eq(x, 10), x, c))

# the complete lines of a chunk, oldest first (a torn last line waits for
# the next chunk, which starts where the last complete line ended)
def Cut.done(c: Cut) -> List<&2, LN>:
  match c:
    case Cut{_, _, done}:
      List.reverse(&2, LN, done)

def Cut.lines(bs: List<&2, U32>) -> List<&2, LN>:
  Cut.done(Cut.go(bs, Cut{Nil{}, 0n, Nil{}}))

# the fold so far: state, byte offset, lines that parse
type B is Data:
  B{st: M.State, off: Nat, n: Nat}

def B.off(b: B) -> Nat:
  match b:
    case B{_, o, _}:
      o

def B.change(m: Maybe<&2, M.Change>, st: M.State) -> M.State:
  match m:
    case Some{c}:
      M.State.apply(st, c)
    case None{}:
      st

def B.parsed(m: Maybe<&2, J.Json>, +len: Nat, b: B) -> B:
  match m b:
    case Some{+o} B{st, +off, +n}:
      B{B.change(C.Change.decode(o), st), Nat.add(off, 1n+len), 1n+n}
    case None{} B{st, +off, n}:
      B{st, Nat.add(off, 1n+len), n}

def B.line(l: LN, b: B) -> B:
  match l:
    case LN{+len, +text}:
      B.parsed(J.Json.parse(text), len, b)

def B.lines(ls: List<&2, LN>, b: B) -> B:
  match ls:
    case Nil{}:
      b
    case l <> t:
      B.lines(t, B.line(l, b))

def Rd.done(r: File & Result<&1, &1, U32 & String, List<&2, U32>>) -> IO(List<&2, U32>):
  (f, res) = r
  do IO<List<&2, U32>>:
    File.close(f)
    return Result.default(&1, &1, U32 & String, List<&2, U32>, res, Nil{})

def Rd.at.k(r: Result<&1, &1, U32 & String, File>, +at: U32, +n: U32) -> IO(List<&2, U32>):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, List<&2, U32>>, List<&2, U32>, File.read_at(f, at, n), Rd.done)
    case Fail{_}:
      IO.pure(List<&2, U32>, Nil{})

def Rd.at(path: String, +at: U32, +n: U32) -> IO(List<&2, U32>):
  IO.bind(Result<&1, &1, U32 & String, File>, List<&2, U32>, File.open(path, "r"), r => Rd.at.k(r, at, n))

def Rd.size.done(r: File & Result<&1, &1, U32 & String, U32>) -> IO(U32):
  (f, res) = r
  do IO<U32>:
    File.close(f)
    return Result.default(&1, &1, U32 & String, U32, res, 0)

def Rd.size.k(r: Result<&1, &1, U32 & String, File>) -> IO(U32):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, U32>, U32, File.size(f), Rd.size.done)
    case Fail{_}:
      IO.pure(U32, 0)

def Rd.size(path: String) -> IO(U32):
  IO.bind(Result<&1, &1, U32 & String, File>, U32, File.open(path, "r"), Rd.size.k)

def Chunk() -> Nat:
  8388608n

# a chunk that ends no line (one line over the chunk size) stops the walk
def Fold.stuck(same: Bool, b: B, k: B -> IO(B)) -> IO(B):
  match same:
    case True{}:
      IO.pure(B, b)
    case False{}:
      k(b)

def Fold.chunk(more: Bool, +path: String, +size: Nat, b: B, k: B -> IO(B)) -> IO(B):
  match more:
    case False{}:
      IO.pure(B, b)
    case True{}:
      do IO<B>:
        +off : Nat = B.off(b)
        bs : List<&2, U32> <- Rd.at(path, U32.from_nat(off), U32.from_nat(Bool.pick(Nat, Nat.is_lt(Nat.sub(size, off), Chunk()), Nat.sub(size, off), Chunk())))
        +b2 : B = B.lines(Cut.lines(bs), b)
        Fold.stuck(Nat.is_eq(B.off(b2), off), b2, k)

def Fold.loop(fuel: Nat, +path: String, +size: Nat, b: B) -> IO(B):
  match fuel:
    case 0n:
      IO.pure(B, b)
    case 1n+f:
      +b2 = b
      Fold.chunk(Nat.is_lt(B.off(b2), size), path, size, b2, x => Fold.loop(f, path, size, x))

def Out.str(s: String) -> String:
  J.Json.show(J.Str{s})

# "<table>\x1f<key...>" → "<table>\t<key json>"
def Out.key(ps: List<&2, String>) -> String:
  match ps:
    case t <> rest:
      t ++ "\t" ++ Out.str(String.join(rest, SDB.Tab.sep()))
    case Nil{}:
      "\t\"\""

def Out.line(+k: String, v: String) -> String:
  Out.key(String.split(k, '\u{1f}')) ++ "\t" ++ Out.str(v)

def Out.go(xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> IO(Unit):
  match xs:
    case Nil{}:
      IO.pure(Unit, ())
    case (+k, v) <> t:
      IO.bind(Unit, Unit, IO.print(Out.line(k, v)), u => Out.go(t))

def Out.done(b: B) -> IO(Unit):
  match b:
    case B{+st, +off, +n}:
      do IO<Unit>:
        IO.print_err("store_dump: " ++ Nat.show(n) ++ " lines parsed, " ++ Nat.show(off) ++ " bytes, state seq " ++ Nat.show(M.State.seq(st)))
        Out.go(Map.to_list(&2, String, SDB.Tab.dump(st)))

def Args.copy(xs: List<String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      x <> Args.copy(t)

def Args.home(+xs: List<&2, String>) -> String:
  match xs:
    case h <> _:
      h
    case Nil{}:
      ""

def main() -> IO(Unit):
  do IO<Unit>:
    xs : List<String> <- A.Argv.args()
    +args : List<&2, String> = Args.copy(xs)
    +home : String = Args.home(args)
    +path : String = home ++ "/events.jsonl"
    sz : U32 <- Rd.size(path)
    +size : Nat = U32.to_nat(sz)
    b : B <- Fold.loop(1000000n, path, size, B{M.State.empty(), 0n, 0n})
    Out.done(b)
