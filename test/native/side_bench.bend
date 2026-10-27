import Base
import ../../src/core/client.bend as U
import ../../src/core/model.bend as M
import ../../src/core/json.bend as J
import ../../src/core/utf8.bend as U8
import ../../src/server/argv.bend as AV

# The sidebar's rows, timed on a real log: Side.rows and its parts, 20
# times each (ms for all 20).
#   scripts/build-js.sh test/native/side_bench.bend build/side_bench
#   build/side_bench EVENTS.jsonl

def items(ls: List<&2, String>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match ls:
    case Nil{}:
      List.reverse(&2, J.Json, acc)
    case l <> t:
      items(t, Maybe.default(&2, J.Json, J.Json.parse(l), J.Null{}) <> acc)

def lines(r: U8.Lines & List<&2, String>) -> List<&2, String>:
  (_, ls) = r
  ls

def real(bs: List<&2, U32>) -> U.Ui:
  U.Ui.recv(U.Ui.init(1790180000n), J.Json.obj([J.KV{"t", J.Str{"log"}}, J.KV{"version", J.Str{"0.1.0"}},
    J.KV{"items", J.Json.arr(items(lines(U8.Lines.feed(U8.Lines.new(), bs)), Nil{}))}]))

def read.k(r: File & Result<&1, &1, U32 & String, List<&2, U32>>) -> IO(List<&2, U32>):
  (f, res) = r
  do IO<List<&2, U32>>:
    File.close(f)
    bs : List<&2, U32> <- IO.pass(List<&2, U32>, res)
    return bs

def read(+path: String) -> IO(List<&2, U32>):
  do IO<List<&2, U32>>:
    f : File <- IO.try(File, File.open(path, "r"))
    r : File & Result<&1, &1, U32 & String, List<&2, U32>> <- File.read_bytes(f, 200000000)
    read.k(r)

def copy(xs: List<&1, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      x <> copy(t)

def arg(+xs: List<&2, String>, +i: Nat) -> String:
  Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, xs, i)), "")

def rep.rows(+r: Nat, +st: M.State, +now: Nat) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(List.length(&2, U.SRow, U.Side.rows(st, now)), rep.rows(q, st, now))

def rep.kids(+r: Nat, +st: M.State) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(List.length(&2, M.Thread, U.Kids.keep(U.Ui.threads(st), U.Kids.set(st))), rep.kids(q, st))

def rep.go(+r: Nat, +st: M.State, +now: Nat) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(List.length(&2, U.PSlot, U.Side.rows.go(U.Ui.projects(st), st, U.Ui.threads(st), M.State.navs(st), now)), rep.go(q, st, now))

def shelves.n(s: M.Shelves) -> Nat:
  match s:
    case M.Shelves{p, a, st, h, sn}:
      Nat.add(Nat.add(List.length(&2, M.Thread, p), List.length(&2, M.Thread, a)), Nat.add(List.length(&2, M.Thread, st), Nat.add(List.length(&2, M.Thread, h), List.length(&2, M.Thread, sn))))

def rep.all(+r: Nat, +st: M.State, +now: Nat) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(shelves.n(M.Sidebar.at(U.Ui.threads(st), M.State.navs(st), now)), rep.all(q, st, now))

def main() -> IO(Unit):
  do IO<Unit>:
    args : List<String> <- AV.Argv.args()
    +as : List<&2, String> = copy(args)
    bs : List<&2, U32> <- read(arg(as, 0n))
    +u : U.Ui = real(bs)
    +st : M.State = U.Ui.state.of(u)
    +now : Nat = 1790520000n
    IO.print("threads " ++ Nat.show(List.length(&2, M.Thread, U.Ui.threads(st))) ++ ", projects " ++ Nat.show(List.length(&2, M.Project, U.Ui.projects(st))))
    +a : Nat <- IO.now()
    +x1 : Nat = rep.rows(20n, st, now)
    +b : Nat <- IO.now()
    +x2 : Nat = rep.kids(20n, st)
    +c : Nat <- IO.now()
    +x3 : Nat = rep.go(20n, st, now)
    +d : Nat <- IO.now()
    +x4 : Nat = rep.all(20n, st, now)
    +e : Nat <- IO.now()
    IO.print("x20 ms: Side.rows " ++ Nat.show(Nat.sub(b, a)) ++ "  kids " ++ Nat.show(Nat.sub(c, b)) ++ "  rows.go (per-project filter + shelves) " ++ Nat.show(Nat.sub(d, c))
      ++ "  shelves of all threads at once " ++ Nat.show(Nat.sub(e, d)) ++ "  (" ++ Nat.show(Nat.add(Nat.add(x1, x2), Nat.add(x3, x4))) ++ ")")
