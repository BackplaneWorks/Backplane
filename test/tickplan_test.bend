import Base
import ./check.bend as C
import ../src/core/tickplan.bend as T
import ../src/core/model.bend as M
import ../src/core/hub.bend as B
import ../src/core/codec.bend as CC
import ../src/core/json.bend as J
import ../src/core/usage.bend as U

def changes(xs: List<&2, M.Change>) -> List<&2, J.Json>:
  match xs:
    case Nil{}: Nil{}
    case c <> rest: CC.Change.encode(c) <> changes(rest)

def out(o: B.Out) -> J.Json:
  match o:
    case B.Git{client, id, job, thread, cwd, a, b}:
      J.Json.arr([J.Str{"Git"}, J.Str{U32.show(client)}, J.Str{id}, J.Str{job}, J.Str{thread}, J.Str{cwd}, J.Str{a}, J.Str{b}])
    case B.WakePoll{prs, files}:
      J.Json.arr([J.Str{"WakePoll"}, J.Str{String.join(prs, "\n")}, J.Str{String.join(files, "\n")}])
    case other: J.Str{B.Out.kind(other)}

def outs(xs: List<&2, B.Out>) -> List<&2, J.Json>:
  match xs:
    case Nil{}: Nil{}
    case o <> rest: out(o) <> outs(rest)

def step(s: B.Step) -> String:
  match s:
    case B.Step{cs, os}: J.Json.show(J.Json.arr([J.Json.arr(changes(cs)), J.Json.arr(outs(os))]))

def parity(+name: String, +s: M.State, +now: Nat) -> IO(Unit):
  C.Check.str(name, step(T.Tick.main(s, now)), step(B.Hub.tick(s, now)))

def drop.parity(name: String, +s: M.State) -> IO(Unit):
  C.Check.str(name, J.Json.show(J.Json.arr(outs(T.Tick.drops(B.Shelve.threads(s), M.Work.kids.map(s), s, 100n, 10n)))),
    J.Json.show(J.Json.arr(outs(B.Drop.asks(B.Shelve.threads(s), M.Work.kids.map(s), s, 100n, 10n)))))

def parents.check(ts: List<&2, M.Thread>, +s: M.State, +parents: Map<&2, String>, ok: Bool) -> Bool:
  match ts:
    case Nil{}: ok
    case t <> rest:
      +id = M.Thread.id(t)
      parents.check(rest, s, parents, Bool.and(ok, String.eq(T.Tick.parent(Map.get(String, "", parents, id)), B.Hub.settle.parent(s, id))))

# Historical tasks include duplicate child IDs and a terminal bot parent;
# Map.values order, rather than insertion order or task liveness, is policy.
def parents.fixture() -> M.State:
  M.State.replay([M.ProjectCreated{"p1", "project", "/project", 1n},
    M.ThreadCreated{"a", "p1", "a", M.Local{}, "", "", "claude", "", 1n},
    M.ThreadCreated{"b", "p1", "b", M.Local{}, "", "", "claude", "", 1n},
    M.BotSet{"bot", "Bub", "bot-thread", 0n, "", 1n},
    M.TaskDelegated{"first", "bot-thread", "a", 1n, "claude", "", 2n},
    M.TaskStateSet{"first", M.TaskDone{}, "done", 3n},
    M.TaskDelegated{"second", "person-thread", "a", 1n, "claude", "", 4n},
    M.TaskStateSet{"second", M.TaskDone{}, "done", 5n},
    M.TaskDelegated{"third", "person-thread", "b", 1n, "claude", "", 4n},
    M.TaskStateSet{"third", M.TaskDone{}, "done", 5n},
    M.TurnChanged{"a", M.Completed{}, 5n}, M.TurnChanged{"b", M.Failed{}, 5n}], M.State.empty())

def fixture.threads(n: Nat, +active: Nat, s: M.State) -> M.State:
  match n:
    case 0n: s
    case 1n+next:
      +k = next
      +id = "history-" ++ Nat.show(k)
      fixture.threads(k, active, M.State.replay([M.ThreadCreated{id, "p1", id, M.Worktree{}, "", "", "claude", "", 1n},
        M.WorktreeSet{id, "branch-" ++ Nat.show(k), "/project/.backplane/worktrees/w" ++ Nat.show(k), 2n},
        M.TurnChanged{id, Bool.pick(M.TurnState, Nat.is_lt(k, active), M.Running{}, M.Completed{}), 3n},
        M.ThreadSettled{id, False{}, 4n}], s))

