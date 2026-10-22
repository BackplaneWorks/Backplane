import Base
import ./model.bend as M
import ./bot.bend as BT
import ./hot.bend as H

# A far mirror's snapshot
# -----------------------
# What a linked machine needs to show another machine's things live, made
# up from a read model as changes it already folds: the shortest list of
# existing M.Change that rebuilds the live part of the state (hot.bend).
# An old client or peer folds it like any other batch. It travels in far
# messages and mirror files only, never into a hub's own events.jsonl.
#
# What it holds: every project here; each live thread (H.Hot.thread.in)
# with the changes that set its record, its queue, its todos, work, fork
# and launch, and its last Snap.k() entries; open asks; live tasks; every
# bot here with its routines, memories, task skills, space and page. What
# it leaves out (settled and archived threads, older entries, closed asks,
# ended tasks) the owning machine answers on demand.
#
# Law far_snap_live: the state folded from a snapshot has the same live
# view (Snap.view) as the state it was made from; far_snap_fix: a
# snapshot of that fold is the snapshot again.

# entries kept per live thread
def Snap.k() -> Nat:
  40n

# an id of this machine's own (one elsewhere holds "~", far.bend's Fr.is)
def Snap.here(+id: String) -> Bool:
  Bool.not(String.contains(id, "~"))

# Projects
# --------

