import Base
import ../../src/server/argv.bend as A
import ../../src/core/model.bend as M
import ../snapequiv_lib.bend as Q

# The hot boot's equivalence on a copy of a real home (S3; never the live
# one: it rewrites HOME/store.db at each cut):
#   bend test/native/snap_equiv.bend HOME [CUT...]
# CUT: a byte offset into HOME/events.jsonl, or a percentage ("50%") of
# its size; default 0, 50% and 100%. For each cut, store.db is built from
# the lines that start before it and Hot.load'ed, and its records (per
# domain) and the open threads' entry windows are compared with the whole
# log folded (test/snapequiv_lib.bend). Exit 1 when anything differs.

def Args.copy(xs: List<String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      x <> Args.copy(t)

def Cut.of(+a: String, +size: Nat) -> Nat:
  Bool.pick(Nat, String.ends_with(a, "%"), Nat.div(Nat.mul(size, M.Switch.num(String.take(a, Nat.sub(String.length(a), 1n)), 0n)), 100n), M.Switch.num(a, 0n))

def Cuts.of(ws: List<&2, String>, +size: Nat) -> List<&2, Nat>:
  match ws:
    case Nil{}:
      Nil{}
    case +a <> t:
      Cut.of(a, size) <> Cuts.of(t, size)

def Cuts.args(+xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case _ <> Nil{}:
      ["0", "50%", "100%"]
    case _ <> t:
      t
    case Nil{}:
      Nil{}

def Home.of(+xs: List<&2, String>) -> String:
  match xs:
    case h <> _:
      h
    case Nil{}:
      ""

def Exit.k(bad: Bool, +refused: Nat) -> IO(Unit):
  match bad:
    case True{}:
      IO.die(Unit, 1, "snap_equiv: the hot boot differs from the full replay")
    case False{}:
      IO.print("snap_equiv: every cut the store was used at is equal; refused at " ++ Nat.show(refused))

def main() -> IO(Unit):
  do IO<Unit>:
    xs : List<String> <- A.Argv.args()
    +args : List<&2, String> = Args.copy(xs)
    +home : String = Home.of(args)
    +sz : U32 <- Q.Rd.size(home ++ "/events.jsonl")
    IO.print("snap_equiv: " ++ home ++ ", log " ++ U32.show(sz) ++ " bytes; folding it whole")
    +full : M.State <- Q.Full.state(home)
    IO.print("folded: seq " ++ Nat.show(M.State.seq(full)))
    +o : Q.Out <- Q.Equiv.cuts(Cuts.of(Cuts.args(args), U32.to_nat(sz)), home, full, Q.Out{0n, 0n})
    Exit.k(Nat.is_gt(Q.Out.bad(o), 0n), Q.Out.refused(o))