def fixture.tasks(n: Nat, s: M.State) -> M.State:
  match n:
    case 0n: s
    case 1n+next:
      +k = next
      +id = "task-" ++ Nat.show(k)
      fixture.tasks(k, M.State.replay([M.TaskDelegated{id, "bot-thread", "history-" ++ Nat.show(k), 1n, "claude", "", 2n},
        M.TaskStateSet{id, M.TaskDone{}, "done", 3n}], s))

def fixture(n: Nat, active: Nat, tasks: Nat) -> M.State:
  fixture.tasks(tasks, fixture.threads(n, active, M.State.replay([M.ProjectCreated{"p1", "project", "/project", 1n},
    M.BotSet{"bot", "Bub", "bot-thread", 0n, "", 1n}], M.State.empty())))

def chips.parity(name: String, +s: M.State, +status: String, +now: Nat) -> IO(Unit):
  C.Check.str(name, J.Json.show(J.Json.arr(changes(T.Tick.chips(s, status, now)))), J.Json.show(J.Json.arr(changes(U.Chip.changes(s, status, now)))))

def main() -> IO(Unit):
  do IO<Unit>:
    parity("empty tick", M.State.empty(), 100n)
    parity("ordinary completed thread", B.Shelve.demo([M.TurnChanged{"t1", M.Completed{}, 5n}]), 1805n)
    parity("bot launched completed thread", B.Shelve.demo.bot([M.TurnChanged{"t1", M.Completed{}, 5n}]), 1805n)
    parity("bot idle window disabled", B.Shelve.demo.bot([M.TurnChanged{"t1", M.Failed{}, 5n}, M.SettingSet{"settle.idle.minutes", "0", 6n}]), 1805n)
    parity("snoozed bot thread", B.Shelve.demo.bot([M.TurnChanged{"t1", M.Interrupted{}, 5n}, M.ThreadSnoozed{"t1", 5000n, 6n}]), 1805n)
    parity("dormant project", M.State.apply(parents.fixture(), M.SettingSet{"project.dormant.p1", "on", 6n}), 1805n)
    C.Check.str("indexed parents preserve first match including terminal tasks", Bool.show(parents.check(B.Shelve.threads(parents.fixture()), parents.fixture(), T.Tick.parents(parents.fixture()), True{})), "True")
    parity("duplicate and terminal task parents", parents.fixture(), 1805n)
    parity("deleted archived pinned threads", M.State.replay([M.ThreadDeleted{"a", 6n}, M.ThreadArchived{"b", True{}, 6n}, M.ThreadPinned{"b", True{}, 7n}], parents.fixture()), 900000n)
    drop.parity("shared folder held by active thread", B.Wt.share.demo(Nil{}))
    drop.parity("shared folder held by running thread", B.Wt.share.demo([M.TurnChanged{"b", M.Running{}, 4n}]))
    drop.parity("both shared threads remove in original order", B.Wt.share.demo([M.ThreadSettled{"b", True{}, 4n}]))
    drop.parity("archived holder does not retain folder", B.Wt.share.demo([M.ThreadArchived{"b", True{}, 4n}]))
    drop.parity("deleted holder does not retain folder", B.Wt.share.demo([M.ThreadDeleted{"b", 4n}]))
    drop.parity("owned work retains shared folder", B.Wt.share.demo([M.ThreadSettled{"b", True{}, 4n}, M.WorkSet{"b", "bg", "w-test\tbuild\tcmd\tbuild\tshell", 5n}]))
    drop.parity("pending ask retains shared folder", B.Wt.share.demo([M.ThreadSettled{"b", True{}, 4n}, M.PendingChanged{"b", 1n, 5n}]))
    parity("tick with 12 running and historical threads", fixture(64n, 12n, 32n), 900000n)
    parity("tick with 20 running and historical threads", fixture(64n, 20n, 32n), 900000n)
    chips.parity("unreadable status preserves chips", parents.fixture(), "broken", 100n)
    chips.parity("missing generated time preserves chips", parents.fixture(), "{\"threads\":{}}", 100n)
    chips.parity("running chip current", parents.fixture(), "{\"generated_at\":100,\"threads\":{\"a\":{\"state\":\"running\",\"size\":\"small\",\"jobs\":2}}}", 110n)
    chips.parity("running chip stale", parents.fixture(), "{\"generated_at\":100,\"threads\":{\"a\":{\"state\":\"running\"}}}", 281n)
    chips.parity("warm chip remains valid", parents.fixture(), "{\"generated_at\":100,\"threads\":{\"a\":{\"state\":\"warm\",\"idle_until\":500}}}", 281n)
    chips.parity("removed chip is cleared", M.State.apply(parents.fixture(), M.SettingSet{U.Chip.key("a"), "running/small/1/0", 100n}), "{\"generated_at\":110,\"threads\":{}}", 110n)
