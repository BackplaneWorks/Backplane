import Base
import ./model.bend as M
import ./bot.bend as BT

# What is live ("hot")
# --------------------
# One definition of the live part of a read model, for everything that
# keeps only that resident: a far mirror (farsnap.bend) and the hub's hot
# set (the store's records of settled threads are read on demand).
#
# A thread is live while it is neither deleted nor archived and it sits on
# the Pinned or Active shelf, or a turn runs or something waits on the
# person (M.Thread.busy). No clock is read: an auto-settle counts once the
# settle tick has logged it (ThreadSettled), so the same log gives the same
# answer on every machine.

def Hot.shelf(s: M.Shelf) -> Bool:
  match s:
    case M.Pinned{}:
      True{}
    case M.Active{}:
      True{}
    case M.Snoozed{}:
      True{}
    case _:
      False{}

def Hot.thread.k(gone: Bool, live: Bool, busy: Bool) -> Bool:
  match gone:
    case True{}:
      False{}
    case False{}:
      Bool.or(live, busy)

def Hot.gone(t: M.Thread) -> Bool:
  match t:
    case M.Thread{_, _, _, _, _, _, _, archived, _, _, _, _, _, _, _, _, _, _, _, _, deleted}:
      Bool.or(archived, deleted)

def Hot.busy(t: M.Thread) -> Bool:
  match t:
    case M.Thread{_, _, _, _, _, turn, +pending, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      M.Thread.busy(turn, pending)

# whether a thread, by its own record, is live
def Hot.thread(+t: M.Thread) -> Bool:
  Hot.thread.k(Hot.gone(t), Hot.shelf(M.Shelf.of(t)), Hot.busy(t))

# A delegated task is live until it has ended and its result reached its
# parent.
def Hot.task(t: M.Task) -> Bool:
  match t:
    case M.Task{_, _, _, _, st, _, delivered, _, _}:
      Bool.or(Bool.not(M.TaskState.terminal(st)), Bool.not(delivered))

def Hot.ask(a: M.Ask) -> Bool:
  match a:
    case M.Ask{_, _, _, _, _, _, open, _}:
      open

# The threads something live keeps live although their own record would
# not: an open ask in them, a live task they delegated or run, a bot's
# thread. (id → True)
def Hot.mark(+m: Map<&2, Bool>, +id: String) -> Map<&2, Bool>:
  Bool.pick(Map<&2, Bool>, String.is_empty(id), m, Map.set(&2, Bool, m, id, True{}))

def Hot.ask.thread(a: M.Ask) -> String:
  match a:
    case M.Ask{_, th, _, _, _, _, _, _}:
      th

def Hot.asks.k(live: Bool, +th: String, +m: Map<&2, Bool>) -> Map<&2, Bool>:
  match live:
    case True{}:
      Hot.mark(m, th)
    case False{}:
      m

def Hot.asks(xs: List<&2, M.Ask>, +m: Map<&2, Bool>) -> Map<&2, Bool>:
  match xs:
    case Nil{}:
      m
    case M.Ask{_, +th, _, _, _, _, op, _} <> t:
      Hot.asks(t, Hot.asks.k(op, th, m))

def Hot.task.parent(t: M.Task) -> String:
  match t:
    case M.Task{_, p, _, _, _, _, _, _, _}:
      p

def Hot.task.child(t: M.Task) -> String:
  match t:
    case M.Task{_, _, c, _, _, _, _, _, _}:
      c

def Hot.tasks.k(live: Bool, +p: String, +c: String, +m: Map<&2, Bool>) -> Map<&2, Bool>:
  match live:
    case True{}:
      Hot.mark(Hot.mark(m, p), c)
    case False{}:
      m

def Hot.tasks(xs: List<&2, M.Task>, +m: Map<&2, Bool>) -> Map<&2, Bool>:
  match xs:
    case Nil{}:
      m
    case +x <> t:
      Hot.tasks(t, Hot.tasks.k(Hot.task(x), Hot.task.parent(x), Hot.task.child(x), m))

def Hot.bots.k(gone: Bool, +th: String, +m: Map<&2, Bool>) -> Map<&2, Bool>:
  match gone:
    case True{}:
      m
    case False{}:
      Hot.mark(m, th)

def Hot.bots(xs: List<&2, BT.Bot>, +m: Map<&2, Bool>) -> Map<&2, Bool>:
  match xs:
    case Nil{}:
      m
    case BT.Bot{_, _, +th, _, _, _, +deleted} <> t:
      Hot.bots(t, Hot.bots.k(deleted, th, m))

def Hot.st.asks(s: M.State) -> List<&2, M.Ask>:
  M.Asks.all(s)

def Hot.st.tasks(s: M.State) -> List<&2, M.Task>:
  match s:
    case M.State{_, _, _, _, tk, _, _, _, _, _, _}:
      Map.values(&2, M.Task, tk)

def Hot.world.bots(w: BT.World) -> List<&2, BT.Bot>:
  match w:
    case BT.World{bs, _, _, _, _, _, _, _, _, _, _, _}:
      Map.values(&2, BT.Bot, bs)

def Hot.st.bots(s: M.State) -> List<&2, BT.Bot>:
  Hot.world.bots(M.State.world.of(s))

def Hot.kept(+s: M.State) -> Map<&2, Bool>:
  Hot.bots(Hot.st.bots(s), Hot.tasks(Hot.st.tasks(s), Hot.asks(Hot.st.asks(s), Map.new(&2, Bool))))

def Hot.has(r: Map<&2, Bool> & Bool) -> Bool:
  (_, b) = r
  b

def Hot.of.k(+t: M.Thread, kept: Bool) -> Bool:
  Hot.thread.k(Hot.gone(t), Bool.or(Hot.thread(t), kept), False{})

def Hot.got(r: Map<&2, M.Thread> & M.Thread) -> M.Thread:
  (_, t) = r
  t

def Hot.threads(s: M.State) -> Map<&2, M.Thread>:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}:
      ts