def Snap.project.k(skip: Bool, +id: String, title: String, root: String, +at: Nat, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match skip:
    case True{}:
      acc
    case False{}:
      M.ProjectCreated{id, title, root, at} <> acc

def Snap.projects(ps: List<&2, M.Project>, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match ps:
    case Nil{}:
      acc
    case M.Project{+id, title, root, +at, removed} <> t:
      Snap.projects(t, Snap.project.k(Bool.or(removed, Bool.not(Snap.here(id))), id, title, root, at, acc))

def Snap.st.projects(s: M.State) -> List<&2, M.Project>:
  match s:
    case M.State{ps, _, _, _, _, _, _, _, _, _, _}:
      Map.values(&2, M.Project, ps)

# Entries
# -------
# a thread's last k entries of its own, oldest first, from State.mine
# (newest first; it also holds what the thread sent elsewhere)

def Snap.window(+s: M.State, +id: String, +k: Nat) -> List<&2, M.Entry>:
  H.Hot.window(s, id, k)

def Snap.entry(e: M.Entry) -> M.Change:
  match e:
    case M.Msg{id, th, role, text, at}:
      M.MessagePosted{th, id, role, text, at}
    case M.Act{id, th, tone, kind, summary, at}:
      M.ActivityLogged{th, id, tone, kind, summary, at}

def Snap.entries(es: List<&2, M.Entry>, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match es:
    case Nil{}:
      acc
    case e <> t:
      Snap.entries(t, Snap.entry(e) <> acc)

# Threads
# -------

def Snap.when(go: Bool, c: M.Change, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match go:
    case True{}:
      c <> acc
    case False{}:
      acc

def Snap.settled(sh: M.Shelf) -> Bool:
  match sh:
    case M.Settled{}:
      True{}
    case _:
      False{}

# the settle state: a manual settle or unsettle as it was chosen; an
# auto-settle only while the thread shows settled (an unsettle by a
# message the window no longer holds needs no settle before it)
def Snap.settle(o: M.Override, shows: Bool, +id: String, +sa: Nat, +ua: Nat, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match o:
    case M.ForcedSettled{}:
      M.ThreadSettled{id, True{}, sa} <> acc
    case M.ForcedActive{}:
      M.ThreadUnsettled{id, ua} <> acc
    case M.NoOverride{}:
      Snap.when(shows, M.ThreadSettled{id, False{}, sa}, acc)

def Snap.queue(qs: List<&2, M.QItem>, +id: String, +at: Nat, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match qs:
    case Nil{}:
      acc
    case M.QItem{msg, text} <> t:
      Snap.queue(t, id, at, M.TurnQueued{id, msg, text, at} <> acc)

def Snap.set(+s: M.State, +k: String) -> String:
  M.Setting.get(s, k, "")

# what a thread's settings say that its changes set (todos, work, fork,
# launch), in that order
def Snap.sets(+s: M.State, +id: String, +at: Nat, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  +todo = Snap.set(s, "todos." ++ id)
  +bg = Snap.set(s, "bg." ++ id)
  +watch = Snap.set(s, "watch." ++ id)
  +fork = Snap.set(s, M.Fork.key(id))
  +launch = Snap.set(s, M.Launch.key(id))
  Snap.when(Bool.not(String.is_empty(launch)), M.ThreadLaunched{id, String.drop(launch, 1n), String.starts_with(launch, "1"), at},
    Snap.when(Bool.not(String.is_empty(fork)), M.ThreadForked{id, fork, at},
      Snap.when(Bool.not(String.is_empty(watch)), M.WorkSet{id, "watch", watch, at},
        Snap.when(Bool.not(String.is_empty(bg)), M.WorkSet{id, "bg", bg, at},
          Snap.when(Bool.not(String.is_empty(todo)), M.TodosSet{id, todo, at}, acc)))))

def Snap.thread.k(+s: M.State, +k: Nat, t: M.Thread, shows: Bool, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match t:
    case M.Thread{+id, p, ti, +c, +a, tu, +pe, _, o, +sa, +ua, pi, en, br, wt, pv, +mo, rm, im, _, _}:
      Snap.sets(s, id, a, Snap.queue(M.Queue.items(s, id), id, a,
        M.ModesSet{id, rm, im, mo, a} <>
          Snap.when(Nat.is_gt(pe, 0n), M.PendingChanged{id, pe, a},
            M.TurnChanged{id, tu, a} <>
              Snap.when(pi, M.ThreadPinned{id, True{}, c},
                Snap.settle(o, shows, id, sa, ua,
                  Snap.entries(Snap.window(s, id, k),
                    M.ThreadCreated{id, p, ti, en, br, wt, pv, mo, c} <> acc))))))

def Snap.thread(+s: M.State, +k: Nat, +t: M.Thread, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  Snap.thread.k(s, k, t, Snap.settled(M.Shelf.of(t)), acc)

def Snap.thread.if(go: Bool, +s: M.State, +k: Nat, +t: M.Thread, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match go:
    case True{}:
      Snap.thread(s, k, t, acc)
    case False{}:
      acc

def Snap.threads(ts: List<&2, M.Thread>, +s: M.State, +kept: Map<&2, Bool>, +k: Nat, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match ts:
    case Nil{}:
      acc
    case +t <> r:
      Snap.threads(r, s, kept, k, Snap.thread.if(Bool.and(Snap.here(M.Thread.id(t)), H.Hot.thread.in(s, kept, M.Thread.id(t))), s, k, t, acc))

# whether a thread is live here and this machine's own
def Snap.live(+s: M.State, +kept: Map<&2, Bool>, +id: String) -> Bool:
  Bool.and(Snap.here(id), H.Hot.thread.in(s, kept, id))

# Asks and tasks
# --------------

def Snap.asks(xs: List<&2, M.Ask>, +s: M.State, +kept: Map<&2, Bool>, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match xs:
    case Nil{}:
      acc
    case M.Ask{id, +th, k, tool, d, o, op, _} <> t:
      Snap.asks(t, s, kept, Snap.when(Bool.and(op, Snap.live(s, kept, th)), M.AskOpened{id, th, k, tool, d, o, 0n}, acc))

def Snap.task.k(go: Bool, x: M.Task, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match go x:
    case True{} M.Task{+id, p, c, depth, st, su, dl, pv, mo}:
      Snap.when(dl, M.TaskDelivered{id, 0n}, M.TaskStateSet{id, st, su, 0n} <> M.TaskDelegated{id, p, c, depth, pv, mo, 0n} <> acc)
    case False{} _:
      acc

def Snap.tasks(xs: List<&2, M.Task>, +s: M.State, +kept: Map<&2, Bool>, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match xs:
    case Nil{}:
      acc
    case +x <> t:
      Snap.tasks(t, s, kept, Snap.task.k(Bool.and(H.Hot.task(x), Snap.live(s, kept, H.Hot.task.parent(x))), x, acc))

# Bots
# ----

def Snap.routine.k(skip: Bool, r: BT.Routine, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match skip r:
    case True{} _:
      acc
    case False{} BT.Routine{+id, bot, name, cron, prompt, on, +last, +created, _, once, tz, kind, done, +slot}:
      Snap.when(done, M.RoutineDone{id, last},
        Snap.when(Nat.is_gt(slot, 0n), M.RoutineSlot{id, slot, last},
          M.RoutineFired{id, last} <> M.RoutinePlan{id, once, tz, kind, created} <> M.RoutineSet{id, bot, name, cron, prompt, on, created} <> acc))

def Snap.routine.gone(r: BT.Routine) -> Bool:
  match r:
    case BT.Routine{+id, _, _, _, _, _, _, _, deleted, _, _, _, _, _}:
      Bool.or(deleted, Bool.not(Snap.here(id)))

def Snap.routines(xs: List<&2, BT.Routine>, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match xs:
    case Nil{}:
      acc
    case +r <> t:
      Snap.routines(t, Snap.routine.k(Snap.routine.gone(r), r, acc))

def Snap.mem(m: BT.Mem, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match m:
    case BT.Mem{+bot, +key, +kind, +text, +tags, +created, +updated}:
      Snap.when(Bool.not(Nat.is_eq(created, updated)), M.MemorySaved{bot, key, kind, text, tags, updated}, M.MemorySaved{bot, key, kind, text, tags, created} <> acc)

def Snap.mem.if(go: Bool, m: BT.Mem, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match go:
    case True{}:
      Snap.mem(m, acc)
    case False{}:
      acc

def Snap.mem.bot(m: BT.Mem) -> String:
  match m:
    case BT.Mem{b, _, _, _, _, _, _}:
      b

def Snap.mems(xs: List<&2, BT.Mem>, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match xs:
    case Nil{}:
      acc
    case +m <> t:
      Snap.mems(t, Snap.mem.if(Snap.here(Snap.mem.bot(m)), m, acc))

def Snap.skills(xs: List<&2, BT.TRow>, +bot: String, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match xs:
    case Nil{}:
      acc
    case BT.TRow{slug, json} <> t:
      Snap.skills(t, bot, M.TaskSet{bot, slug, json, 0n} <> acc)

def Snap.space(+spec: String, +bot: String, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  Snap.when(Bool.not(String.is_empty(spec)), M.SpaceSet{bot, spec, 0n}, acc)

def Snap.page(p: BT.Page, +bot: String, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match p:
    case BT.Page{+html, +at}:
      Snap.when(Bool.not(String.is_empty(html)), M.SpacePageSet{bot, html, at}, acc)

def Snap.page.of(w: BT.World, +id: String) -> BT.Page:
  match w:
    case BT.World{_, _, _, _, _, _, _, _, _, _, pg, _}:
      BT.Got(BT.Page, Map.get(BT.Page, BT.Page{"", 0n}, pg, id))

def Snap.space.of(w: BT.World, +id: String) -> String:
  match w:
    case BT.World{_, _, _, _, _, _, _, sp, _, _, _, _}:
      BT.Got(String, Map.get(String, "", sp, id))

def Snap.bot.k(skip: Bool, +w: BT.World, b: BT.Bot, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match skip b:
    case True{} _:
      acc
    case False{} BT.Bot{+id, name, thread, look, persona, created, _}:
      Snap.page(Snap.page.of(w, id), id, Snap.space(Snap.space.of(w, id), id,
        Snap.skills(BT.World.tasks_of(w, id), id, M.BotSet{id, name, thread, look, persona, created} <> acc)))

def Snap.bot.gone(b: BT.Bot) -> Bool:
  match b:
    case BT.Bot{+id, _, _, _, _, _, deleted}:
      Bool.or(deleted, Bool.not(Snap.here(id)))

def Snap.bots(xs: List<&2, BT.Bot>, +w: BT.World, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match xs:
    case Nil{}:
      acc
    case +b <> t:
      Snap.bots(t, w, Snap.bot.k(Snap.bot.gone(b), w, b, acc))

def Snap.world(+w: BT.World, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match w:
    case BT.World{bs, _, _, _, ro, ms, _, _, _, _, _, _}:
      Snap.mems(Map.values(&2, BT.Mem, ms), Snap.routines(Map.values(&2, BT.Routine, ro), Snap.bots(Map.values(&2, BT.Bot, bs), w, acc)))

# The snapshot
# ------------

def Snap.st.threads(s: M.State) -> List<&2, M.Thread>:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}:
      Map.values(&2, M.Thread, ts)

def Snap.st.tasks(s: M.State) -> List<&2, M.Task>:
  match s:
    case M.State{_, _, _, _, tk, _, _, _, _, _, _}:
      Map.values(&2, M.Task, tk)

def Snap.of.k(+s: M.State, +kept: Map<&2, Bool>, +k: Nat) -> List<&2, M.Change>:
  List.reverse(&2, M.Change,
    Snap.tasks(Snap.st.tasks(s), s, kept,
      Snap.asks(M.Asks.all(s), s, kept,
        Snap.world(M.State.world.of(s),
          Snap.threads(Snap.st.threads(s), s, kept, k,
            Snap.projects(Snap.st.projects(s), Nil{}))))))

# the live part of a state as changes, oldest first, each live thread with
# its last k entries
def Snap.of.at(+s: M.State, +k: Nat) -> List<&2, M.Change>:
  Snap.of.k(s, H.Hot.kept(s), k)

def Snap.of(+s: M.State) -> List<&2, M.Change>:
  Snap.of.at(s, Snap.k())

# a snapshot folded into a read model of its own
def Snap.fold(cs: List<&2, M.Change>) -> M.State:
  M.State.replay(cs, M.State.empty())

# A machine's state for the laws: a project (and one removed); a live
# thread with a queue, todos, work, an open ask, a live task and more
# entries than the window; a pinned one; one unsettled by hand; one that
# settled while its turn runs; one settled by hand; a settled, an archived
# and a deleted one; a thread elsewhere; a bot with a routine, a memory
# saved twice, a task skill, a space and a page; a deleted bot.
def Snap.demo.log() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "Board", "/b", 1n}, M.ProjectCreated{"p2", "Old", "/o", 1n}, M.ProjectRemoved{"p2", 2n},
    M.ThreadCreated{"t1", "p1", "Live", M.Local{}, "", "", "claude", "opus", 3n},
    M.MessagePosted{"t1", "m1", M.User{}, "one", 4n}, M.MessagePosted{"t1", "m2", M.Assistant{}, "two", 5n},
    M.ActivityLogged{"t1", "a1", "tool", "Bash", "ls", 6n}, M.MessagePosted{"t1", "m3", M.User{}, "three", 7n},
    M.TurnChanged{"t1", M.Running{}, 8n}, M.PendingChanged{"t1", 1n, 8n}, M.TurnQueued{"t1", "q1", "next", 9n}, M.TurnQueued{"t1", "q2", "later", 9n},
    M.TodosSet{"t1", "[1]", 9n}, M.WorkSet{"t1", "bg", "row", 9n}, M.ModesSet{"t1", M.AutoMode{}, M.PlanMode{}, "fable", 9n},
    M.AskOpened{"k1", "t1", "approval", "Bash", "rm", "", 10n}, M.AskOpened{"k2", "t1", "input", "", "?", "", 10n}, M.AskClosed{"k2", "yes", 11n},
    M.ThreadCreated{"t2", "p1", "Pinned", M.Local{}, "", "", "codex", "", 12n}, M.ThreadPinned{"t2", True{}, 13n},
    M.ThreadCreated{"t3", "p1", "Back", M.Worktree{}, "b", "/w", "claude", "", 14n}, M.ThreadSettled{"t3", False{}, 15n}, M.ThreadUnsettled{"t3", 16n},
    M.ThreadCreated{"t4", "p1", "Busy", M.Local{}, "", "", "claude", "", 17n}, M.MessagePosted{"t4", "m4", M.User{}, "go", 18n}, M.TurnChanged{"t4", M.Running{}, 19n}, M.ThreadSettled{"t4", False{}, 20n},
    M.ThreadCreated{"t5", "p1", "Hand", M.Local{}, "", "", "claude", "", 21n}, M.TurnChanged{"t5", M.Running{}, 22n}, M.ThreadSettled{"t5", True{}, 23n},
    M.ThreadCreated{"t6", "p1", "Done", M.Local{}, "", "", "claude", "", 24n}, M.MessagePosted{"t6", "m6", M.User{}, "old", 25n}, M.ThreadSettled{"t6", False{}, 26n},
    M.ThreadCreated{"t7", "p1", "Shelved", M.Local{}, "", "", "claude", "", 27n}, M.ThreadArchived{"t7", True{}, 28n},
    M.ThreadCreated{"t8", "p1", "Gone", M.Local{}, "", "", "claude", "", 29n}, M.ThreadDeleted{"t8", 30n},
    M.ThreadCreated{"x~t9", "", "Far", M.Local{}, "", "", "claude", "", 31n},
    M.ThreadCreated{"t10", "p1", "Child", M.Local{}, "", "", "codex", "", 32n}, M.ThreadSettled{"t10", False{}, 33n},
    M.TaskDelegated{"j1", "t1", "t10", 1n, "codex", "", 34n}, M.TaskStateSet{"j1", M.TaskRunning{}, "on it", 35n},
    M.TaskDelegated{"j2", "t1", "t6", 1n, "claude", "", 36n}, M.TaskStateSet{"j2", M.TaskDone{}, "ok", 37n}, M.TaskDelivered{"j2", 38n},
    M.ThreadLaunched{"t10", "t1", True{}, 38n}, M.ThreadForked{"t3", "t1", 38n},
    M.BotSet{"b1", "Kit", "t2", 2n, "calm", 39n}, M.RoutineSet{"r1", "b1", "Daily", "0 9 * * *", "check", True{}, 40n}, M.RoutineFired{"r1", 41n},
    M.RoutinePlan{"r1", 0n, "person", "", 41n}, M.RoutineSlot{"r1", 900n, 42n},
    M.MemorySaved{"b1", "k", "fact", "v1", "", 43n}, M.MemorySaved{"b1", "k", "fact", "v2", "", 44n},
    M.TaskSet{"b1", "buy", "{}", 45n}, M.SpaceSet{"b1", "{\"a\":1}", 46n}, M.SpacePageSet{"b1", "<p>hi</p>", 47n},
    M.BotSet{"b2", "Old", "t6", 1n, "", 48n}, M.BotDeleted{"b2", 49n}]

def Snap.demo() -> M.State:
  Snap.fold(Snap.demo.log())

# the threads a snapshot makes, by id
def Snap.made.go(cs: List<&2, M.Change>, acc: String) -> String:
  match cs:
    case Nil{}:
      acc
    case M.ThreadCreated{id, _, _, _, _, _, _, _, _} <> t:
      Snap.made.go(t, acc ++ id ++ " ")
    case _ <> t:
      Snap.made.go(t, acc)

def Snap.made(cs: List<&2, M.Change>) -> String:
  Snap.made.go(cs, "")
