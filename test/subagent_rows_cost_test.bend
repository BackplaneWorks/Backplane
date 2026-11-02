import Base
import ../src/core/model.bend as M
import ../src/core/subs.bend as SB

# A thread's subagent rows cost what the rows show, not what the subagents
# said: every render of the thread (a Subagents fold included) builds them,
# and a row keeps its newest Subs.shown() steps of up to Subs.window(). The
# driver (test/tools/subagent_rows_cost.mjs) times the rows over the same
# subagents with short and with long step lines: the time must not follow
# the length of steps no row shows (the older ones: a row keeps the
# newest Subs.shown(), and the walk applies step 1 last).
def steps(+n: Nat, +call: String, +text: String, st: M.State) -> M.State:
  match n:
    case 0n: st
    case 1n+k:
      steps(k, call, text, M.State.apply(st, M.ActivityLogged{"@sub:t1", call ++ "/" ++ Nat.show(n), call, "Bash", "Step " ++ Nat.show(n) ++ " " ++ Bool.pick(String, Nat.is_gt(n, SB.Subs.shown()), text, "x"), 200n}))

def agent(+call: String, +text: String, st: M.State) -> M.State:
  +begun = M.State.apply(st, M.ActivityLogged{"@sub:t1", call ++ "/start", call, "start:subagent", "Review " ++ call, 100n})
  M.State.apply(steps(SB.Subs.window(), call, text, begun), M.ActivityLogged{"@sub:t1", call ++ "/end", call, "end", "completed", 201n})

def agents(+n: Nat, +text: String, st: M.State) -> M.State:
  match n:
    case 0n: st
    case 1n+k: agents(k, text, agent("c" ++ Nat.show(n), text, st))

# eight finished subagents, each with Subs.window() steps, those no row
# shows about len characters long
def state(+len: Nat) -> M.State:
  agents(8n, String.repeat("x", len), M.State.empty())

def step.texts(ss: List<&2, SB.SubStep>) -> List<&2, String>:
  match ss:
    case Nil{}: Nil{}
    case SB.SubStep{_, text} <> t: String.take(text, 8n) <> step.texts(t)

def shown(rs: List<&2, SB.SubRow>) -> List<&2, String>:
  match rs:
    case Nil{}: Nil{}
    case SB.SubRow{key, _, title, _, word, _, _, _, n, ss} <> t:
      (key ++ "|" ++ title ++ "|" ++ word ++ "|" ++ Nat.show(n) ++ "|" ++ String.join(step.texts(ss), ",")) <> shown(t)

def opened(+n: Nat, m: Map<&2, String>) -> Map<&2, String>:
  match n:
    case 0n: m
    case 1n+k: opened(k, Map.set(&2, String, m, "fold:sub:c" ++ Nat.show(n), "1"))

# what the panel shows, every row opened (its steps included)
def rows(+st: M.State) -> String:
  String.join(shown(SB.Subs.rows(st, opened(8n, Map.set(&2, String, Map.new(&2, String), "fold:subs.done:t1", "1")), "t1", 210n)), "\n")
