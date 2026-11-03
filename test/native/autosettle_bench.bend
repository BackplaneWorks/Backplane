import Base
import ../../src/core/model.bend as M
import ../../src/core/autosettle.bend as A
import ../../src/core/tickplan.bend as T
import ../../src/core/hub.bend as B
import ../../src/core/mail.bend as ML
import ../tickplan_test.bend as W
import ../check.bend as C
import ../../src/core/signal.bend as SG

# Realistic resident shape: ~10k historical records, 20 running turns,
# 40 completed workers with 50 messages each, shared worktree groups,
# 476 historical tasks (matching the copied store) and scalable mail.
def messages(n: Nat, +id: String, s: M.State) -> M.State:
  match n:
    case 0n: s
    case 1n+next:
      +k = next
      messages(k, id, M.State.apply(s, M.MessagePosted{id, id ++ ":answer:" ++ Nat.show(k), M.Assistant{}, "A synthetic work result for this thread", Nat.add(k, 1793672100n)}))

def history(old: Bool, +id: String, +k: Nat, s: M.State) -> M.State:
  match old:
    case True{}:
      M.State.replay(M.ThreadSettled{id, False{}, 1793673100n} <>
        Bool.pick(List<&2, M.Change>, Nat.is_lt(k, 536n),
          [M.TaskDelegated{"task-" ++ id, "parent", id, 1n, "codex", "", 2n},
           M.TaskStateSet{"task-" ++ id, M.TaskDone{}, "done", 1793673000n}, M.TaskDelivered{"task-" ++ id, 1793673000n}], Nil{}), s)
    case False{}: messages(50n, id, s)


def thread(+id: String, +k: Nat, s: M.State) -> M.State:
  +s1 = M.State.replay([M.ThreadCreated{id, "p", id, M.Worktree{}, "", "", "codex", "", 1n},
    M.WorktreeSet{id, "unmerged", "/tmp/autosettle/worktree-" ++ Nat.show(Nat.div(k, 4n)), 2n},
    M.ThreadLaunched{id, "parent", True{}, 2n},
    M.TurnChanged{id, Bool.pick(M.TurnState, Nat.is_lt(k, 20n), M.Running{}, M.Completed{}), 1793673000n},
    M.SettingSet{"autosettle.report." ++ id, "1793673000", 1793673000n}, M.SettingSet{"autosettle.audience." ++ id, "worker", 2n}], s)
  history(Nat.is_ge(k, 60n), id, k, s1)


def threads(n: Nat, s: M.State) -> M.State:
  match n:
    case 0n: s
    case 1n+next:
      +k = next
      threads(k, thread("th-" ++ Nat.show(k), k, s))

def mail(n: Nat, m: Map<&2, List<&2, ML.Mail>>) -> Map<&2, List<&2, ML.Mail>>:
  match n:
    case 0n: m
    case 1n+next:
      +k = next
      +id = "archive-" ++ Nat.show(k)
      mail(k, Map.set(&2, List<&2, ML.Mail>, m, id,
        [ML.Mail{M.Link.id(id, "lr-old"), id, "thread:" ++ id, "thread:parent", "launch", "", "fyi", "Synthetic archived report", 100n, 1n}]))

def fixture(n: Nat, mails: Nat) -> M.State:
  +archive = mail(mails, Map.new(&2, List<&2, ML.Mail>))
  M.State.with_mail((box => ML.Box.with_mails(box, archive)), threads(n, M.State.replay([
    M.ProjectCreated{"p", "Project", "/tmp/autosettle", 1n}, M.SettingSet{"autosettle.version", "1", 1n}], M.State.empty())))

def tick(+s: M.State, now: Nat) -> String:
  W.step(T.Tick.main(s, now))

def quiet_batch(n: Nat, m: Map<&2, Nat>) -> Map<&2, Nat>:
  match n:
    case 0n: m
    case 1n+next:
      +k = next
      quiet_batch(k, Map.set(&2, Nat, m, "q-" ++ Nat.show(k), 1793673000n))

def quiet(n: Nat) -> Nat:
  List.length(&2, M.Change, A.Auto.quiet.changes(Map.to_list(&2, Nat, quiet_batch(n, Map.new(&2, Nat))), M.State.empty()))

def upgraded(+s: M.State) -> Nat:
  List.length(&2, M.Change, A.Auto.upgrade(s, 1793674000n))

def old_reports(+s: M.State) -> Nat:
  List.length(&2, Nat, Map.values(&2, Nat, A.Auto.old.reports(s)))

# Keep state inside Bend while measuring. Calling tick(state) from a JS
# exported-module driver converts the entire snapshot at each call, which
# is not a cost the running hub pays and swamps the actual planner.
def legacy.base(+s: M.State, +now: Nat) -> B.Step:
  match s:
    case M.State{ps, +ts, _, _, _, _, _, _, _, _, _}:
      B.Step.before(B.Shelve.after(T.Tick.settles(Map.values(&2, M.Thread, ts), T.Tick.parents(s), M.State.navs(s), M.Work.kids.map(s), s, now, B.Hub.settle.window(s), B.Hub.settle.idle(s)), s, now, M.Work.kids.map(s)),
        List.append(&2, B.Out, B.Merge.asks(Map.values(&2, M.Project, ps), Map.values(&2, M.Thread, ts), s), T.Tick.drops(Map.values(&2, M.Thread, ts), M.Work.kids.map(s), s, now, B.Hub.wt.window(s))),
        B.Wt.stale(Map.values(&2, M.Thread, ts), s, now))

