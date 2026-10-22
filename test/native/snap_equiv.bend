import Base
import ../../src/server/argv.bend as A
import ../../src/core/model.bend as M
import ../snapequiv_lib.bend as Q
import ../../src/core/json.bend as J
import ../../src/core/codec.bend as CD
import ../../src/core/storedb.bend as S

# The hot boot's equivalence on a copy of a real home (S3; never the live
# one: it rewrites HOME/store.db). One folded state per process, so a real
# log fits (test/tools/snap_equiv.sh runs the phases and compares):
#   bend test/native/snap_equiv.bend build HOME CUT      store.db from the lines before CUT (bytes, or "50%")
#   bend test/native/snap_equiv.bend hot HOME OUT        Hot.load'ed records + open threads' entries
#   bend test/native/snap_equiv.bend full HOME OUT ID... the whole log folded: records + those threads' entries
#   bend test/native/snap_equiv.bend demo HOME           HOME/events.jsonl: storedb's demo log, threads left open (to try the phases)
# (test/snapequiv_lib.bend; test/snap_equiv_test.bend runs all in one on
# the demo logs)

def Args.copy(xs: List<String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      x <> Args.copy(t)

def Cut.of(+a: String, +size: Nat) -> Nat:
  Bool.pick(Nat, String.ends_with(a, "%"), Nat.div(Nat.mul(size, M.Switch.num(String.take(a, Nat.sub(String.length(a), 1n)), 0n)), 100n), M.Switch.num(a, 0n))

def Arg.at(xs: List<&2, String>, n: Nat) -> String:
  match xs n:
    case x <> _ 0n:
      x
    case _ <> t 1n+m:
      Arg.at(t, m)
    case Nil{} _:
      ""

def Build.run(+home: String, +cut: String) -> IO(Unit):
  do IO<Unit>:
    +sz : U32 <- Q.Rd.size(home ++ "/events.jsonl")
    IO.print("snap_equiv build: " ++ home ++ ", log " ++ U32.show(sz) ++ " bytes, cut " ++ cut)
    Q.Build.store(home, Cut.of(cut, U32.to_nat(sz)))

def Demo.lines(cs: List<&2, M.Change>, acc: String) -> String:
  match cs:
    case Nil{}:
      acc
    case c <> t:
      Demo.lines(t, acc ++ J.Json.show(CD.Change.encode(c)) ++ "\n")

def Demo.write(+home: String) -> IO(Unit):
  Q.Wr.file(home ++ "/events.jsonl", Demo.lines(List.take(&2, M.Change, S.Demo.changes(), Nat.sub(List.length(&2, M.Change, S.Demo.changes()), 5n)), ""))

def Mode.run(+mode: String, +args: List<&2, String>) -> IO(Unit):
  match mode:
    case "build":
      Build.run(Arg.at(args, 1n), Arg.at(args, 2n))
    case "hot":
      Q.Phase.hot(Arg.at(args, 1n), Arg.at(args, 2n))
    case "demo":
      Demo.write(Arg.at(args, 1n))
    case "full":
      Q.Phase.full(Arg.at(args, 1n), Arg.at(args, 2n), List.drop(&2, String, args, 3n))
    case _:
      IO.die(Unit, 2, "usage: snap_equiv.bend build HOME CUT | hot HOME OUT | full HOME OUT ID...")

def main() -> IO(Unit):
  do IO<Unit>:
    xs : List<String> <- A.Argv.args()
    +args : List<&2, String> = Args.copy(xs)
    Mode.run(Arg.at(args, 0n), args)
