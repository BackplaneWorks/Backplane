import Base
import ../../src/core/model.bend as M
import ../../src/core/client.bend as U
import ../../src/core/subs.bend as SB
import ../../src/server/argv.bend as AV

# What a sidebar frame asks of the whole state, on a big one: 100
# projects, 3000 threads, 5000 asks, 800 delegated tasks. "old" walks the
# state per row (Side.rows, Side.phase, Subs.count), "ix" builds the index
# once a frame (Side.ix); "build" only builds the index, "rows" builds it
# and lays out the rows. Time each run from outside, less a run of 0:
#   time bend test/native/side_ix_bench.bend old 1
#   time bend test/native/side_ix_bench.bend ix 20
# (2026-10-02, bun with its JIT: old 11.2 s a frame; ix 205 ms, of which
# the index 140 ms and the rows 20 ms)
# (the JS build, as the web and the phones run it), or build it native
# with scripts/build-js.sh. It prints a checksum that must match.

def Gen.cat(xs: List<&2, M.Change>, ys: List<&2, M.Change>) -> List<&2, M.Change>:
  match xs:
    case Nil{}:
      ys
    case x <> t:
      Gen.cat(t, x <> ys)

def Gen.threads(+n: Nat, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match n:
    case 0n:
      acc
    case 1n+k:
      Gen.threads(k, M.ThreadCreated{"t" ++ Nat.show(k), "p" ++ Nat.show(Nat.mod(k, 100n)), "Thread " ++ Nat.show(k), M.Local{}, "", "", "claude", "", Nat.add(10n, k)}
        <> M.TurnChanged{"t" ++ Nat.show(k), Bool.pick(M.TurnState, Nat.is_eq(Nat.mod(k, 3n), 0n), M.Running{}, M.Completed{}), Nat.add(10n, k)} <> acc)

def Gen.projects(+n: Nat, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match n:
    case 0n:
      acc
    case 1n+k:
      Gen.projects(k, M.ProjectCreated{"p" ++ Nat.show(k), "Project " ++ Nat.show(k), "/p/" ++ Nat.show(k), Nat.add(1n, k)} <> acc)

def Gen.kind(+j: Nat) -> String:
  Bool.pick(String, Nat.is_eq(Nat.mod(j, 3n), 0n), "approval", Bool.pick(String, Nat.is_eq(Nat.mod(j, 3n), 1n), "input", "plan"))

def Gen.asks(+n: Nat, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match n:
    case 0n:
      acc
    case 1n+k:
      Gen.asks(k, M.AskOpened{"a" ++ Nat.show(k), "t" ++ Nat.show(Nat.mod(Nat.mul(k, 7n), 3000n)), Gen.kind(k), "Bash", "x", "", 20000n}
        <> Gen.cat(Bool.pick(List<&2, M.Change>, Nat.is_eq(Nat.mod(k, 2n), 0n), [M.AskClosed{"a" ++ Nat.show(k), "ok", 20001n}], Nil{}), acc))

def Gen.tasks(+n: Nat, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match n:
    case 0n:
      acc
    case 1n+k:
      Gen.tasks(k, M.TaskDelegated{"k" ++ Nat.show(k), "t" ++ Nat.show(Nat.mod(Nat.mul(k, 13n), 2000n)), "t" ++ Nat.show(Nat.add(2000n, Nat.mod(k, 1000n))), 1n, "claude", "", 30000n}
        <> Gen.cat(Bool.pick(List<&2, M.Change>, Nat.is_eq(Nat.mod(k, 2n), 0n), [M.TaskStateSet{"k" ++ Nat.show(k), M.TaskDone{}, "ok", 30001n}], Nil{}), acc))

def Gen.state() -> M.State:
  M.State.replay(Gen.projects(100n, Nil{}), M.State.replay(Gen.threads(3000n, Nil{}), M.State.replay(Gen.asks(5000n, Nil{}), M.State.replay(Gen.tasks(800n, Nil{}), M.State.empty()))))

def Sum.phase(p: M.Phase) -> Nat:
  String.length(M.Phase.name(p))

def Sum.rows.old(ts: List<&2, M.Thread>, +st: M.State, acc: Nat) -> Nat:
  match ts:
    case Nil{}:
      acc
    case +t <> rest:
      Sum.rows.old(rest, st, Nat.add(acc, Nat.add(Sum.phase(U.Side.phase(st, t)), SB.Subs.count(st, M.Thread.id(t)))))

def Sum.rows.ix(ts: List<&2, M.Thread>, +ix: U.SIx, +st: M.State, acc: Nat) -> Nat:
  match ts:
    case Nil{}:
      acc
    case +t <> rest:
      Sum.rows.ix(rest, ix, st, Nat.add(acc, Nat.add(Sum.phase(U.Side.phase.ix(ix, st, t)), SB.Subs.count.ix(ix, st, M.Thread.id(t)))))

def Sum.shelves(rs: List<&2, U.SRow>, acc: List<&2, M.Thread>) -> List<&2, M.Thread>:
  match rs:
    case Nil{}:
      acc
    case U.SRow{_, _, M.Shelves{p, a, se, _, sn}} <> t:
      Sum.shelves(t, List.append(&2, M.Thread, p, List.append(&2, M.Thread, a, List.append(&2, M.Thread, se, List.append(&2, M.Thread, sn, acc)))))

def Frame.old(+st: M.State) -> Nat:
  Sum.rows.old(Sum.shelves(U.Side.rows(st, 50000n), Nil{}), st, 0n)

def Frame.ix.k(+ix: U.SIx, +st: M.State) -> Nat:
  Sum.rows.ix(Sum.shelves(U.Side.rows.ix(st, ix, 50000n), Nil{}), ix, st, 0n)

def Frame.ix(+st: M.State) -> Nat:
  Frame.ix.k(U.Side.ix(st), st)

def SIx.size(ix: U.SIx) -> Nat:
  match ix:
    case U.SIx{by, as, ks, _}:
      Nat.add(List.length(&2, List<&2, M.Thread>, Map.values(&2, List<&2, M.Thread>, by)), List.length(&2, Nat, Map.values(&2, Nat, ks)))

def Frame.rows(+st: M.State) -> Nat:
  List.length(&2, U.SRow, U.Side.rows.ix(st, U.Side.ix(st), 50000n))

def Frame.build(+st: M.State) -> Nat:
  SIx.size(U.Side.ix(st))

def Frame.sel(+i: Nat, +st: M.State) -> Nat:
  match i:
    case 0n:
      Frame.ix(st)
    case 1n:
      Frame.old(st)
    case 2n:
      Frame.build(st)
    case _:
      Frame.rows(st)

def Frame.of(+mode: String, +st: M.State) -> Nat:
  Frame.sel(Bool.pick(Nat, String.eq(mode, "ix"), 0n, Bool.pick(Nat, String.eq(mode, "old"), 1n, Bool.pick(Nat, String.eq(mode, "build"), 2n, 3n))), st)

def Frames(n: Nat, +mode: String, +st: M.State, acc: Nat) -> Nat:
  match n:
    case 0n:
      acc
    case 1n+k:
      Frames(k, mode, st, Nat.add(acc, Frame.of(mode, st)))

def Bench.run(+mode: String, +n: Nat) -> IO(Unit):
  IO.print(mode ++ " frames " ++ Nat.show(n) ++ " sum " ++ Nat.show(Frames(n, mode, Gen.state(), 0n)))

def Bench.main(xs: List<&1, String>) -> IO(Unit):
  match xs:
    case Nil{}:
      Bench.run("ix", 5n)
    case +m <> Nil{}:
      Bench.run(m, 5n)
    case +m <> +n <> _:
      Bench.run(m, M.Switch.num(n, 0n))

def main() -> IO(Unit):
  IO.bind(List<&1, String>, Unit, AV.Argv.args(), xs => Bench.main(xs))