# whether the thread `id` is live in a state, with what keeps it live (an
# absent thread is not: M.Thread.blank is deleted)
def Hot.thread.in(+s: M.State, +kept: Map<&2, Bool>, +id: String) -> Bool:
  Hot.of.k(Hot.got(Map.get(M.Thread, M.Thread.blank(id), Hot.threads(s), id)), Hot.has(Map.has(&2, Bool, kept, id)))

def Hot.thread.of(+s: M.State, +id: String) -> Bool:
  Hot.thread.in(s, Hot.kept(s), id)

# A live thread's recent window: its last k entries of its own, oldest
# first, from State.mine (newest first; it also holds what the thread sent
# to others, which their own windows hold)

def Hot.own(+e: M.Entry, +id: String) -> Bool:
  String.eq(M.Idx.thread(e), id)

def Hot.push(own: Bool, e: M.Entry, acc: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match own:
    case True{}:
      e <> acc
    case False{}:
      acc

def Hot.dec(own: Bool, +k: Nat) -> Nat:
  match own:
    case True{}:
      Nat.sub(k, 1n)
    case False{}:
      k

def Hot.win(es: List<&2, M.Entry>, +id: String, +k: Nat, acc: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match es k:
    case _ 0n:
      acc
    case Nil{} _:
      acc
    case +e <> +t 1n+p:
      Hot.win(t, id, Hot.dec(Hot.own(e, id), 1n+p), Hot.push(Hot.own(e, id), e, acc))

def Hot.window(+s: M.State, +id: String, +k: Nat) -> List<&2, M.Entry>:
  Hot.win(M.State.mine(s, id), id, k, Nil{})


# The live view
# -------------
# What is live in a state, read straight from it: every project here, each
# live thread (its record with the settle times read as its shelf, its
# queue, its last k entries, the settings its changes set), open asks,
# live tasks, and every bot here with its routines, memories, task skills,
# space and page. Two states with the same view show the same live things;
# a far mirror keeps only what makes this view (farsnap.bend, law
# far_snap_live).

type HRow is Data:
  HProject{p: M.Project}
  HThread{t: M.Thread, shelf: M.Shelf, queue: List<&2, M.QItem>, entries: List<&2, M.Entry>, sets: String}
  HAsk{a: M.Ask}
  HTask{k: M.Task}
  HBot{b: BT.Bot, space: String, page: BT.Page, skills: List<&2, BT.TRow>}
  HRoutine{r: BT.Routine}
  HMem{m: BT.Mem}

# an id of this machine's own (one elsewhere holds "~", far.bend's Fr.is)
def Hot.here(+id: String) -> Bool:
  Bool.not(String.contains(id, "~"))

def Hot.row.if(go: Bool, r: HRow, acc: List<&2, HRow>) -> List<&2, HRow>:
  match go:
    case True{}:
      r <> acc
    case False{}:
      acc

def Hot.v.projects(ps: List<&2, M.Project>, acc: List<&2, HRow>) -> List<&2, HRow>:
  match ps:
    case Nil{}:
      acc
    case M.Project{+id, ti, rt, c, +removed} <> t:
      Hot.v.projects(t, Hot.row.if(Bool.and(Bool.not(removed), Hot.here(id)), HProject{M.Project{id, ti, rt, c, removed}}, acc))

# the record with what no client reads off it cleared: the settle and
# unsettle times (the shelf says where it sits) and the agent's session
def Hot.plain(t: M.Thread) -> M.Thread:
  match t:
    case M.Thread{id, p, ti, c, a, tu, pe, ar, o, _, _, pi, en, br, wt, pv, mo, rm, im, _, de}:
      M.Thread{id, p, ti, c, a, tu, pe, ar, o, 0n, 0n, pi, en, br, wt, pv, mo, rm, im, "", de}

def Hot.sets(+s: M.State, +id: String) -> String:
  M.Setting.get(s, "todos." ++ id, "") ++ "\u{1}" ++ M.Setting.get(s, "bg." ++ id, "") ++ "\u{1}" ++ M.Setting.get(s, "watch." ++ id, "") ++ "\u{1}" ++
    M.Setting.get(s, M.Fork.key(id), "") ++ "\u{1}" ++ M.Setting.get(s, M.Launch.key(id), "")

def Hot.v.thread(+s: M.State, +t: M.Thread, +k: Nat) -> HRow:
  HThread{Hot.plain(t), M.Shelf.of(t), M.Queue.items(s, M.Thread.id(t)), Hot.window(s, M.Thread.id(t), k), Hot.sets(s, M.Thread.id(t))}

def Hot.v.thread.if(go: Bool, +s: M.State, +t: M.Thread, +k: Nat, acc: List<&2, HRow>) -> List<&2, HRow>:
  match go:
    case True{}:
      Hot.v.thread(s, t, k) <> acc
    case False{}:
      acc

def Hot.v.threads(ts: List<&2, M.Thread>, +s: M.State, +kept: Map<&2, Bool>, +k: Nat, acc: List<&2, HRow>) -> List<&2, HRow>:
  match ts:
    case Nil{}:
      acc
    case +t <> r:
      Hot.v.threads(r, s, kept, k, Hot.v.thread.if(Bool.and(Hot.here(M.Thread.id(t)), Hot.thread.in(s, kept, M.Thread.id(t))), s, t, k, acc))

def Hot.live(+s: M.State, +kept: Map<&2, Bool>, +id: String) -> Bool:
  Bool.and(Hot.here(id), Hot.thread.in(s, kept, id))

def Hot.v.asks(xs: List<&2, M.Ask>, +s: M.State, +kept: Map<&2, Bool>, acc: List<&2, HRow>) -> List<&2, HRow>:
  match xs:
    case Nil{}:
      acc
    case +a <> t:
      Hot.v.asks(t, s, kept, Hot.row.if(Bool.and(Hot.ask(a), Hot.live(s, kept, Hot.ask.thread(a))), HAsk{a}, acc))

def Hot.v.tasks(xs: List<&2, M.Task>, +s: M.State, +kept: Map<&2, Bool>, acc: List<&2, HRow>) -> List<&2, HRow>:
  match xs:
    case Nil{}:
      acc
    case +x <> t:
      Hot.v.tasks(t, s, kept, Hot.row.if(Bool.and(Hot.task(x), Hot.live(s, kept, Hot.task.parent(x))), HTask{x}, acc))

def Hot.page.of(w: BT.World, +id: String) -> BT.Page:
  match w:
    case BT.World{_, _, _, _, _, _, _, _, _, _, pg, _}:
      BT.Got(BT.Page, Map.get(BT.Page, BT.Page{"", 0n}, pg, id))

def Hot.space.of(w: BT.World, +id: String) -> String:
  match w:
    case BT.World{_, _, _, _, _, _, _, sp, _, _, _, _}:
      BT.Got(String, Map.get(String, "", sp, id))

def Hot.bot.gone(b: BT.Bot) -> Bool:
  match b:
    case BT.Bot{+id, _, _, _, _, _, deleted}:
      Bool.or(deleted, Bool.not(Hot.here(id)))

def Hot.v.bot(+w: BT.World, +b: BT.Bot) -> HRow:
  HBot{b, Hot.space.of(w, BT.Bot.id(b)), Hot.page.of(w, BT.Bot.id(b)), BT.World.tasks_of(w, BT.Bot.id(b))}

def Hot.v.bots(xs: List<&2, BT.Bot>, +w: BT.World, acc: List<&2, HRow>) -> List<&2, HRow>:
  match xs:
    case Nil{}:
      acc
    case +b <> t:
      Hot.v.bots(t, w, Hot.row.if(Bool.not(Hot.bot.gone(b)), Hot.v.bot(w, b), acc))

def Hot.routine.gone(r: BT.Routine) -> Bool:
  match r:
    case BT.Routine{+id, _, _, _, _, _, _, _, deleted, _, _, _, _, _}:
      Bool.or(deleted, Bool.not(Hot.here(id)))

def Hot.v.routines(xs: List<&2, BT.Routine>, acc: List<&2, HRow>) -> List<&2, HRow>:
  match xs:
    case Nil{}:
      acc
    case +r <> t:
      Hot.v.routines(t, Hot.row.if(Bool.not(Hot.routine.gone(r)), HRoutine{r}, acc))

def Hot.mem.bot(m: BT.Mem) -> String:
  match m:
    case BT.Mem{b, _, _, _, _, _, _}:
      b

def Hot.v.mems(xs: List<&2, BT.Mem>, acc: List<&2, HRow>) -> List<&2, HRow>:
  match xs:
    case Nil{}:
      acc
    case +m <> t:
      Hot.v.mems(t, Hot.row.if(Hot.here(Hot.mem.bot(m)), HMem{m}, acc))

def Hot.v.world(+w: BT.World, acc: List<&2, HRow>) -> List<&2, HRow>:
  match w:
    case BT.World{bs, _, _, _, ro, ms, _, _, _, _, _, _}:
      Hot.v.mems(Map.values(&2, BT.Mem, ms), Hot.v.routines(Map.values(&2, BT.Routine, ro), Hot.v.bots(Map.values(&2, BT.Bot, bs), w, acc)))

def Hot.st.projects(s: M.State) -> List<&2, M.Project>:
  match s:
    case M.State{ps, _, _, _, _, _, _, _, _, _, _}:
      Map.values(&2, M.Project, ps)

def Hot.view.k(+s: M.State, +kept: Map<&2, Bool>, +k: Nat) -> List<&2, HRow>:
  List.reverse(&2, HRow,
    Hot.v.tasks(Hot.st.tasks(s), s, kept,
      Hot.v.asks(Hot.st.asks(s), s, kept,
        Hot.v.world(M.State.world.of(s),
          Hot.v.threads(Map.values(&2, M.Thread, Hot.threads(s)), s, kept, k,
            Hot.v.projects(Hot.st.projects(s), Nil{}))))))

# the live view of a state, each live thread with its last k entries
def Hot.view(+s: M.State, +k: Nat) -> List<&2, HRow>:
  Hot.view.k(s, Hot.kept(s), k)
