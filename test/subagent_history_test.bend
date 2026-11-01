import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/subs.bend as SB
import ../src/core/client.bend as U

# One completed review followed by another agent's busy activity. Its
# identity and its own newest steps must survive.
def steps(+n: Nat, +call: String, st: M.State) -> M.State:
  match n:
    case 0n: st
    case 1n+k:
      steps(k, call, M.State.apply(st, M.ActivityLogged{"@sub:t1", call ++ "/" ++ Nat.show(n), call, "Bash", "Inspect " ++ Nat.show(n), 200n}))

def started() -> M.State:
  M.State.apply(M.State.empty(), M.ActivityLogged{"@sub:t1", "c1/start", "c1", "start:subagent", "Review power", 100n})

def busy(+done: Bool) -> M.State:
  +own = steps(12n, "c1", started())
  +ended = Bool.pick(M.State, done, M.State.apply(own, M.ActivityLogged{"@sub:t1", "c1/end", "c1", "end", "completed", 201n}), own)
  steps(450n, "c2", M.State.apply(ended, M.ActivityLogged{"@sub:t1", "c2/start", "c2", "start:subagent", "Review routing", 202n}))

def finished() -> M.State:
  M.State.apply(busy(True{}), M.ActivityLogged{"@sub:t1", "c2/end", "c2", "end", "completed", 203n})

def rows(+st: M.State, +info: Map<&2, String>) -> List<&2, SB.SubRow>:
  SB.Subs.rows(st, info, "t1", 210n)

def find(rs: List<&2, SB.SubRow>, +call: String) -> SB.SubRow:
  match rs:
    case Nil{}: SB.SubRow{"", True{}, "", "", "", "", False{}, False{}, 0n, Nil{}}
    case +r <> t: Bool.pick(SB.SubRow, String.eq(SB.Subs.key(r), call), r, find(t, call))

def step.texts(ss: List<&2, SB.SubStep>) -> List<&2, String>:
  match ss:
    case Nil{}: Nil{}
    case SB.SubStep{_, text} <> t: text <> step.texts(t)

def fingerprint(r: SB.SubRow) -> String:
  match r:
    case SB.SubRow{_, _, title, _, word, _, _, _, _, ss}:
      title ++ "|" ++ word ++ "|" ++ String.join(step.texts(ss), ",")

def expanded() -> Map<&2, String>:
  Map.set(&2, String, Map.set(&2, String, Map.new(&2, String), "fold:subs.done:t1", "1"), "fold:sub:c1", "1")

def main() -> IO(Unit):
  do IO<Unit>:
    +info : Map<&2, String> = expanded()
    C.Check.str("other agents cannot erase completed reviews", SB.Subs.panel.word(rows(finished(), Map.new(&2, String))), "2 done")
    C.Check.str("completed review opens its own newest steps", fingerprint(find(rows(finished(), info), "c1")), "Review power|done|Inspect 6,Inspect 5,Inspect 4,Inspect 3,Inspect 2,Inspect 1")
    +live : M.State = M.State.apply(busy(False{}), M.WorkSet{"t1", "bg", "a1\tChecking power\tc1\tReview power\tsubagent\na2\tChecking routing\tc2\tReview routing\tsubagent", 204n})
    C.Check.str("busy neighbour cannot empty a running review", fingerprint(find(rows(live, info), "c1")), "Review power|running|Inspect 6,Inspect 5,Inspect 4,Inspect 3,Inspect 2,Inspect 1")
    C.Check.str("live counts still come from provider work", Nat.show(SB.Subs.count(live, "t1")), "2")
    C.Check.str("finished counts survive expanding the group", SB.Subs.panel.word(rows(finished(), info)), "2 done")
