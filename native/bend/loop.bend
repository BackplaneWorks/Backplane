import Base
import ./status.bend as St
import ./history.bend as Hi
import ./kcommon.bend as C
import ../../src/core/model.bend as M

# Memory behaviour of the native runtime (bendlang/bend#1007): one process
# repeats an allocation-heavy kernel N times and prints, per iteration, its
# wall ms, the process RSS in pages (/proc/self/statm) and the digest.
#   loop KERNEL ITERATIONS      KERNEL 1: status (1000 threads), 2: history (16 pages)

def Mem.pick(xs: List<&2, String>) -> Nat:
  match xs:
    case _ <> x <> _: C.K.digits(x, 0n)
    case _: 0n

# the second field of statm: resident pages
def Mem.second(s: String) -> Nat:
  Mem.pick(String.split(s, ' '))

def Mem.str(res: Result<&1, &1, U32 & String, String>) -> String:
  match res:
    case Done{s}: s
    case Fail{_}: ""

def Mem.done(r: File & Result<&1, &1, U32 & String, String>) -> IO(Nat):
  (f, res) = r
  do IO<Nat>:
    File.close(f)
    return Mem.second(Mem.str(res))

def Mem.rss() -> IO(Nat):
  do IO<Nat>:
    f : File <- IO.try(File, File.open("/proc/self/statm", "r"))
    r : File & Result<&1, &1, U32 & String, String> <- File.read(f, 256)
    n : Nat <- Mem.done(r)
    return n

def L.work(which: Nat, +ths: List<&2, M.Thread>, +st: M.State, +qs: List<&2, List<&2, List<&2, String>>>) -> Nat:
  match which:
    case 1n: St.S.once(ths, st)
    case _: Hi.H.once(qs, 0n, 0n)

def L.go(k: Nat, +i: Nat, +which: Nat, +ths: List<&2, M.Thread>, +st: M.State, +qs: List<&2, List<&2, List<&2, String>>>) -> IO(Unit):
  match k:
    case 0n: IO.pure(Unit, Unit{})
    case 1n+rest:
      do IO<Unit>:
        t0 : Nat <- IO.now()
        +d : Nat = L.work(which, ths, st, qs)
        IO.print("it " ++ Nat.show(i) ++ " digest " ++ Nat.show(d))
        t1 : Nat <- IO.now()
        pages : Nat <- Mem.rss()
        IO.print("it " ++ Nat.show(i) ++ " ms " ++ Nat.show(Nat.sub(t1, t0)) ++ " rss_pages " ++ Nat.show(pages))
        L.go(rest, Nat.add(i, 1n), which, ths, st, qs)

def main() -> IO(Unit):
  do IO<Unit>:
    +a : C.Args <- C.K.args()
    +n : Nat = Bool.pick(Nat, Nat.is_eq(C.K.Args.c(a), 0n), 1000n, C.K.Args.c(a))
    +st : M.State = St.G.state(n)
    +ths : List<&2, M.Thread> = St.G.list(n, 0n, Nat.add(Nat.div(n, 10n), 1n), Nil{})
    +qs : List<&2, List<&2, List<&2, String>>> = Hi.H.queries(16n, 0n, Nil{})
    p0 : Nat <- Mem.rss()
    IO.print("start rss_pages " ++ Nat.show(p0))
    L.go(C.K.Args.b(a), 1n, C.K.Args.a(a), ths, st, qs)