def legacy(+s: M.State, +now: Nat) -> String:
  W.step(B.Step.join(B.Step.join(B.Step.join(B.Step.join(legacy.base(s, now), B.Update.due(s, now)), B.Update.held(s, now)),
    B.Step.cat(B.Step.cat(B.Lr.far.tick(s), B.Fq.tick(s)), B.Wake.poll(s))), B.Step.cat(B.Deploy.tick(s, now), B.Ship.tick(s, now))))

def measure(+s: M.State, +n: Nat, +mails: Nat, rounds: Nat) -> IO(Unit):
  match rounds:
    case 0n: IO.pure(Unit, Unit{})
    case 1n+k:
      do IO<Unit>:
        +t0 : Nat <- IO.now()
        +baseline : String = legacy(s, 1793674000n)
        +t1 : Nat <- IO.now()
        +decisions : Nat = List.length(&2, M.Change, A.Auto.changes(s, 1793674000n))
        +t2 : Nat <- IO.now()
        +planned : String = tick(s, 1793674000n)
        +t3 : Nat <- IO.now()
        IO.print("threads=" ++ Nat.show(n) ++ " mail=" ++ Nat.show(mails) ++ " old_ms=" ++ Nat.show(Nat.sub(t1, t0)) ++ " policy_ms=" ++ Nat.show(Nat.sub(t2, t1)) ++ " new_ms=" ++ Nat.show(Nat.sub(t3, t2)) ++ " decisions=" ++ Nat.show(decisions) ++ " old_bytes=" ++ Nat.show(String.length(baseline)) ++ " new_bytes=" ++ Nat.show(String.length(planned)))
        C.Check.str("eligible worker decisions exercised", Nat.show(decisions), "40")
        C.Check.str("automatic settlement policy below 750 ms", Bool.show(Nat.is_lt(Nat.sub(t2, t1), 750n)), "True")
        C.Check.str("full tick within indexed baseline plus 1000 ms budget", Bool.show(Nat.is_le(Nat.sub(t3, t2), Nat.add(Nat.sub(t1, t0), 1000n))), "True")
        measure(s, n, mails, k)

def bench(+n: Nat, +mails: Nat) -> IO(Unit):
  +s = fixture(n, mails)
  do IO<Unit>:
    C.Check.str("steady upgrade is empty", Nat.show(upgraded(s)), "0")
    C.Check.str("steady archive index is empty", Nat.show(old_reports(s)), "0")
    measure(s, n, mails, 3n)

def full_bench() -> IO(Unit):
  do IO<Unit>:
    bench(1024n, 0n)
    bench(10000n, 0n)
    bench(10000n, 20000n)
    C.Check.str("quiet batch 32", Nat.show(quiet(32n)), "32")
    C.Check.str("quiet batch 256", Nat.show(quiet(256n)), "256")
    C.Check.str("quiet batch 2048", Nat.show(quiet(2048n)), "2048")

def stage(+which: Nat, +s: M.State) -> String:
  match which:
    case 0n: Nat.show(List.length(&2, M.Change, A.Auto.changes(s, 1793674000n)))
    case 1n: Nat.show(List.length(&2, Nat, Map.values(&2, Nat, SG.Signal.index(s))))
    case 2n: Nat.show(List.length(&2, M.Change, B.Shelve.wakes(B.Shelve.tick.post(s, 1793674000n, M.Work.kids.map(s)))))
    case 3n: Nat.show(List.length(&2, B.Out, T.Tick.drops(B.Shelve.threads(s), M.Work.kids.map(s), s, 1793674000n, B.Hub.wt.window(s))))
    case 4n: Nat.show(List.length(&2, B.Out, B.Merge.asks([M.Project{"p", "Project", "/tmp/autosettle", 1n, False{}}], B.Shelve.threads(s), s)))
    case 5n: W.step(B.Wt.stale(B.Shelve.threads(s), s, 1793674000n))
    case 6n: W.step(B.Step.before(B.Shelve.after(T.Tick.settles(B.Shelve.threads(s), T.Tick.parents(s), M.State.navs(s), M.Work.kids.map(s), s, 1793674000n, B.Hub.settle.window(s), B.Hub.settle.idle(s)), s, 1793674000n, M.Work.kids.map(s)), Nil{}, B.Step.none()))
    case _: tick(s, 1793674000n)

def stages(n: Nat, +s: M.State) -> IO(Unit):
  match n:
    case 0n: IO.pure(Unit, Unit{})
    case 1n+next:
      +k = next
      do IO<Unit>:
        +t0 : Nat <- IO.now()
        +result : String = stage(k, s)
        t1 : Nat <- IO.now()
        IO.print("stage=" ++ Nat.show(k) ++ " ms=" ++ Nat.show(Nat.sub(t1, t0)) ++ " bytes=" ++ Nat.show(String.length(result)))
        stages(k, s)

def main() -> IO(Unit):
  full_bench()
