import Base
import ./check.bend as C
import ../src/server/host.bend as H
import ../src/core/store.bend as ST
import ../src/core/thin.bend as TH
import ../src/server/stobuild.bend as SB

# H.EvLog.read_from (effects/evlog.c, evlog.js) answers what
# ST.Store.read.from answers over the same file's bytes from that offset:
# the whole lines, decoded as utf8.bend's Utf8.decode (malformed bytes
# included), the tail's fix and the offset past the last whole line.
# Then the walks the JS hub makes over a long log and a long message
# (ST.Store.size, TH.Words.of) at sizes Bun's stack never held as plain
# recursion. Run on both targets: `bend test/evlog_read_test.bend` (JS)
# and its C build.

# code points, so a difference in U+FFFD placement shows
def show.cps.go(s: String, acc: String) -> String:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{c}, t}:
      show.cps.go(t, acc ++ U32.show(c) ++ ".")

def show.lines.go(ls: List<&2, String>, acc: String) -> String:
  match ls:
    case Nil{}:
      acc
    case l <> t:
      show.lines.go(t, acc ++ "[" ++ show.cps.go(l, "") ++ "]")

def show.read(+r: List<&2, String>) -> String:
  show.lines.go(ST.Store.read.body(r), "") ++ " tail " ++ show.cps.go(ST.Store.read.tail(r), "") ++ " end " ++ ST.Store.read.end(r)

type Case is Data:
  Case{name: String, bytes: List<&2, U32>}

def cases() -> List<&2, Case>:
  [Case{"empty", []},
   Case{"two lines", [97, 10, 98, 10]},
   Case{"torn tail", [97, 10, 98]},
   Case{"blank lines", [10, 10]},
   Case{"only torn", [97, 98]},
   Case{"utf8", [195, 169, 226, 130, 172, 240, 159, 152, 128, 10]},
   Case{"bad lead and cut", [255, 65, 195, 40, 226, 130, 10]},
   Case{"overlong", [192, 128, 224, 128, 175, 10]},
   Case{"surrogate", [237, 160, 128, 10]},
   Case{"past U+10FFFF", [244, 144, 128, 128, 10]},
   Case{"bom", [239, 187, 191, 65, 10]},
   Case{"cr", [97, 13, 10]},
   Case{"lone continuation", [128, 191, 10, 120, 10]}]

def fixture.close(w: File & Result<&1, &1, U32 & String, Unit>) -> IO(Unit):
  (f, _) = w
  File.close(f)

def fixture.k(r: Result<&1, &1, U32 & String, File>, +bs: List<&2, U32>) -> IO(Unit):
  match r:
    case Done{f}:
      do IO<Unit>:
        w : File & Result<&1, &1, U32 & String, Unit> <- File.write_bytes(f, bs)
        fixture.close(w)
    case Fail{_}:
      IO.print("FAIL cannot write the fixture")

def fixture(+path: String, +bs: List<&2, U32>) -> IO(Unit):
  IO.bind(Result<&1, &1, U32 & String, File>, Unit, File.open(path, "w"), r => fixture.k(r, bs))

# The builder's bounded native chunk reader must equal the original
# byte-list Cut.of for every decoder fixture, including truncated reads.
def chunk.lines(ls: List<&2, SB.Ln>, acc: String) -> String:
  match ls:
    case Nil{}:
      acc
    case SB.Ln{len, text} <> t:
      chunk.lines(t, acc ++ "[" ++ Nat.show(len) ++ ":" ++ show.cps.go(text, "") ++ "]")

def chunk.show(c: SB.Cut) -> String:
  match c:
    case SB.Cut{ls, used, total}:
      Nat.show(used) ++ "/" ++ Nat.show(total) ++ chunk.lines(ls, "")

def chunk.result(r: Result<&1, &1, U32 & String, List<&2, String>>, name: String, want: SB.Cut) -> IO(Unit):
  match r:
    case Done{xs}:
      C.Check.str(name, chunk.show(SB.Cut.native(xs)), chunk.show(want))
    case Fail{(_, e)}:
      C.Check.str(name, "read failed: " ++ e, chunk.show(want))

def chunk.got(r: File & Result<&1, &1, U32 & String, List<&2, String>>, name: String, want: SB.Cut) -> IO(Unit):
  (f, res) = r
  do IO<Unit>:
    File.close(f)
    chunk.result(res, name, want)

