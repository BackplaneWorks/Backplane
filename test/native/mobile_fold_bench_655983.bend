import Base
import ./mobile_fold_materialization_655983.bend as T
import ../../src/mobile/screen.bend as MOB
import ../../src/core/model.bend as M
import ../../src/core/fold.bend as F
import ../../src/core/stamp.bend as ST
import ../../src/core/i18n.bend as I
import ../../src/core/json.bend as J

def render.old() -> String:
  +es = T.entries()
  +rs : List<&2, F.Role> = [F.RShow{}, F.RHead{"fold", "tools", False{}, False{}}, F.RIn{False{}}, F.RShow{}, F.RShow{}]
  +st = M.State.empty()
  +sp = ST.Stamps.of.s(es, st, 100n, False{}, False{})
  J.Json.show(J.Json.arr(List.reverse(&2, J.Json, T.legacy(es, rs, st, sp, "", Nil{}))))

def render.new() -> String:
  +es = T.entries()
  +rs : List<&2, F.Role> = [F.RShow{}, F.RHead{"fold", "tools", False{}, False{}}, F.RIn{False{}}, F.RShow{}, F.RShow{}]
  +st = M.State.empty()
  +sp = ST.Stamps.of.s(es, st, 100n, False{}, False{})
  J.Json.show(J.Json.arr(List.reverse(&2, J.Json, MOB.Mob.entries.go(I.En{}, es, rs, st, "t", Map.new(&2, String), sp, "", Nil{}))))

def render(old: Bool) -> String:
  match old:
    case True{}: render.old()
    case False{}: render.new()

def measure(+old: Bool, +n: Nat) -> IO(Unit):
  match n:
    case 0n: IO.pure(Unit, Unit{})
    case 1n+k:
      do IO<Unit>:
        +a : Nat <- IO.now()
        +s : String = render(old)
        IO.print("bytes " ++ Nat.show(String.length(s)))
        +b : Nat <- IO.now()
        IO.print("measurement " ++ Bool.show(old) ++ " " ++ Nat.show(Nat.sub(b, a)))
        measure(old, k)

def main() -> IO(Unit):
  do IO<Unit>:
    IO.print("old-output " ++ render.old())
    IO.print("new-output " ++ render.new())
    measure(True{}, 6n)
    measure(False{}, 6n)
