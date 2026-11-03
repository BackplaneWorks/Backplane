import Base
import ./check.bend as C
import ./autosettle_test.bend as F
import ./tickplan_test.bend as W
import ../src/core/autosettle.bend as A
import ../src/core/model.bend as M
import ../src/core/hub.bend as B
import ../src/core/json.bend as J

# Bounded recursive reference implementations make event ordering explicit.
# The live implementations use accumulators for large-store stack safety.
def shown(cs: List<&2, M.Change>) -> String:
  J.Json.show(J.Json.arr(W.changes(cs)))

def go(xs: List<&2, M.Thread>, +s: M.State, +now: Nat, +ix: A.Index) -> List<&2, M.Change>:
  match xs:
    case Nil{}: Nil{}
    case +t <> rest:
      A.Auto.emit(A.Auto.due(s, t, now, ix), M.Thread.id(t), now, go(rest, s, now, ix))

def quiet(xs: List<&2, Sigma<&2, &2, String, _ => Nat>>, +s: M.State) -> List<&2, M.Change>:
  match xs:
    case Nil{}: Nil{}
    case (+th, +at) <> rest:
      A.Auto.emit.quiet(Bool.and(Bool.not(String.is_empty(th)), Nat.is_gt(at, A.Auto.num(s, "autosettle.quiet." ++ th, 0n))), th, at, quiet(rest, s))

def receipts(xs: List<&2, Sigma<&2, &2, String, _ => Nat>>, +s: M.State, +now: Nat) -> List<&2, M.Change>:
  match xs:
    case Nil{}: Nil{}
    case (+key, at) <> rest:
      A.Auto.receipt.keep(A.Auto.receipt.relevant(M.Shelf.of(M.State.thread.get(s, M.Bg.field(key, 0n)))), key, at, now, receipts(rest, s, now))

def metadata(cs: List<&2, M.Change>, +s: M.State) -> List<&2, M.Change>:
  match cs:
    case Nil{}: Nil{}
    case c <> rest:
      +one = A.Auto.message.metadata(c, s)
      List.append(&2, M.Change, one, metadata(rest, M.State.replay(one, s)))

def upgrade(ix: A.Index, +s: M.State, +now: Nat) -> List<&2, M.Change>:
  match ix s:
    case A.Index{_, _, reports, _} M.State{_, ts, _, _, _, _, _, _, _, _, _}:
      List.append(&2, M.Change, go(Map.values(&2, M.Thread, ts), s, now, A.Auto.index(s)),
        List.append(&2, M.Change, receipts(Map.to_list(&2, Nat, reports), s, now), [M.SettingSet{"autosettle.version", "1", now}]))

def main() -> IO(Unit):
  +s = F.base(True{}, [M.SettingSet{"autosettle.quiet.parent", "20", 20n}, M.MessagePosted{"parent", M.Link.id("t", "lr-report"), M.User{}, "Done", 5n}])
  +xs : List<&2, Sigma<&2, &2, String, _ => Nat>> = [("t", 8n), ("", 9n), ("parent", 10n), ("t", 12n)]
  +cs : List<&2, M.Change> = [M.MessagePosted{"t", "human", M.User{}, "Hello", 10n}, M.MessagePosted{"t", "reply", M.Assistant{}, "Result", 11n}, M.MessagePosted{"t", M.Link.id("parent", "note"), M.User{}, "Worker note", 12n}]
  do IO<Unit>:
    C.Check.str("quiet accumulator preserves exact order and filtering", shown(A.Auto.quiet.changes(xs, s)), shown(quiet(xs, s)))
    C.Check.str("receipt accumulator preserves exact order", shown(A.Auto.old.receipts(xs, 30n, s)), shown(receipts(xs, s, 30n)))
    C.Check.str("metadata accumulator preserves exact order and sticky audience", shown(A.Auto.messages.metadata(cs, s)), shown(metadata(cs, s)))
    C.Check.str("decision accumulator preserves exact order", shown(A.Auto.changes(s, 1000n)), shown(go(B.Shelve.threads(s), s, 1000n, A.Auto.index(s))))
    C.Check.str("upgrade decisions receipts then version marker", shown(A.Auto.upgrade(s, 1000n)), shown(upgrade(A.Auto.index(s), s, 1000n)))