def chunk.opened(r: Result<&1, &1, U32 & String, File>, name: String, off: U32, max: U32, want: SB.Cut) -> IO(Unit):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, List<&2, String>>, Unit, H.EvLog.read_chunk(f, off, max), x => chunk.got(x, name, want))
    case Fail{(_, e)}:
      C.Check.str(name, "open failed: " ++ e, chunk.show(want))

def chunk.at(path: String, name: String, off: U32, max: U32, want: SB.Cut) -> IO(Unit):
  IO.bind(Result<&1, &1, U32 & String, File>, Unit, File.open(path, "r"), r => chunk.opened(r, name, off, max, want))

def case.read(+name: String, +bs: List<&2, U32>, +path: String) -> IO(Unit):
  do IO<Unit>:
    fixture(path, bs)
    got : List<&2, String> <- H.EvLog.read_from(path, "0")
    C.Check.str("read " ++ name, show.read(got), show.read(ST.Store.read.of(bs)))
    at2 : List<&2, String> <- H.EvLog.read_from(path, "2")
    C.Check.str("read " ++ name ++ " from 2", show.read(at2), show.read(ST.Store.read.from(2n, List.drop(&2, U32, bs, 2n))))
    chunk.at(path, "chunk " ++ name, 0, 4096, SB.Cut.of(bs))
    chunk.at(path, "chunk capped " ++ name, 0, 2, SB.Cut.of(List.take(&2, U32, bs, 2n)))
    chunk.at(path, "chunk offset " ++ name, 2, 4096, SB.Cut.of(List.drop(&2, U32, bs, 2n)))
    chunk.at(path, "chunk zero " ++ name, 0, 0, SB.Cut.of(Nil{}))
    chunk.at(path, "chunk past end " ++ name, 4096, 2, SB.Cut.of(Nil{}))

def case.run(c: Case, +path: String) -> IO(Unit):
  match c:
    case Case{name, +bs}:
      case.read(name, bs, path)

def all(cs: List<&2, Case>, +path: String) -> IO(Unit):
  match cs:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case c <> t:
      IO.bind(Unit, Unit, case.run(c, path), _ => all(t, path))

# n lines "x\n", made in a tail call
def many.go(n: Nat, acc: List<&2, U32>) -> List<&2, U32>:
  match n:
    case 0n:
      acc
    case 1n+k:
      many.go(k, 120 <> 10 <> acc)

# a message of n letters (a word every 8), lowered as the index reads it
def long.go(+n: Nat, acc: String) -> String:
  match n:
    case 0n:
      acc
    case 1n+k:
      long.go(k, SCon{Chr{Bool.pick(U32, Nat.is_eq(Nat.mod(k, 8n), 0n), 32, 65)}, acc})

def count(r: List<&2, String>) -> Nat:
  ST.Store.size(ST.Store.read.body(r))

def has(r: Map<&2, Bool> & Bool) -> String:
  (_, b) = r
  Bool.show(b)

def gone(r: Result<&1, &1, U32 & String, Unit>) -> IO(Unit):
  IO.pure(Unit, Unit{})

def main() -> IO(Unit):
  do IO<Unit>:
    t : Result<&1, &1, U32 & String, String> <- IO.get_env("TMPDIR")
    r : Result<&1, &1, U32 & String, U32> <- IO.random_u32()
    +tmp : String = Result.default(&1, &1, U32 & String, String, t, "/tmp")
    +n : U32 = Result.default(&1, &1, U32 & String, U32, r, 0)
    +path : String = tmp ++ "/evlog-test-" ++ U32.show(n) ++ ".jsonl"
    all(cases(), path)
    nf : List<&2, String> <- H.EvLog.read_from(tmp ++ "/evlog-test-none-" ++ U32.show(n), "7")
    C.Check.str("a missing file", show.read(nf), " tail  end 7")
    fixture(path, many.go(500000n, []))
    big : List<&2, String> <- H.EvLog.read_from(path, "0")
    C.Check.str("500k lines counted in a tail call", Nat.show(count(big)), "500000")
    tl : List<&2, String> <- H.EvLog.read_from(path, "999996")
    C.Check.str("the tail after an offset", show.read(tl), "[120.][120.] tail  end 1000000")
    IO.bind(Result<&1, &1, U32 & String, Unit>, Unit, H.FS.remove(path), gone)
    C.Check.str("a 500k-letter message indexed", has(Map.has(&2, Bool, TH.Words.of(long.go(500000n, "")), "aaaaaaa")), "True")
