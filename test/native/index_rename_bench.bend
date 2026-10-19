import Base
import ./memo_replay.bend as R
import ../index_rename_test.bend as F
import ../../src/core/model.bend as M
import ../../src/core/client.bend as U
import ../../src/core/json.bend as J
import ../../src/core/utf8.bend as U8
import ../../src/app/mm.bend as MM
import ../../src/server/argv.bend as AV
import ../../src/server/perf.bend as P

# Box deep client values across IO boundaries. Timings use the exact index
# force effect from Shell.paint.go; complete fresh-index equality is checked
# outside the timer for EVERY sample, rather than using that weak checksum.
type Input is Data:
  Input{label: String, saved: List<&2, MM.IxK>, after: List<&2, U.Ui>, touch: List<&2, MM.TouchK>, expected: String, checksum: Nat, old_mode: Nat, new_mode: Nat}

def Input.ui.go(us: List<&2, U.Ui>) -> U.Ui:
  match us:
    case u <> _: u
    case Nil{}: U.Ui.init(0n)

def Input.ui(input: Input) -> U.Ui:
  match input:
    case Input{_, _, us, _, _, _, _, _}: Input.ui.go(us)

def Input.k.go(ks: List<&2, MM.IxK>) -> MM.IxK:
  match ks:
    case k <> _: k
    case Nil{}: MM.Ix.full(U.Ui.init(0n), 0n)

def Input.k(input: Input) -> MM.IxK:
  match input:
    case Input{_, ks, _, _, _, _, _, _}: Input.k.go(ks)

def Input.touch(input: Input) -> MM.TouchK:
  match input:
    case Input{_, _, _, ts, _, _, _, _}: MM.TouchK.of(ts)

def Input.label(input: Input) -> String:
  match input:
    case Input{label, _, _, _, _, _, _, _}: label

def Input.expected(input: Input) -> String:
  match input:
    case Input{_, _, _, _, expected, _, _, _}: expected

def Input.checksum(input: Input) -> Nat:
  match input:
    case Input{_, _, _, _, _, checksum, _, _}: checksum

# Exact original Ix.mode, retained as the baseline. Only g vs index_g
# differs; the full/incremental builders and forcing implementation are
# shared unchanged, in the SAME binary/compiler/runtime/font environment.
def Old.mode(+n0: Nat, +lt0: String, +t: MM.TouchK, +u: U.Ui) -> Nat:
  Bool.pick(Nat, Bool.not(String.eq(lt0, U.Ui.info(u, "local.touch"))), 1n,
    Bool.pick(Nat, Bool.or(Nat.is_gt(MM.TouchK.g(t), n0), Nat.is_gt(MM.TouchK.r(t), n0)), 1n,
      Bool.pick(Nat, Nat.is_eq(MM.TouchK.n(t), n0), 0n, 2n)))

def Input.modes.ok(input: Input) -> Bool:
  match input:
    case Input{+label, _, _, _, _, _, old_mode, new_mode}:
      Bool.and(Nat.is_eq(old_mode, Bool.pick(Nat, String.eq(label, "turn-control"), 2n, 1n)), Nat.is_eq(new_mode, Bool.pick(Nat, String.eq(label, "global-before-rename"), 1n, 2n)))

def built(old: Bool, +input: Input) -> MM.IxK:
  match old:
    case True{}:
      MM.Ix.up.k(Old.mode(MM.IxK.n(Input.k(input)), MM.IxK.lt(Input.k(input)), Input.touch(input), Input.ui(input)), Input.k(input), Input.touch(input), Input.ui(input))
    case False{}:
      MM.Ix.up([Input.k(input)], Input.touch(input), Input.ui(input))

def prepare.k(label: String, +u: U.Ui, +k: MM.IxK, +t: MM.TouchK) -> Input:
  +fresh = MM.Ix.full(u, MM.TouchK.n(t))
  Input{label, [k], [u], [t], F.Fp.ix(MM.IxK.ix(fresh)), MM.IxK.force(fresh), Old.mode(MM.IxK.n(k), MM.IxK.lt(k), t, u), MM.Ix.mode(MM.IxK.n(k), MM.IxK.lt(k), t, u)}

def prepare(label: String, +u: U.Ui, +cs: List<&2, M.Change>) -> Input:
  prepare.k(label, U.Ui.fold(cs, u), MM.Ix.full(u, 0n), MM.TouchK.msg(F.changes(cs), MM.TouchK.none()))

def inputs(+u: U.Ui, +target: String) -> List<&2, Input>:
  [prepare("rename", u, [M.ThreadRenamed{target, "Native rename Δ | \\\"title\\\"", 1900000000n}]),
   prepare("turn-control", u, [M.TurnChanged{target, M.Running{}, 1900000000n}]),
   prepare("global-before-rename", u, [M.SettingSet{"perf.index.global.control", "1", 1900000000n}, M.ThreadRenamed{target, "Global rename", 1900000001n}])]

def present(m: Maybe<&2, M.Thread>) -> Bool:
  match m:
    case Some{_}: True{}
    case None{}: False{}

def checked(ok: Bool, label: String) -> IO(Unit):
  match ok:
    case True{}: IO.pure(Unit, Unit{})
    case False{}: IO.die(Unit, 1, "FAIL index rename native: " ++ label)

def sample(+old: Bool, +input: Input, +i: Nat, go: Unit -> IO(Unit)) -> IO(Unit):
  do IO<Unit>:
    a : U32 <- P.Perf.us()
    +k : MM.IxK = built(old, input)
    +v : Nat = MM.IxK.force(k)
    P.Perf.add("ui.ix.n", U32.from_nat(Nat.mod(v, 1000000n)))
    b : U32 <- P.Perf.us()
    checked(Nat.is_eq(v, Input.checksum(input)), "forcing checksum")
    checked(String.eq(F.Fp.ix(MM.IxK.ix(k)), Input.expected(input)), "complete fresh-index fields/asks/children/work")
    IO.print("sample scenario=" ++ Input.label(input) ++ " old=" ++ Bool.show(old) ++ " i=" ++ Nat.show(i) ++ " us=" ++ U32.show(U32.sub(b, a)) ++ " checksum=" ++ Nat.show(v))
    go(Unit{})

def samples(+fuel: Nat, +old: Bool, +input: Input) -> IO(Unit):
  match fuel:
    case 0n: IO.pure(Unit, Unit{})
    case 1n+k: sample(old, input, fuel, _ => samples(k, old, input))

def warm(+input: Input, +reps: Nat) -> IO(Unit):
  do IO<Unit>:
    checked(Input.modes.ok(input), "expected old/new index dispatch modes")
    # Force the complete independent oracle and input before the first clock.
    P.Perf.add("index.warm.oracle", U32.from_nat(Nat.mod(String.length(Input.expected(input)), 1000000n)))
    P.Perf.add("index.warm.old", U32.from_nat(Nat.mod(MM.IxK.force(Input.k(input)), 1000000n)))
    checked(String.eq(F.Fp.ix(MM.IxK.ix(built(True{}, input))), Input.expected(input)), "warm old exact")
    checked(String.eq(F.Fp.ix(MM.IxK.ix(built(False{}, input))), Input.expected(input)), "warm new exact")
    IO.print("phase " ++ Input.label(input) ++ " A1")
    samples(reps, True{}, input)
    IO.print("phase " ++ Input.label(input) ++ " B1")
    samples(reps, False{}, input)
    IO.print("phase " ++ Input.label(input) ++ " B2")
    samples(reps, False{}, input)
    IO.print("phase " ++ Input.label(input) ++ " A2")
    samples(reps, True{}, input)

def scenarios(xs: List<&2, Input>, +reps: Nat) -> IO(Unit):
  match xs:
    case Nil{}: IO.print("PASS exact fresh-index component ABBA; no whole-app FPS acceptance")
    case x <> rest:
      do IO<Unit>:
        warm(x, reps)
        scenarios(rest, reps)

def bench(+u: U.Ui, +target: String, reps: Nat) -> IO(Unit):
  do IO<Unit>:
    checked(present(MM.Ix.thread(U.Ui.state(u), target)), "requested thread exists in retained real log")
    scenarios(inputs(u, target), reps)

def main() -> IO(Unit):
  do IO<Unit>:
    args : List<String> <- AV.Argv.args()
    +as : List<&2, String> = R.copy(args)
    bytes : List<&2, U32> <- R.read(R.arg(as, 0n))
    bench(R.real(R.items(R.lines(U8.Lines.feed(U8.Lines.new(), bytes)), Nil{}), ""), R.arg(as, 1n), Maybe.default(&2, Nat, Nat.read(R.arg(as, 2n)), 40n))
